//! The one tolerant JSON file reader behind the runtime file and the failure templates.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde_json::Value;

/// Read a small JSON document the service published.
///
/// A missing file, a file over `limit` bytes and a file that is not JSON all read as `None`: a
/// reader treats them as "no service has published here". Any other I/O failure is returned.
pub(crate) fn read_json(path: &Path, limit: u64) -> io::Result<Option<Value>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Ok(None);
    }
    Ok(serde_json::from_slice(&bytes).ok())
}
