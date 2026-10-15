//! Windows named pipes for the relay endpoint: the stream, the listener, the client connect and
//! a connected pair.
//!
//! Every pipe handle is overlapped. A read or write issues its own `OVERLAPPED` with its own
//! event and waits for it up to the stream's timeout, so one handle serves a reader and a writer
//! on different threads, and a timeout or a shutdown cancels exactly the operation concerned.
//! The caller of each operation waits for the kernel to finish with its buffer before returning,
//! so no buffer is ever borrowed past the call.

use std::ffi::c_void;
use std::io::{self, Read, Write};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::token::{
    Event, Handle, Owned, SecurityDescriptor, Token, UserSid, INVALID_HANDLE_VALUE,
};

/// Open mode: the pipe carries data both ways.
const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;

/// Open mode: operations on the handle are overlapped.
const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;

/// Open mode: creation fails when any instance of the name already exists.
const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;

/// Pipe mode: a byte stream, read as a byte stream, with blocking waits, closed to remote
/// clients. (`PIPE_TYPE_BYTE`, `PIPE_READMODE_BYTE` and `PIPE_WAIT` are all zero.)
const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x0000_0008;

/// Instance count: as many as the system allows.
const PIPE_UNLIMITED_INSTANCES: u32 = 255;

/// Size of each direction's buffer: 64 KiB.
const BUFFER_BYTES: u32 = 64 * 1024;

/// Access right: read.
const GENERIC_READ: u32 = 0x8000_0000;

/// Access right: write.
const GENERIC_WRITE: u32 = 0x4000_0000;

/// Creation disposition: open only an existing pipe.
const OPEN_EXISTING: u32 = 3;

/// Client flag: the quality-of-service fields below are present.
const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;

/// Client flag: the server may learn who the client is but may not act as the client.
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;

/// The lowest integrity level the service admits: medium, which keeps sandboxed and
/// AppContainer processes out.
const SECURITY_MANDATORY_MEDIUM_RID: u32 = 0x2000;

const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_BROKEN_PIPE: i32 = 109;
const ERROR_SEM_TIMEOUT: i32 = 121;
const ERROR_PIPE_BUSY: i32 = 231;
const ERROR_NO_DATA: i32 = 232;
const ERROR_PIPE_NOT_CONNECTED: i32 = 233;
const ERROR_PIPE_CONNECTED: i32 = 535;
const ERROR_OPERATION_ABORTED: i32 = 995;
const ERROR_IO_PENDING: i32 = 997;

const WAIT_OBJECT_0: u32 = 0;

/// Wait without limit.
const INFINITE: u32 = 0xFFFF_FFFF;

/// The longest finite wait, in milliseconds.
const LONGEST_WAIT_MS: u64 = 0xFFFF_FFFE;

/// A stored timeout meaning "none".
const NO_TIMEOUT: u64 = u64::MAX;

/// The C `OVERLAPPED`.
#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}

impl Overlapped {
    fn with_event(event: Handle) -> Self {
        Self {
            internal: 0,
            internal_high: 0,
            offset: 0,
            offset_high: 0,
            event,
        }
    }
}

/// The C `SECURITY_ATTRIBUTES`.
#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateNamedPipeW(
        name: *const u16,
        open_mode: u32,
        pipe_mode: u32,
        maximum_instances: u32,
        out_buffer_bytes: u32,
        in_buffer_bytes: u32,
        default_timeout_ms: u32,
        attributes: *const SecurityAttributes,
    ) -> Handle;
    fn CreateFileW(
        name: *const u16,
        desired_access: u32,
        share_mode: u32,
        attributes: *const SecurityAttributes,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template: Handle,
    ) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlapped: *mut Overlapped) -> i32;
    fn DisconnectNamedPipe(pipe: Handle) -> i32;
    fn WaitNamedPipeW(name: *const u16, timeout_ms: u32) -> i32;
    fn GetNamedPipeClientProcessId(pipe: Handle, process_id: *mut u32) -> i32;
    fn GetNamedPipeServerProcessId(pipe: Handle, process_id: *mut u32) -> i32;
    fn ReadFile(
        file: Handle,
        buffer: *mut u8,
        bytes: u32,
        read: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn WriteFile(
        file: Handle,
        buffer: *const u8,
        bytes: u32,
        written: *mut u32,
        overlapped: *mut Overlapped,
    ) -> i32;
    fn GetOverlappedResult(
        file: Handle,
        overlapped: *mut Overlapped,
        transferred: *mut u32,
        wait: i32,
    ) -> i32;
    fn CancelIoEx(file: Handle, overlapped: *const Overlapped) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn WaitForMultipleObjects(
        count: u32,
        handles: *const Handle,
        wait_all: i32,
        milliseconds: u32,
    ) -> u32;
}

