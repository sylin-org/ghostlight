//! The webview's rules: where it may go, what its page is handed, and where it keeps web storage
//! (ADR-0199 Decision 7 and its amendment of 2026-10-15).
//!
//! The window shows one of two pages: the service's loopback workbench, or one of the shell's own
//! state screens (`states`). [`workbench`] builds the first: [`allows_navigation`] keeps the
//! webview on that origin, so a link the page wants opened elsewhere is a request to the service
//! (opener `service`), never a navigation, and [`access_script`] hands the page the bearer that
//! makes its requests the window's, and nothing else: the bearer exists only inside the workbench
//! origin's main frame. [`state_screen`] builds the second, which is handed nothing and may go
//! nowhere but to its own document. [`harden`] applies every webview setting that keeps the
//! window on its page and off everything else, in one place. [`profile_directory`] is the one
//! folder the webview writes web data to.

use std::io;
use std::path::{Path, PathBuf};

use wry::{NewWindowResponse, WebViewBuilder};

use crate::palette;
use crate::states::{self, Navigation, StateButton, StateScreen};

/// The name of the global the page reads its access from.
const ACCESS_GLOBAL: &str = "GhostlightAccess";

/// The directory under the Ghostlight home that holds machine-local state.
const STATE_DIRECTORY_NAME: &str = "state";

/// The directory under the state directory that holds the window's web storage.
const PROFILE_DIRECTORY_NAME: &str = "desktop-webview";

/// The window ground colour, the night ground, opaque, so a window never flashes white.
const GROUND: (u8, u8, u8, u8) = (
    palette::GROUND[0],
    palette::GROUND[1],
    palette::GROUND[2],
    u8::MAX,
);

/// Whether the webview may navigate to `url`: only the origin itself or a path under it.
///
/// `origin` is a scheme, host and port with no trailing slash. Another port, another host name
/// for the same address, user information and every other scheme are refused.
pub fn allows_navigation(origin: &str, url: &str) -> bool {
    url.strip_prefix(origin)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The script every main frame starts with.
///
/// When the page's origin is `origin` it defines `window.GhostlightAccess` as a frozen object
/// holding `bearer`, not writable and not configurable. In any other page it does nothing. Both
/// values are written as JSON string literals, so no quote, backslash or control character can
/// leave the literal. On Windows the webview adds initialization scripts to subframes whatever
/// the main-frame-only flag says, so the origin check inside the script is what keeps the bearer
/// to the workbench page; the workbench's content policy allows no frame at all.
pub fn access_script(origin: &str, bearer: &str) -> String {
    let origin = serde_json::to_string(origin).expect("a string is always JSON");
    let bearer = serde_json::to_string(bearer).expect("a string is always JSON");
    format!(
        "(function () {{ if (location.origin !== {origin}) return; \
         Object.defineProperty(window, \"{ACCESS_GLOBAL}\", {{ \
         value: Object.freeze({{ bearer: {bearer} }}), \
         writable: false, configurable: false, enumerable: true }}); }})();"
    )
}

/// Whether this computer has the web view runtime the window needs: WebView2 on Windows, and the
/// WebKitGTK the shell was linked against on Linux.
pub fn runtime_available() -> bool {
    wry::webview_version().is_ok()
}

/// The folder the window keeps web storage in: `<home>/state/desktop-webview`.
///
/// It holds only what the workbench page stores for its own loopback origin, which is why the
/// page's "On this screen" settings last across launches.
pub fn profile_directory(home: &Path) -> PathBuf {
    home.join(STATE_DIRECTORY_NAME).join(PROFILE_DIRECTORY_NAME)
}

/// Creates the profile folder and any missing parent, closed to everyone but the user.
pub fn prepare_profile(directory: &Path) -> io::Result<()> {
    ghostlight_relay::private::create_dir_all(directory)
}

/// Applies every webview setting that keeps the window on its page: new windows denied,
/// downloads refused, no zoom hotkeys, no autofill, the clipboard on so the page can copy text,
/// the night ground, and devtools only in debug builds. The caller adds the navigation rule. This
/// is checked live, not in `cargo test`.
pub fn harden(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    builder
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_devtools(cfg!(debug_assertions))
        .with_hotkeys_zoom(false)
        .with_clipboard(true)
        .with_general_autofill_enabled(false)
        .with_background_color(GROUND)
}

/// The builder of the workbench page: hardened, limited to `origin`, handed `bearer`, and loading
/// `start_url`. This is checked live, not in `cargo test`.
pub fn workbench<'a>(
    builder: WebViewBuilder<'a>,
    origin: &str,
    bearer: &str,
    start_url: String,
) -> WebViewBuilder<'a> {
    let navigable = origin.to_owned();
    harden(builder)
        .with_navigation_handler(move |url| allows_navigation(&navigable, &url))
        .with_initialization_script_for_main_only(access_script(origin, bearer), true)
        .with_url(start_url)
}

/// The builder of a state screen: hardened, handed nothing, loading the screen's document, and
/// limited by [`states::navigation`]. A press of a button is refused as a navigation and passed to
/// `pressed`. This is checked live, not in `cargo test`.
pub fn state_screen<'a>(
    builder: WebViewBuilder<'a>,
    screen: StateScreen,
    pressed: impl Fn(StateButton) + 'static,
) -> WebViewBuilder<'a> {
    harden(builder)
        .with_navigation_handler(move |url| match states::navigation(&url) {
            Navigation::Allow => true,
            Navigation::Refuse => false,
            Navigation::Press(button) => {
                pressed(button);
                false
            }
        })
        .with_html(states::document(screen))
}
