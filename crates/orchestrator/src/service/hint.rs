//! The one-time hint that explains the tray icon.
//!
//! The first time a person closes the window while the service runs, Ghostlight says once that it
//! keeps running and where its icon is. Once per home: the marker `state/tray-hint` records that
//! it was said. It is service-private state, outside what the frozen edge and extension read.

use std::io;
use std::path::Path;

use crate::install::record;

/// Record the first close at `marker`, and say whether this is the first.
///
/// The marker is written crash-safe and private to the user before the caller shows the hint, so
/// a crash after the write loses the hint and never repeats it.
pub fn first_close(marker: &Path) -> io::Result<bool> {
    if marker.exists() {
        return Ok(false);
    }
    record::atomic_replace(marker, b"")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::scratch::Scratch;

    #[test]
    fn only_the_first_close_is_the_first() {
        let scratch = Scratch::new();
        let marker = scratch.path().join("state").join("tray-hint");
        assert!(first_close(&marker).unwrap());
        assert!(marker.is_file());
        assert!(!first_close(&marker).unwrap());
        assert!(!first_close(&marker).unwrap());
    }
}
