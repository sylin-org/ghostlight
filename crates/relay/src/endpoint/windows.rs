//! The Windows endpoint: a named pipe, through [`crate::os`].

use std::io;
use std::path::Path;
use std::time::Instant;

use super::ConnectError;
use crate::os::{self, PipeConnectError};

pub(super) use os::PipeListener as Listener;
pub(super) use os::PipeStream as Stream;

/// The pipe name of the current user and `home`.
fn current_pipe_name(home: &Path) -> io::Result<String> {
    Ok(super::pipe_name(&os::current_user_sid()?, home))
}

/// Create the first pipe instance for `home`.
pub(super) fn bind_first(home: &Path) -> io::Result<Listener> {
    Listener::bind_first(&current_pipe_name(home)?)
}

/// Connect to the pipe of `home`.
pub(super) fn connect(home: &Path, deadline: Instant) -> Result<Stream, ConnectError> {
    os::connect_pipe(&current_pipe_name(home)?, deadline).map_err(|failure| match failure {
        PipeConnectError::NoService => ConnectError::NoService,
        PipeConnectError::Foreign => ConnectError::ForeignServer,
        PipeConnectError::Closed => ConnectError::Closed,
        PipeConnectError::Deadline => ConnectError::Deadline,
        PipeConnectError::Io(failure) => ConnectError::Io(failure),
    })
}

/// Two connected ends over a pipe with a unique name.
pub(super) fn pair() -> io::Result<(Stream, Stream)> {
    os::pipe_pair()
}
