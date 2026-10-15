//! The words the workbench server says: who can see a view, why an act is not available here, and
//! how a refused request reads.
//!
//! The page asks the service what context it is in and renders what it is told. Every sentence a
//! request can end in lives here, so the server routes and decides and never words anything
//! itself (ADR-0199, amendment of 2026-10-15).

/// The word for the context that carries the right key: the Ghostlight window.
pub const WORD_CONTROL: &str = "Full control";

/// The word for the context of a browser tab while watching is on.
pub const WORD_WATCH: &str = "Watching";

/// The word for the context of a browser tab while watching is off.
pub const WORD_CLOSED: &str = "Watching is off";

/// What the window says while other screens can watch.
pub const SENTENCE_CONTROL_WATCHED: &str =
    "Others on this computer can watch this view in a browser.";

/// What the window says while only the window shows Ghostlight.
pub const SENTENCE_CONTROL_PRIVATE: &str = "Watching is off: only this window shows Ghostlight.";

/// What a watching tab says about itself.
pub const SENTENCE_WATCH: &str =
    "Anyone on this computer can open this view. Changes are made in the Ghostlight window.";

/// Why a view shows no data: watching is off.
pub const WATCHING_OFF: &str =
    "Watching is off on this computer. Turn it on in the Ghostlight window.";

/// Why an act is not available in a view that only watches.
pub const CHANGE_IN_WINDOW: &str = "Change this in the Ghostlight window.";

/// A request carried a key that is not this service start's key.
pub const WRONG_KEY: &str = "This window's key is out of date. Reopen the Ghostlight window.";

/// A view asked for more than it may in a short time.
pub const TOO_MANY: &str = "Too many requests from this view. Try again in a moment.";

/// The rules on disk changed after the draft was made.
pub const STALE_POLICY: &str = "Your rules changed in another window; your draft is kept.";

/// A client's setup changed after the view was drawn.
pub const STALE_HARNESS: &str = "This client changed since this view was drawn. Look again.";

/// A request the server could not read.
pub const BAD_REQUEST: &str = "Ghostlight could not read that request.";

/// A request body over the route's size.
pub const TOO_LARGE: &str = "That request is too large.";

/// A request whose body is not JSON.
pub const UNSUPPORTED_TYPE: &str = "Ghostlight reads requests sent as JSON.";

/// A request that came from another site.
pub const CROSS_SITE: &str = "That request came from another site, so Ghostlight refused it.";

/// An address that is not served.
pub const NOT_FOUND: &str = "There is nothing at this address.";

/// An address that does not take this kind of request.
pub const METHOD_NOT_ALLOWED: &str = "This address does not take that kind of request.";

/// A request addressed to a host name this view is not served under.
pub const MISDIRECTED: &str = "This view is served only at its own address.";

/// The license shown on the About page.
pub const PRODUCT_LICENSE: &str = "Apache-2.0 OR MIT";

/// Confirmation after copying a harness's MCP command.
pub const COPIED_COMMAND: &str = "MCP command copied.";

/// Confirmation after copying a harness's manual setup.
pub const COPIED_SETUP: &str = "Manual setup copied.";

/// Why the runtime is held, when the pause came from the Ghostlight window.
pub const PAUSED_IN_WINDOW: &str = "Paused in the Ghostlight window.";

/// Why the runtime is held, when the pause came from a screen that is watching.
pub const PAUSED_FROM_WATCHER: &str = "Paused from a screen that is watching.";

/// Why the runtime is held, when the pause came from the tray.
pub const PAUSED_FROM_TRAY: &str = "Paused from the tray.";

/// Why the runtime is held, when the pause came from the browser toolbar.
pub const PAUSED_FROM_BROWSER: &str = "Paused from the browser toolbar.";

/// What a screen is told when every place for a live view of its kind is taken.
pub const TOO_MANY_SCREENS: &str = "Too many screens are watching. Close one and try again.";

/// Why the diagnostics folder cannot be shown: diagnostics are off.
pub const DIAGNOSTICS_OFF: &str = "Diagnostics are off; turn them on first.";

/// A link the service would not open because it is not a secure address.
pub const LINK_NOT_SECURE: &str = "Ghostlight opens only secure links.";

/// The title of the dialog where a person points at a client's application or settings file.
pub const LOCATE_DIALOG_TITLE: &str = "Locate the harness application or settings file";

/// A search that is longer than the search box allows.
#[must_use]
pub fn search_too_long(limit: usize) -> String {
    format!("Search is limited to {limit} characters.")
}

/// A search that holds a character no search needs.
pub const SEARCH_CONTROL_CHARACTER: &str = "Search contains an unsupported control character.";

/// A policy document with nothing in it.
pub const POLICY_EMPTY: &str = "A policy document cannot be empty.";

/// A policy document longer than the window sends.
#[must_use]
pub fn policy_too_large(limit: usize) -> String {
    format!("A policy document is limited to {limit} bytes.")
}

/// How the policy page counts the sites marked never-touch for a view that may not name them.
#[must_use]
pub fn never_touch_count(count: usize) -> String {
    if count == 1 {
        "1 site is marked never-touch by policy.".to_owned()
    } else {
        format!("{count} sites are marked never-touch by policy.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sentence_is_plain_ascii_and_ends_a_sentence() {
        for sentence in [
            SENTENCE_CONTROL_WATCHED,
            SENTENCE_CONTROL_PRIVATE,
            SENTENCE_WATCH,
            WATCHING_OFF,
            CHANGE_IN_WINDOW,
            WRONG_KEY,
            TOO_MANY,
            STALE_POLICY,
            STALE_HARNESS,
            BAD_REQUEST,
            TOO_LARGE,
            UNSUPPORTED_TYPE,
            CROSS_SITE,
            NOT_FOUND,
            METHOD_NOT_ALLOWED,
            MISDIRECTED,
            PAUSED_IN_WINDOW,
            PAUSED_FROM_WATCHER,
            PAUSED_FROM_TRAY,
            PAUSED_FROM_BROWSER,
            TOO_MANY_SCREENS,
            COPIED_COMMAND,
            COPIED_SETUP,
            SEARCH_CONTROL_CHARACTER,
            POLICY_EMPTY,
            DIAGNOSTICS_OFF,
            LINK_NOT_SECURE,
        ] {
            assert!(sentence.is_ascii(), "{sentence}");
            assert!(sentence.ends_with('.'), "{sentence}");
        }
    }

    #[test]
    fn the_never_touch_count_reads_in_the_singular_and_the_plural() {
        assert_eq!(
            never_touch_count(1),
            "1 site is marked never-touch by policy."
        );
        assert_eq!(
            never_touch_count(3),
            "3 sites are marked never-touch by policy."
        );
    }
}
