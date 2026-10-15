//! The in-memory, content-addressed bundle of web files the shell serves.
//!
//! A bundle is the whole of what the window can show: pages the service sends, and the shell's own
//! pages under the reserved `_shell/` prefix. It lives in memory only; the shell never reads a page
//! from disk or the network. A bundle fails closed: a bad path, an unknown file type, a duplicate
//! or an oversize set is refused as a whole, never trimmed.
//!
//! A bundle is named by its [`Bundle::id`], the SHA-256 over every path and file digest in path
//! order. Equal file sets have equal ids whatever the order they were supplied in, and the service
//! computes the same id with the same rule, so the two sides can name a bundle without a counter.

use std::collections::BTreeMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use thiserror::Error;

/// The first path segment reserved for the shell's own pages.
pub const SHELL_PREFIX: &str = "_shell";

/// The most files one bundle may hold.
pub const MAX_FILES: usize = 256;

/// The most bytes one bundle may hold in all (one relay frame).
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// The longest path, in bytes.
pub const MAX_PATH_BYTES: usize = 128;

const TYPE_HTML: &str = "text/html; charset=utf-8";
const TYPE_CSS: &str = "text/css; charset=utf-8";
const TYPE_JS: &str = "text/javascript; charset=utf-8";
const TYPE_PNG: &str = "image/png";
const TYPE_SVG: &str = "image/svg+xml";
const TYPE_JSON: &str = "application/json";

/// The closed table of served file types, keyed by the file extension.
const CONTENT_TYPES: [(&str, &str); 6] = [
    ("html", TYPE_HTML),
    ("css", TYPE_CSS),
    ("js", TYPE_JS),
    ("png", TYPE_PNG),
    ("svg", TYPE_SVG),
    ("json", TYPE_JSON),
];

/// Why a set of files cannot become a [`Bundle`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundleError {
    /// The path is not a plain relative path of allowed characters within the length bound.
    #[error("invalid bundle path {path:?}")]
    InvalidPath {
        /// The refused path.
        path: String,
    },
    /// A service bundle used the first segment the shell reserves for itself.
    #[error("bundle path {path:?} is in the reserved {SHELL_PREFIX} prefix")]
    ReservedPath {
        /// The refused path.
        path: String,
    },
    /// A shell bundle held a path that is not under the reserved prefix.
    #[error("shell bundle path {path:?} is outside the {SHELL_PREFIX} prefix")]
    OutsideShellPrefix {
        /// The refused path.
        path: String,
    },
    /// The file extension is not in the table of served types.
    #[error("bundle path {path:?} has a file type the shell does not serve")]
    UnsupportedType {
        /// The refused path.
        path: String,
    },
    /// The same path was supplied twice.
    #[error("bundle path {path:?} appears twice")]
    Duplicate {
        /// The repeated path.
        path: String,
    },
    /// More than [`MAX_FILES`] files were supplied.
    #[error("a bundle holds at most {MAX_FILES} files")]
    TooManyFiles,
    /// More than [`MAX_BYTES`] bytes were supplied in all.
    #[error("a bundle holds at most {MAX_BYTES} bytes")]
    TooLarge,
}

/// One stored file: its served type, its bytes and the digest of those bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFile {
    /// The media type the scheme serves the file with.
    pub content_type: &'static str,
    /// The file content.
    pub bytes: Arc<[u8]>,
    /// The SHA-256 of the content, as lowercase hex.
    pub sha256: String,
}

/// A validated, immutable set of web files with a content-derived id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    files: BTreeMap<String, BundleFile>,
    id: String,
}

impl Bundle {
    /// Builds a bundle of pages the service sends. Any path under [`SHELL_PREFIX`] is refused.
    pub fn service(files: Vec<(String, Vec<u8>)>) -> Result<Bundle, BundleError> {
        Bundle::build(files, Origin::Service)
    }

    /// Builds the shell's own bundle. Every path must be under [`SHELL_PREFIX`].
    pub fn shell(files: Vec<(String, Vec<u8>)>) -> Result<Bundle, BundleError> {
        Bundle::build(files, Origin::Shell)
    }

    /// The bundle id: lowercase hex SHA-256 over `<path>\0<file sha256>\n` lines in path order.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The stored file at `path`, if the bundle holds one.
    pub fn get(&self, path: &str) -> Option<&BundleFile> {
        self.files.get(path)
    }