/// The text of the current user's security identifier, as a pipe name carries it.
pub fn current_user_sid() -> io::Result<String> {
    Token::of_current_process()?.user()?.text()
}

/// `name` as NUL-terminated UTF-16.
fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().chain(Some(0)).collect()
}

/// A wait in whole milliseconds for `limit`, rounded up so a short timeout never reads as zero.
fn wait_milliseconds(limit: Duration) -> u32 {
    let millis = limit.as_nanos().div_ceil(1_000_000);
    u32::try_from(millis.min(u128::from(LONGEST_WAIT_MS))).unwrap_or(INFINITE - 1)
}

/// Create one pipe instance. The first instance of a name sets `first`, so a name already served
/// is refused with [`io::ErrorKind::AddrInUse`].
fn create_instance(
    name: &[u16],
    descriptor: Option<&SecurityDescriptor>,
    first: bool,
) -> io::Result<Owned> {
    let attributes = descriptor.map(|descriptor| SecurityAttributes {
        length: size_of::<SecurityAttributes>() as u32,
        descriptor: descriptor.as_ptr(),
        inherit: 0,
    });
    let open_mode = PIPE_ACCESS_DUPLEX
        | FILE_FLAG_OVERLAPPED
        | if first {
            FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            0
        };
    // SAFETY: name is NUL-terminated and outlives the call; the attributes, when present, point
    // at a descriptor that outlives the call and inheritance is off.
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            open_mode,
            PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            BUFFER_BYTES,
            BUFFER_BYTES,
            0,
            attributes
                .as_ref()
                .map_or(null(), |attributes| attributes as *const SecurityAttributes),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        let failure = io::Error::last_os_error();
        return Err(match failure.raw_os_error() {
            Some(ERROR_ACCESS_DENIED | ERROR_PIPE_BUSY) if first => {
                io::Error::new(io::ErrorKind::AddrInUse, failure)
            }
            _ => failure,
        });
    }
    Ok(Owned(handle))
}

/// What a wait on a pipe instance ended with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Arrival {
    /// A client is connected to the instance.
    Client,
    /// A client connected and closed its end before the wait saw it. The instance still needs
    /// [`DisconnectNamedPipe`] before it can serve again.
    ClientGone,
}

