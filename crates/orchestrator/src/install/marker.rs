//! The development marker: which executable stands in for the installed service.
//!
//! `home/development` holds the path of a development build's `ghostlight`. While it exists,
//! package invocations forward to that executable and it may serve in place of
//! `bin/ghostlight` (ADR-0197 Decisions 10, 13 and 15).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::record::atomic_replace;

/// The contents of the development marker.
///
/// Readers ignore unknown fields.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DevelopmentMarker {
    /// The development build's service executable.
    pub executable: PathBuf,
}

impl DevelopmentMarker {
    /// Read the marker at `path`.
    ///
    /// A missing file and a file that does not hold a marker both read as `None`, so a damaged
    /// marker never blocks a start. Only a failure to read the file is an error.
    pub fn read(path: &Path) -> io::Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Write the marker to `path` with [`atomic_replace`].
    pub fn write(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        atomic_replace(path, &bytes)
    }

    /// Remove the marker at `path`. A marker that is already gone is success.
    pub fn remove(path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::scratch::Scratch;
    use super::*;

    #[test]
    fn a_marker_round_trips_and_is_removed() {
        let scratch = Scratch::new();
        let path = scratch.path().join("development");
        assert_eq!(DevelopmentMarker::read(&path).unwrap(), None);
        let marker = DevelopmentMarker {
            executable: scratch.path().join("dev").join("ghostlight"),
        };
        marker.write(&path).unwrap();
        assert_eq!(DevelopmentMarker::read(&path).unwrap(), Some(marker));
        DevelopmentMarker::remove(&path).unwrap();
        assert_eq!(DevelopmentMarker::read(&path).unwrap(), None);
        DevelopmentMarker::remove(&path).unwrap();
    }

    #[test]
    fn unknown_fields_are_ignored_and_junk_reads_as_none() {
        let scratch = Scratch::new();
        let path = scratch.path().join("development");
        fs::write(
            &path,
            br#"{"executable":"/dev/ghostlight","source_tree":"/src"}"#,
        )
        .unwrap();
        assert_eq!(
            DevelopmentMarker::read(&path).unwrap(),
            Some(DevelopmentMarker {
                executable: PathBuf::from("/dev/ghostlight")
            })
        );
        fs::write(&path, b"junk").unwrap();
        assert_eq!(DevelopmentMarker::read(&path).unwrap(), None);
    }
}