    /// Every stored path, in path order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    fn build(input: Vec<(String, Vec<u8>)>, origin: Origin) -> Result<Bundle, BundleError> {
        if input.len() > MAX_FILES {
            return Err(BundleError::TooManyFiles);
        }
        let total: usize = input
            .iter()
            .fold(0usize, |sum, (_, bytes)| sum.saturating_add(bytes.len()));
        if total > MAX_BYTES {
            return Err(BundleError::TooLarge);
        }
        let mut files = BTreeMap::new();
        for (path, bytes) in input {
            validate_path(&path)?;
            origin.check(&path)?;
            let content_type = content_type_of(&path)
                .ok_or_else(|| BundleError::UnsupportedType { path: path.clone() })?;
            if files.contains_key(&path) {
                return Err(BundleError::Duplicate { path });
            }
            let sha256 = hex(&Sha256::digest(&bytes));
            files.insert(
                path,
                BundleFile {
                    content_type,
                    bytes: Arc::from(bytes),
                    sha256,
                },
            );
        }
        let id = bundle_id(&files);
        Ok(Bundle { files, id })
    }
}

/// Which side a bundle is built for, which decides what its paths may be.
#[derive(Clone, Copy)]
enum Origin {
    Service,
    Shell,
}

impl Origin {
    fn check(self, path: &str) -> Result<(), BundleError> {
        let under_prefix = path
            .split_once('/')
            .is_some_and(|(first, _)| first == SHELL_PREFIX);
        match self {
            Origin::Service if path == SHELL_PREFIX || under_prefix => {
                Err(BundleError::ReservedPath {
                    path: path.to_string(),
                })
            }
            Origin::Shell if !under_prefix => Err(BundleError::OutsideShellPrefix {
                path: path.to_string(),
            }),
            _ => Ok(()),
        }
    }
}

/// Checks that `path` is a plain relative path: `/`-separated segments of ASCII letters, digits,
/// `.`, `_` and `-`, none empty, `.` or `..`, within [`MAX_PATH_BYTES`].
pub fn validate_path(path: &str) -> Result<(), BundleError> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    let valid = path.len() <= MAX_PATH_BYTES
        && path.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && segment.chars().all(allowed)
        });
    if valid {
        Ok(())
    } else {
        Err(BundleError::InvalidPath {
            path: path.to_string(),
        })
    }
}

/// The served media type for the extension of `path`, if it is in the closed table.
fn content_type_of(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next()?;
    let (_, extension) = name.rsplit_once('.')?;
    CONTENT_TYPES
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, content_type)| *content_type)
}