/// Wait until a client connects to `pipe`, or until `stop` is signalled.
///
/// A client that has already come and gone is reported as [`Arrival::ClientGone`], not as a
/// failure. A stop cancels the wait and returns [`io::ErrorKind::ConnectionAborted`].
fn wait_for_client(pipe: &Owned, stop: Option<&Event>) -> io::Result<Arrival> {
    if stop.is_some_and(is_signalled) {
        return Err(io::ErrorKind::ConnectionAborted.into());
    }
    let event = Event::manual_reset()?;
    let mut overlapped = Overlapped::with_event(event.raw());
    // SAFETY: pipe is a valid overlapped server handle, and overlapped lives until the
    // operation is finished: every path below waits for it to complete or cancels it and waits.
    let connected = unsafe { ConnectNamedPipe(pipe.0, &mut overlapped) } != 0;
    if connected {
        return Ok(Arrival::Client);
    }
    let failure = io::Error::last_os_error();
    match failure.raw_os_error() {
        Some(ERROR_PIPE_CONNECTED) => return Ok(Arrival::Client),
        Some(ERROR_NO_DATA) => return Ok(Arrival::ClientGone),
        Some(ERROR_IO_PENDING) => {}
        _ => return Err(failure),
    }
    let handles = [event.raw(), stop.map_or(event.raw(), Event::raw)];
    let count = if stop.is_some() { 2 } else { 1 };
    // SAFETY: handles holds `count` valid handles for the call.
    let woken = unsafe { WaitForMultipleObjects(count, handles.as_ptr(), 0, INFINITE) };
    if woken != WAIT_OBJECT_0 {
        // SAFETY: the operation is still pending on pipe with this overlapped; cancelling it
        // and waiting for the result lets the overlapped go out of scope.
        unsafe { CancelIoEx(pipe.0, &overlapped) };
        let _ = finish(pipe.0, &mut overlapped);
        return Err(if woken == WAIT_OBJECT_0 + 1 {
            io::ErrorKind::ConnectionAborted.into()
        } else {
            io::Error::last_os_error()
        });
    }
    match finish(pipe.0, &mut overlapped) {
        Ok(_) => Ok(Arrival::Client),
        Err(failure) if failure.raw_os_error() == Some(ERROR_NO_DATA) => Ok(Arrival::ClientGone),
        Err(failure) => Err(failure),
    }
}

/// Drop the client of the connected server instance `pipe`, so the instance can serve again.
fn disconnect(pipe: &Owned) {
    // SAFETY: pipe is a valid server handle. The call fails harmlessly when no client is
    // connected.
    unsafe { DisconnectNamedPipe(pipe.0) };
}

/// Whether `event` is signalled right now.
fn is_signalled(event: &Event) -> bool {
    // SAFETY: the event handle is valid and a zero wait never blocks.
    unsafe { WaitForSingleObject(event.raw(), 0) == WAIT_OBJECT_0 }
}

/// Wait for the operation `overlapped` on `file` to finish and return its byte count.
fn finish(file: Handle, overlapped: &mut Overlapped) -> io::Result<u32> {
    let mut transferred = 0;
    // SAFETY: the overlapped belongs to an operation issued on file, and waiting makes the call
    // return only when the kernel is done with it.
    let ok = unsafe { GetOverlappedResult(file, overlapped, &mut transferred, 1) };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(transferred)
    }
}

/// State shared by every clone of a stream.
struct Shared {
    /// The handle, until [`PipeStream::shutdown`] closes it. Operations hold a read lock for
    /// their whole duration, so the handle is never closed under one.
    handle: RwLock<Option<Owned>>,
    closed: AtomicBool,
    read_timeout_ms: AtomicU64,
    write_timeout_ms: AtomicU64,
}

/// Which way one transfer goes.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Direction {
    Read,
    Write,
}

/// A connected named pipe end, one type for the server and the client.
///
/// Clones share the handle, the closed state and the timeouts, as clones of a socket do.
#[derive(Clone)]
pub struct PipeStream {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for PipeStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PipeStream").finish_non_exhaustive()
    }
}

impl PipeStream {
    fn new(handle: Owned) -> Self {
        Self {
            shared: Arc::new(Shared {
                handle: RwLock::new(Some(handle)),
                closed: AtomicBool::new(false),
                read_timeout_ms: AtomicU64::new(NO_TIMEOUT),
                write_timeout_ms: AtomicU64::new(NO_TIMEOUT),
            }),
        }
    }

