//! Bounded local connection exchanges. Idle peers are allowed; unfinished frames and writes expire.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::sync::{Condvar, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use serde::{de::DeserializeOwned, Serialize};

use ghostlight_relay::endpoint::Connection;
use ghostlight_relay::frame::{self, FrameError};

use crate::sync::lock;

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

/// A connection reader whose frame deadline includes buffered bytes and slow incremental delivery.
pub struct SocketReader {
    reader: BufReader<Connection>,
    deadline: Option<Instant>,
    frame_timeout: Duration,
    stop: Option<Arc<AtomicBool>>,
}

impl SocketReader {
    /// Own a reader without imposing an idle-session timeout.
    pub fn new(stream: Connection) -> Self {
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

    /// Read one native frame; an opening exchange supplies its absolute authentication deadline.
    pub fn native<T: DeserializeOwned>(
        &mut self,
        opening: Option<Instant>,
    ) -> Result<Option<T>, FrameError> {
        self.deadline = opening;
        frame::read_frame(self)
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
    stream: Mutex<Connection>,
    shutdown: Connection,
    closed: Mutex<bool>,
    wake: Condvar,
}

impl SocketWriter {
    /// Own the stream and retain a handle that can interrupt stalled I/O.
    pub fn new(stream: Connection) -> io::Result<Self> {
        Ok(Self {
            shutdown: stream.try_clone()?,
            stream: Mutex::new(stream),
            closed: Mutex::new(false),
            wake: Condvar::new(),
        })
    }

    /// Interrupt both directions without acquiring the serialization lock.
    pub fn close(&self) {
        *lock(&self.closed) = true;
        self.wake.notify_all();
        self.shutdown.shutdown();
    }

    /// Wait between liveness probes, ending immediately when this connection is retired.
    pub fn wait_closed(&self, interval: Duration) -> bool {
        let closed = lock(&self.closed);
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

    /// Send a native frame within a bounded total delivery budget.
    pub fn native<T: Serialize>(&self, value: &T) -> Result<(), FrameError> {
        self.native_until(value, Instant::now() + DELIVERY_TIMEOUT)
    }

    /// Send one frame whose payload is already bytes, within the bounded delivery budget.
    pub fn raw(&self, payload: &[u8]) -> Result<(), FrameError> {
        let result = self
            .until(Instant::now() + DELIVERY_TIMEOUT)
            .map_err(FrameError::from)
            .and_then(|mut writer| frame::write_frame_raw(&mut writer, payload));
        if result.is_err() {
            self.close();
        }
        result
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
            .and_then(|mut writer| frame::write_frame(&mut writer, value));
        if result.is_err() {
            self.close();
        }
        result
    }
}

/// A writer lease whose deadline covers every partial write, including chunked transfers.
pub struct SocketWrite<'a> {
    owner: &'a SocketWriter,
    stream: MutexGuard<'a, Connection>,
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
    use std::sync::mpsc;
    use std::thread;

    use super::*;
    use ghostlight_relay::endpoint::pair as connection_pair;
    use serde_json::{json, Value};

    /// The service end and the peer end of a connected pair.
    fn pair() -> (Connection, Connection) {
        connection_pair().unwrap()
    }

    /// ACCEPTANCE C17 ("no single call's deadline fails another call in flight") and ADR-0160: a
    /// call whose deadline passes while it waits for the writer gives up alone. The connection
    /// stays alive, so the holder of the writer still delivers whole frames, and closing it
    /// needs no writer lock: the peer reads the end at once while the writer is still held.
    #[test]
    fn caller_expiry_does_not_disconnect_another_writer_and_close_needs_no_lock() {
        let (stream, mut peer) = pair();
        let writer = SocketWriter::new(stream).unwrap();
        let mut held = writer
            .until(Instant::now() + Duration::from_secs(2))
            .unwrap();
        assert!(writer
            .until(Instant::now() + Duration::from_millis(30))
            .is_err());
        let sent = json!({"after": "the waiting call gave up"});
        frame::write_frame(&mut held, &sent).unwrap();
        let received: Option<Value> = frame::read_frame(&mut peer).unwrap();
        assert_eq!(
            received,
            Some(sent),
            "the waiting call's deadline left the connection alive"
        );

        let (closed, close_returned) = mpsc::channel();
        thread::scope(|scope| {
            scope.spawn(|| {
                writer.close();
                closed.send(()).unwrap();
            });
            assert!(
                close_returned.recv_timeout(Duration::from_secs(5)).is_ok(),
                "closing waited for the writer lock"
            );
        });
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
        assert!(writer.wait_closed(Duration::from_secs(2)));
        drop(held);
    }
}