fn bundle_id(files: &BTreeMap<String, BundleFile>) -> String {
    let mut hasher = Sha256::new();
    for (path, file) in files {
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
        hasher.update(file.sha256.as_bytes());
        hasher.update(b"\n");
    }
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, body: &str) -> (String, Vec<u8>) {
        (path.to_string(), body.as_bytes().to_vec())
    }

    #[test]
    fn the_id_ignores_input_order() {
        let a = Bundle::service(vec![file("a.html", "one"), file("b/c.css", "two")]).unwrap();
        let b = Bundle::service(vec![file("b/c.css", "two"), file("a.html", "one")]).unwrap();
        assert_eq!(a.id(), b.id());
        assert_eq!(a.id().len(), 64);
        assert!(a
            .id()
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    #[test]
    fn the_id_is_sha256_over_path_and_digest_lines() {
        let bundle = Bundle::service(vec![file("a.html", "one")]).unwrap();
        let digest = hex(&Sha256::digest(b"one"));
        let expected = hex(&Sha256::digest(format!("a.html\0{digest}\n").as_bytes()));
        assert_eq!(bundle.id(), expected);
    }

    #[test]
    fn one_changed_byte_changes_the_file_digest_and_the_id() {
        let a = Bundle::service(vec![file("a.html", "one"), file("b.css", "two")]).unwrap();
        let b = Bundle::service(vec![file("a.html", "one"), file("b.css", "twp")]).unwrap();
        assert_eq!(
            a.get("a.html").unwrap().sha256,
            b.get("a.html").unwrap().sha256
        );
        assert_ne!(
            a.get("b.css").unwrap().sha256,
            b.get("b.css").unwrap().sha256
        );
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn each_extension_in_the_table_has_its_content_type() {
        let cases = [
            ("a.html", "text/html; charset=utf-8"),
            ("a.css", "text/css; charset=utf-8"),
            ("a.js", "text/javascript; charset=utf-8"),
            ("a.png", "image/png"),
            ("a.svg", "image/svg+xml"),
            ("a.json", "application/json"),
        ];
        let files = cases.iter().map(|(path, _)| file(path, "x")).collect();
        let bundle = Bundle::service(files).unwrap();
        for (path, content_type) in cases {
            assert_eq!(
                bundle.get(path).unwrap().content_type,
                content_type,
                "{path}"
            );
        }
    }

    #[test]
    fn an_unknown_or_missing_extension_is_refused() {
        for path in ["a.exe", "a.txt", "noext", "a.HTML", "dir.html/readme"] {
            assert_eq!(
                Bundle::service(vec![file(path, "x")]),
                Err(BundleError::UnsupportedType {
                    path: path.to_string()
                }),
                "{path}"
            );
        }
    }

    #[test]
    fn bad_paths_are_refused() {
        let long = format!("{}.html", "a".repeat(124));
        assert_eq!(long.len(), MAX_PATH_BYTES + 1);
        for path in [
            "../x.html",
            "/x.html",
            "a//b.html",
            "a\\b.html",
            "a/./b.html",
            "a/../b.html",
            "a/",
            "",
            "caf\u{e9}.html",
            "a b.html",
            long.as_str(),
        ] {
            assert_eq!(
                Bundle::service(vec![file(path, "x")]),
                Err(BundleError::InvalidPath {
                    path: path.to_string()
                }),
                "{path:?}"
            );
        }
    }

    #[test]
    fn a_path_of_exactly_the_longest_length_is_accepted() {
        let path = format!("{}.html", "a".repeat(123));
        assert_eq!(path.len(), MAX_PATH_BYTES);
        assert!(validate_path(&path).is_ok());
    }

    #[test]
    fn the_shell_prefix_is_reserved_for_the_shell_bundle() {
        assert_eq!(
            Bundle::service(vec![file("_shell/x.html", "x")]),
            Err(BundleError::ReservedPath {
                path: "_shell/x.html".to_string()
            })
        );
        assert!(Bundle::service(vec![file("_shells/x.html", "x")]).is_ok());
        assert!(Bundle::shell(vec![file("_shell/x.html", "x")]).is_ok());
        for path in ["x.html", "other/x.html", "_shells/x.html"] {
            assert_eq!(
                Bundle::shell(vec![file(path, "x")]),
                Err(BundleError::OutsideShellPrefix {
                    path: path.to_string()
                }),
                "{path}"
            );
        }
    }

    #[test]
    fn a_duplicate_path_is_refused() {
        assert_eq!(
            Bundle::service(vec![file("a.html", "one"), file("a.html", "two")]),
            Err(BundleError::Duplicate {
                path: "a.html".to_string()
            })
        );
    }

    #[test]
    fn the_file_count_and_total_size_are_bounded() {
        let many = (0..=MAX_FILES)
            .map(|n| file(&format!("f{n}.html"), "x"))
            .collect();
        assert_eq!(Bundle::service(many), Err(BundleError::TooManyFiles));
        let at_limit = (0..MAX_FILES)
            .map(|n| file(&format!("f{n}.html"), "x"))
            .collect();
        assert!(Bundle::service(at_limit).is_ok());

        let exact = vec![("a.html".to_string(), vec![0u8; MAX_BYTES])];
        assert!(Bundle::service(exact).is_ok());
        let over = vec![("a.html".to_string(), vec![0u8; MAX_BYTES + 1])];
        assert_eq!(Bundle::service(over), Err(BundleError::TooLarge));
        let split = vec![
            ("a.html".to_string(), vec![0u8; MAX_BYTES]),
            ("b.html".to_string(), vec![0u8; 1]),
        ];
        assert_eq!(Bundle::service(split), Err(BundleError::TooLarge));
    }

    #[test]
    fn get_returns_a_stored_file_and_none_for_an_absent_one() {
        let bundle = Bundle::service(vec![file("a/b.json", "{}")]).unwrap();
        let stored = bundle.get("a/b.json").unwrap();
        assert_eq!(&*stored.bytes, b"{}");
        assert_eq!(stored.content_type, "application/json");
        assert_eq!(stored.sha256, hex(&Sha256::digest(b"{}")));
        assert!(bundle.get("a/c.json").is_none());
        assert_eq!(bundle.paths().collect::<Vec<_>>(), vec!["a/b.json"]);
    }
}