    /// Another handle to the same end.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(self.clone())
    }

    /// Limit how long one read waits, or `None` to wait without limit. Zero is refused.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        store_timeout(&self.shared.read_timeout_ms, timeout)
    }

    /// Limit how long one write waits, or `None` to wait without limit. Zero is refused.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        store_timeout(&self.shared.write_timeout_ms, timeout)
    }

    /// End this stream for every clone: pending and later reads return end of file, writes
    /// fail with [`io::ErrorKind::BrokenPipe`], and the peer sees the end of the stream.
    /// Callable from any thread.
    pub fn shutdown(&self) {
        self.shared.closed.store(true, Ordering::SeqCst);
        if let Some(handle) = self
            .shared
            .handle
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            // SAFETY: the handle is valid under the read lock. A null overlapped cancels every
            // pending operation on it, from any thread.
            unsafe { CancelIoEx(handle.0, null()) };
        }
        // Every operation is cancelled or checks the closed flag after it starts, so the write
        // lock is granted promptly and the close waits for no one.
        self.shared
            .handle
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }

    /// Read some bytes; `Ok(0)` is the end of the stream.
    fn read_some(&self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = load_timeout(&self.shared.read_timeout_ms);
        self.transfer(Direction::Read, buffer.as_mut_ptr(), buffer.len(), limit)
    }

    /// Write some bytes and return how many.
    fn write_some(&self, buffer: &[u8]) -> io::Result<usize> {
        let limit = load_timeout(&self.shared.write_timeout_ms);
        self.transfer(
            Direction::Write,
            buffer.as_ptr().cast_mut(),
            buffer.len(),
            limit,
        )
    }

    /// Run one overlapped read or write to completion, timeout or cancellation.
    fn transfer(
        &self,
        direction: Direction,
        buffer: *mut u8,
        length: usize,
        limit: u32,
    ) -> io::Result<usize> {
        if length == 0 {
            return Ok(0);
        }
        if self.shared.closed.load(Ordering::SeqCst) {
            return self.ended(direction, None);
        }
        let length = u32::try_from(length).unwrap_or(u32::MAX);
        let guard = self.shared.handle.read().unwrap_or_else(|e| e.into_inner());
        let Some(file) = guard.as_ref().map(|handle| handle.0) else {
            return self.ended(direction, None);
        };
        let event = Event::manual_reset()?;
        let mut overlapped = Overlapped::with_event(event.raw());
        // SAFETY: file is a valid overlapped handle. The buffer is valid for `length` bytes for
        // the caller's whole call, and this function returns only after the kernel is finished
        // with the buffer and the overlapped: it waits for the result on every path.
        let started = unsafe {
            match direction {
                Direction::Read => ReadFile(file, buffer, length, null_mut(), &mut overlapped),
                Direction::Write => WriteFile(file, buffer, length, null_mut(), &mut overlapped),
            }
        } != 0;
        if !started {
            let failure = io::Error::last_os_error();
            if failure.raw_os_error() != Some(ERROR_IO_PENDING) {
                return self.ended(direction, Some(failure));
            }
            if self.shared.closed.load(Ordering::SeqCst) {
                // SAFETY: the operation is pending on file with this overlapped.
                unsafe { CancelIoEx(file, &overlapped) };
            } else {
                // SAFETY: event is a valid handle for the wait.
                let woken = unsafe { WaitForSingleObject(event.raw(), limit) };
                if woken != WAIT_OBJECT_0 {
                    // SAFETY: the operation is still pending with this overlapped.
                    unsafe { CancelIoEx(file, &overlapped) };
                }
            }
        }
        match finish(file, &mut overlapped) {
            Ok(transferred) => Ok(transferred as usize),
            Err(failure) => self.ended(direction, Some(failure)),
        }
    }

    /// The result of a transfer that did not move bytes, from the failure that stopped it.
    fn ended(&self, direction: Direction, failure: Option<io::Error>) -> io::Result<usize> {
        let closed = self.shared.closed.load(Ordering::SeqCst);
        let code = failure.as_ref().and_then(io::Error::raw_os_error);
        let gone = matches!(
            code,
            Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED | ERROR_NO_DATA)
        );
        match direction {
            Direction::Read if closed || gone => Ok(0),
            Direction::Write if closed || gone => Err(io::ErrorKind::BrokenPipe.into()),
            _ if code == Some(ERROR_OPERATION_ABORTED) => Err(io::ErrorKind::TimedOut.into()),
            _ => Err(failure.unwrap_or_else(|| io::ErrorKind::BrokenPipe.into())),
        }
    }
}

