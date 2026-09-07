//! Bounded local socket exchanges. Idle peers are allowed; unfinished frames and writes expire.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::sync::{Condvar, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use serde::{de::DeserializeOwned, Serialize};

use crate::framing::{self, FrameError};

/// Total time allowed to authenticate or finish a frame once its first byte arrives.
pub const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(5);
/// Total writer wait plus delivery budget, independent of the peer's incremental progress.
pub const DELIVERY_TIMEOUT: Duration = Duration::from_secs(2);

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "local exchange timed out"))
}

/// A socket reader whose frame deadline includes buffered bytes and slow incremental delivery.
pub struct SocketReader {
    reader: BufReader<TcpStream>,
    deadline: Option<Instant>,
    frame_timeout: Duration,
    stop: Option<Arc<AtomicBool>>,
}

impl SocketReader {
    /// Own a reader without imposing an idle-session timeout.
    pub fn new(stream: TcpStream) -> Self {
        Self {
            reader: BufReader::new(stream),
            deadline: None,
            frame_timeout: EXCHANGE_TIMEOUT,
            stop: None,
        }
    }

    /// Interrupt idle or incomplete reads when the owning service shuts down.
    pub fn interrupted_by(mut self, stop: Arc<AtomicBool>) -> Self {
        self.stop = Some(stop);
        self
    }

    /// Read one JSON frame; an opening exchange supplies its absolute authentication deadline.
    pub fn json<T: DeserializeOwned>(
        &mut self,
        opening: Option<Instant>,
    ) -> Result<Option<T>, FrameError> {
        self.deadline = opening;
        framing::read_json_line(self)
    }

    /// Read one native frame with the same opening/idle distinction as JSON framing.
    pub fn native<T: DeserializeOwned>(
        &mut self,
        opening: Option<Instant>,
    ) -> Result<Option<T>, FrameError> {
        self.deadline = opening;
        framing::read_native(self)
    }
}

impl BufRead for SocketReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        loop {
            if self
                .stop
                .as_ref()
                .is_some_and(|stop| stop.load(Ordering::Acquire))
            {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "service stopped",
                ));
            }
            let timeout = self.deadline.map(remaining).transpose()?;
            let timeout = if self.stop.is_some() {
                Some(
                    timeout
                        .unwrap_or(Duration::from_millis(250))
                        .min(Duration::from_millis(250)),
                )
            } else {
                timeout
            };
            self.reader.get_ref().set_read_timeout(timeout)?;
            match self.reader.fill_buf() {
                Ok(_) => break,
                Err(error)
                    if self.stop.is_some()
                        && matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        ) =>
                {
                    continue
                }
                Err(error) => return Err(error),
            }
        }
        if !self.reader.buffer().is_empty() && self.deadline.is_none() {
            self.deadline = Some(Instant::now() + self.frame_timeout);
        }
        Ok(self.reader.buffer())
    }

    fn consume(&mut self, amount: usize) {
        self.reader.consume(amount);
    }
}

impl Read for SocketReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let bytes = self.fill_buf()?;
        let count = bytes.len().min(output.len());
        output[..count].copy_from_slice(&bytes[..count]);
        self.consume(count);
        Ok(count)
    }
}

/// One serialized writer with an independent shutdown handle, so cleanup never waits for its lock.
#[derive(Debug)]
pub struct SocketWriter {
    stream: Mutex<TcpStream>,
    shutdown: TcpStream,
    closed: Mutex<bool>,
    wake: Condvar,
}

impl SocketWriter {
    /// Own the stream and retain a handle that can interrupt stalled I/O.
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        Ok(Self {
            shutdown: stream.try_clone()?,
            stream: Mutex::new(stream),
            closed: Mutex::new(false),
            wake: Condvar::new(),
        })
    }

    /// Interrupt both directions without acquiring the serialization lock.
    pub fn close(&self) {
        *self
            .closed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.wake.notify_all();
        let _ = self.shutdown.shutdown(Shutdown::Both);
    }

    /// Wait between liveness probes, ending immediately when this connection is retired.
    pub fn wait_closed(&self, interval: Duration) -> bool {
        let closed = self
            .closed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *self
            .wake
            .wait_timeout_while(closed, interval, |closed| !*closed)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
    }

    /// Acquire serialization within a total deadline without interrupting another writer.
    pub fn until(&self, deadline: Instant) -> io::Result<SocketWrite<'_>> {
        self.until_unless(deadline, || false)
    }

    /// Wait for serialization while still observing caller cancellation, before any bytes are sent.
    pub fn until_unless(
        &self,
        deadline: Instant,
        cancelled: impl Fn() -> bool,
    ) -> io::Result<SocketWrite<'_>> {
        loop {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "request cancelled",
                ));
            }
            remaining(deadline)?;
            match self.stream.try_lock() {
                Ok(stream) => {
                    return Ok(SocketWrite {
                        owner: self,
                        stream,
                        deadline,
                    })
                }
                Err(TryLockError::Poisoned(error)) => {
                    return Ok(SocketWrite {
                        owner: self,
                        stream: error.into_inner(),
                        deadline,
                    })
                }
                Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(2)),
            }
        }
    }

    /// Send a JSON response within a bounded total delivery budget.
    pub fn json<T: Serialize>(&self, value: &T) -> Result<(), FrameError> {
        let result = self
            .until(Instant::now() + DELIVERY_TIMEOUT)
            .map_err(FrameError::from)
            .and_then(|mut writer| framing::write_json_line(&mut writer, value));
        if result.is_err() {
            self.close();
        }
        result
    }

    /// Send a native frame within a bounded total delivery budget.
    pub fn native<T: Serialize>(&self, value: &T) -> Result<(), FrameError> {
        self.native_until(value, Instant::now() + DELIVERY_TIMEOUT)
    }

    /// Send a native frame with a caller-supplied total deadline (also used for human controls).
    pub fn native_until<T: Serialize>(
        &self,
        value: &T,
        deadline: Instant,
    ) -> Result<(), FrameError> {
        let result = self
            .until(deadline)
            .map_err(FrameError::from)
            .and_then(|mut writer| framing::write_native(&mut writer, value));
        if result.is_err() {
            self.close();
        }
        result
    }
}

