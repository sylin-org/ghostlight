//! The handles and ids Ghostlight mints and an agent passes back: each kind's prefix and the
//! shape of what follows it, named once for the decoder, the catalog, the sentences that must
//! not repeat a handle from page text, and the places that mint them.

/// How many lowercase base32 characters follow the prefix of a handle the workspace mints.
pub const WORKSPACE_SUFFIX_CHARS: usize = 8;
/// How many lowercase hex digits follow the prefix of an invocation or browser id.
const UUID_SUFFIX_CHARS: usize = 32;
/// How many lowercase hex digits end a `diag_<counter>_<check>` cursor.
const CURSOR_CHECK_CHARS: usize = 8;

/// One kind of handle or id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandleKind {
    /// A tab bound to a workspace.
    Tab,
    /// A control or element on a page.
    Target,
    /// A screenshot's coordinate space.
    View,
    /// A captured image an upload may reuse.
    Image,
    /// A recorded document tree.
    Snapshot,
    /// A connected browser, minted by the extension.
    Browser,
    /// A diagnostics cursor, minted by the extension.
    Diagnostics,
    /// One request's id.
    Invocation,
}

impl HandleKind {
    /// The kinds page text may not pose as: a sentence that quotes page text replaces any word
    /// shaped like one. Browser ids stay out, because their prefix is also how tool names begin.
    pub const IN_PAGE_TEXT: [Self; 6] = [
        Self::Target,
        Self::Tab,
        Self::View,
        Self::Diagnostics,
        Self::Image,
        Self::Invocation,
    ];

    /// The prefix every handle of this kind starts with.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Tab => "tab_",
            Self::Target => "target_",
            Self::View => "view_",
            Self::Image => "image_",
            Self::Snapshot => "snapshot_",
            Self::Browser => "browser_",
            Self::Diagnostics => "diag_",
            Self::Invocation => "invocation_",
        }
    }

    /// Whether `text` is shaped exactly as Ghostlight mints a handle of this kind.
    #[must_use]
    pub fn minted(self, text: &str) -> bool {
        let Some(suffix) = text.strip_prefix(self.prefix()) else {
            return false;
        };
        match self {
            Self::Tab | Self::Target | Self::View | Self::Image | Self::Snapshot => {
                suffix.len() == WORKSPACE_SUFFIX_CHARS
                    && suffix
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c))
            }
            Self::Browser | Self::Invocation => is_lower_hex(suffix, UUID_SUFFIX_CHARS),
            Self::Diagnostics => suffix.split_once('_').is_some_and(|(counter, check)| {
                !counter.is_empty()
                    && counter
                        .chars()
                        .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
                    && is_lower_hex(check, CURSOR_CHECK_CHARS)
            }),
        }
    }
}

fn is_lower_hex(text: &str, chars: usize) -> bool {
    text.len() == chars
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::HandleKind;

    /// Contract between the extension and the service: the extension mints browser ids
    /// (`extension/lib/state.js`) and diagnostics cursors (`extension/lib/diagnostics.js`), so
    /// their prefixes cross a language boundary, and the service's kinds must read what the
    /// extension writes.
    #[test]
    fn the_extension_mints_the_prefixes_the_service_reads() {
        let read = |path: &str| {
            std::fs::read_to_string(crate::language::self_check::golden::repository_path(path))
                .expect("the extension source exists")
        };
        let state = read("extension/lib/state.js");
        assert!(state.contains(&format!(
            "`{}${{randomUuid().replaceAll(\"-\", \"\")}}`",
            HandleKind::Browser.prefix()
        )));
        let diagnostics = read("extension/lib/diagnostics.js");
        assert!(diagnostics.contains(&format!(
            "/^{}([0-9a-z]+)_([0-9a-f]{{8}})$/",
            HandleKind::Diagnostics.prefix()
        )));
    }
}