/// Store a timeout as whole milliseconds, refusing zero as a socket does.
fn store_timeout(slot: &AtomicU64, timeout: Option<Duration>) -> io::Result<()> {
    let stored = match timeout {
        None => NO_TIMEOUT,
        Some(limit) if limit.is_zero() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot set a zero duration timeout",
            ));
        }
        Some(limit) => u64::from(wait_milliseconds(limit)),
    };
    slot.store(stored, Ordering::SeqCst);
    Ok(())
}

/// The stored timeout as a wait in milliseconds.
fn load_timeout(slot: &AtomicU64) -> u32 {
    match slot.load(Ordering::SeqCst) {
        NO_TIMEOUT => INFINITE,
        millis => u32::try_from(millis).unwrap_or(INFINITE - 1),
    }
}

impl Read for PipeStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_some(buffer)
    }
}

impl Read for &PipeStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read_some(buffer)
    }
}

impl Write for PipeStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.write_some(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for &PipeStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.write_some(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The server side of the endpoint: the pipe name, served to one user.
pub struct PipeListener {
    name: Vec<u16>,
    descriptor: SecurityDescriptor,
    user: UserSid,
    stop: Event,
    waiting: Mutex<Option<Owned>>,
}

impl std::fmt::Debug for PipeListener {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PipeListener")
            .finish_non_exhaustive()
    }
}

impl PipeListener {
    /// Create the first instance of the pipe `name`, open to the current user alone.
    ///
    /// A name that already has an instance yields [`io::ErrorKind::AddrInUse`].
    pub fn bind_first(name: &str) -> io::Result<Self> {
        let user = Token::of_current_process()?.user()?;
        let descriptor = SecurityDescriptor::for_user_only(&user.text()?)?;
        let name = wide(name);
        let first = create_instance(&name, Some(&descriptor), true)?;
        Ok(Self {
            name,
            descriptor,
            user,
            stop: Event::manual_reset()?,
            waiting: Mutex::new(Some(first)),
        })
    }

    /// Wait for the next client that passes the check, and return its end.
    ///
    /// The pipe name is never left without an instance: when this returns an error, an instance
    /// is still waiting. A client that connects and closes before it is accepted is skipped. A
    /// client is admitted when its user is this process's user and its integrity level is at
    /// least medium. Any other client is disconnected before a byte is exchanged and the wait
    /// goes on. After [`PipeListener::shutdown`] this returns
    /// [`io::ErrorKind::ConnectionAborted`].
    pub fn accept(&self) -> io::Result<PipeStream> {
        let mut waiting = self.waiting.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            let pipe = match waiting.take() {
                Some(pipe) => pipe,
                None => create_instance(&self.name, Some(&self.descriptor), false)?,
            };
            match wait_for_client(&pipe, Some(&self.stop)) {
                Ok(Arrival::Client) => {}
                Ok(Arrival::ClientGone) => {
                    disconnect(&pipe);
                    *waiting = Some(pipe);
                    continue;
                }
                Err(failure) => {
                    *waiting = Some(pipe);
                    return Err(failure);
                }
            }
            match create_instance(&self.name, Some(&self.descriptor), false) {
                Ok(next) => *waiting = Some(next),
                Err(failure) => {
                    disconnect(&pipe);
                    *waiting = Some(pipe);
                    return Err(failure);
                }
            }
            if self.client_is_admitted(&pipe) {
                return Ok(PipeStream::new(pipe));
            }
            disconnect(&pipe);
        }
    }

    /// Wake an [`accept`](PipeListener::accept) blocked on another thread, and fail every later
    /// one.
    pub fn shutdown(&self) {
        self.stop.set();
    }

    /// Whether the process on the other end of `pipe` is the same user at medium integrity or
    /// above.
    fn client_is_admitted(&self, pipe: &Owned) -> bool {
        let mut process_id = 0;
        // SAFETY: pipe is a valid connected server handle and process_id is writable.
        if unsafe { GetNamedPipeClientProcessId(pipe.0, &mut process_id) } == 0 {
            return false;
        }
        let Ok(token) = Token::of_process(process_id) else {
            return false;
        };
        let same_user = token
            .user()
            .is_ok_and(|client| client.same_user(&self.user));
        same_user
            && token
                .integrity_rid()
                .is_ok_and(|rid| rid >= SECURITY_MANDATORY_MEDIUM_RID)
    }
}