/// A writer lease whose deadline covers every partial write, including chunked transfers.
pub struct SocketWrite<'a> {
    owner: &'a SocketWriter,
    stream: MutexGuard<'a, TcpStream>,
    deadline: Instant,
}

impl Write for SocketWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = remaining(self.deadline).and_then(|left| {
            self.stream.set_write_timeout(Some(left))?;
            self.stream.write(bytes)
        });
        if result.is_err() {
            self.owner.close();
        }
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::net::TcpListener;

    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        (listener.accept().unwrap().0, peer)
    }

    #[test]
    fn abandoned_and_partial_native_openings_expire() {
        for prefix in [vec![], vec![10], vec![10, 0, 0, 0, b'{']] {
            let (stream, mut peer) = pair();
            peer.write_all(&prefix).unwrap();
            let mut reader = SocketReader::new(stream);
            assert!(reader
                .native::<Value>(Some(Instant::now() + Duration::from_millis(60)))
                .is_err());
        }
    }

    #[test]
    fn idle_sessions_survive_but_buffered_partial_frames_expire() {
        let (stream, mut peer) = pair();
        let mut reader = SocketReader::new(stream);
        reader.frame_timeout = Duration::from_millis(60);
        std::thread::scope(|scope| {
            scope.spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                peer.write_all(b"{}\n{\"partial\":").unwrap();
                std::thread::sleep(Duration::from_millis(200));
            });
            assert_eq!(reader.json::<Value>(None).unwrap(), Some(json!({})));
            let started = Instant::now();
            assert!(reader.json::<Value>(None).is_err());
            assert!(started.elapsed() < Duration::from_millis(180));
        });
    }

    #[test]
    fn incremental_progress_does_not_restart_the_frame_clock() {
        let (stream, mut peer) = pair();
        let mut reader = SocketReader::new(stream);
        reader.frame_timeout = Duration::from_millis(100);
        std::thread::scope(|scope| {
            scope.spawn(move || {
                for _ in 0..12 {
                    if peer.write_all(b" ").is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
            });
            let started = Instant::now();
            assert!(reader.json::<Value>(None).is_err());
            assert!(started.elapsed() < Duration::from_millis(250));
        });
    }

    #[test]
    fn caller_expiry_does_not_disconnect_another_writer_and_close_needs_no_lock() {
        let (stream, mut peer) = pair();
        let writer = SocketWriter::new(stream).unwrap();
        let _held = writer
            .until(Instant::now() + Duration::from_secs(2))
            .unwrap();
        assert!(writer
            .until(Instant::now() + Duration::from_millis(30))
            .is_err());
        peer.set_read_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        assert!(
            peer.read(&mut [0]).is_err(),
            "a waiting call's deadline leaves the connection alive"
        );
        writer.close();
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
        assert!(writer.wait_closed(Duration::from_secs(2)));
    }

    #[test]
    fn a_nonreading_peer_cannot_hold_delivery_indefinitely() {
        let (stream, _peer) = pair();
        let writer = SocketWriter::new(stream).unwrap();
        let started = Instant::now();
        let mut output = writer.until(started + Duration::from_millis(100)).unwrap();
        let block = vec![0; 64 * 1024];
        let mut failed = false;
        for _ in 0..1024 {
            if output.write_all(&block).is_err() {
                failed = true;
                break;
            }
        }
        assert!(
            failed,
            "bounded 64 MiB fixture must reach socket backpressure"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(writer.wait_closed(Duration::from_millis(1)));
    }

    #[test]
    fn stopping_the_service_interrupts_an_idle_reader() {
        let (stream, _peer) = pair();
        let stop = Arc::new(AtomicBool::new(false));
        let mut reader = SocketReader::new(stream).interrupted_by(stop.clone());
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(20));
                stop.store(true, Ordering::Release);
            });
            assert!(reader.json::<Value>(None).is_err());
        });
    }
}