/// Why [`connect_pipe`] did not return a stream.
#[derive(Debug)]
pub enum PipeConnectError {
    /// No pipe of that name exists.
    NoService,
    /// A pipe of that name exists but its server is not this user's process.
    Foreign,
    /// The server end closed while the connection was being checked.
    Closed,
    /// The deadline passed while every instance was busy.
    Deadline,
    /// Any other operating-system failure.
    Io(io::Error),
}

impl From<io::Error> for PipeConnectError {
    fn from(failure: io::Error) -> Self {
        Self::Io(failure)
    }
}

/// Open the client end of the pipe `name`, and check that its server runs as this user.
///
/// While every instance is busy the call waits until `deadline`. The server may learn who the
/// client is but may not act as the client.
pub fn connect_pipe(name: &str, deadline: Instant) -> Result<PipeStream, PipeConnectError> {
    let me = Token::of_current_process()?.user()?;
    let name = wide(name);
    let pipe = open_client(&name, deadline)?;
    let mut process_id = 0;
    // SAFETY: pipe is a valid client handle and process_id is writable.
    if unsafe { GetNamedPipeServerProcessId(pipe.0, &mut process_id) } == 0 {
        let failure = io::Error::last_os_error();
        return Err(match failure.raw_os_error() {
            Some(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) => PipeConnectError::Closed,
            _ => PipeConnectError::Io(failure),
        });
    }
    let server = match Token::of_process(process_id).and_then(|token| token.user()) {
        Ok(server) => server,
        Err(failure) if failure.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
            return Err(PipeConnectError::Foreign);
        }
        Err(failure) => return Err(PipeConnectError::Io(failure)),
    };
    if !server.same_user(&me) {
        return Err(PipeConnectError::Foreign);
    }
    Ok(PipeStream::new(pipe))
}

/// Open the pipe `name` as a client, waiting for a free instance until `deadline`.
fn open_client(name: &[u16], deadline: Instant) -> Result<Owned, PipeConnectError> {
    loop {
        let attributes = SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION;
        // SAFETY: name is NUL-terminated and outlives the call; no security attributes or
        // template, and exclusive sharing.
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | attributes,
                null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            return Ok(Owned(handle));
        }
        let failure = io::Error::last_os_error();
        match failure.raw_os_error() {
            Some(ERROR_FILE_NOT_FOUND) => return Err(PipeConnectError::NoService),
            Some(ERROR_ACCESS_DENIED) => return Err(PipeConnectError::Foreign),
            Some(ERROR_PIPE_BUSY) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(PipeConnectError::Deadline);
                }
                // SAFETY: name is NUL-terminated and outlives the call.
                let available = unsafe { WaitNamedPipeW(name.as_ptr(), wait_milliseconds(left)) };
                if available == 0
                    && io::Error::last_os_error().raw_os_error() != Some(ERROR_SEM_TIMEOUT)
                {
                    return Err(PipeConnectError::Io(io::Error::last_os_error()));
                }
            }
            _ => return Err(PipeConnectError::Io(failure)),
        }
    }
}

/// Two connected ends of a pipe with a name used once, for tests. No peer is checked.
pub fn pipe_pair() -> io::Result<(PipeStream, PipeStream)> {
    use std::sync::atomic::AtomicU32;
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        r"\\.\pipe\ghostlight-pair-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    );
    let name = wide(&name);
    let server = create_instance(&name, None, true)?;
    let client = match open_client(&name, Instant::now() + Duration::from_secs(5)) {
        Ok(client) => client,
        Err(PipeConnectError::Io(failure)) => return Err(failure),
        Err(_) => return Err(io::ErrorKind::NotFound.into()),
    };
    if wait_for_client(&server, None)? == Arrival::ClientGone {
        return Err(io::ErrorKind::BrokenPipe.into());
    }
    Ok((PipeStream::new(server), PipeStream::new(client)))
}
