//! The window and its webview (ADR-0199 Decision 7; behavior ADR-0119 and ADR-0120).
//!
//! The shell has at most one window. Closing it destroys the window and leaves the process
//! running; only the service's Quit ends the process. [`WindowLifecycle`] is the pure rule set
//! that decides what to do with the window when something happens to it. [`ShellWindow`] is the
//! thin native glue that creates the `tao` window and its `wry` webview and carries out the
//! lifecycle's decisions.
//!
//! Linux keeps one extra rule. Wayland gives a client no way to unminimize its own window, so on
//! Linux an Open request destroys the existing window and builds a fresh one after the destroyed
//! event, and further Open requests while that replacement is pending are coalesced. A Linux
//! renderer crash (WebKitGTK `web-process-terminated`) discards only that window and nothing
//! rebuilds it by itself; the next Open does.
//!
//! The Linux startup helpers run in this order before the event loop starts:
//! [`apply_renderer_policy`], [`prepare_identity`], [`init_display`]. On Windows each is a no-op.
//!
//! Everything but the native glue is pure and unit tested. The glue is checked live.

use std::cell::RefCell;
use std::ffi::OsStr;
use std::rc::Rc;

use serde_json::Value;
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Icon, Window, WindowBuilder, WindowId};
use wry::{WebView, WebViewBuilder};

use crate::bridge::{delivery_script, BRIDGE_SCRIPT};
use crate::bundle::BundleSet;
use crate::scheme::{harden, page_url, respond, SCHEME};
use crate::tray::icon_image;

/// The window title, the product name.
pub const WINDOW_TITLE: &str = "Ghostlight";

/// The window's inner size in logical pixels, width then height.
pub const WINDOW_SIZE: (f64, f64) = (1180.0, 760.0);

/// The smallest inner size the window can be resized to, in logical pixels.
pub const WINDOW_MIN_SIZE: (f64, f64) = (880.0, 620.0);

/// The Windows window class name, which journeys use to find the window.
pub const WINDOW_CLASS: &str = "Ghostlight Window";

/// The application id. On Linux it is the Wayland app id and the X11 class (with the `.desktop`
/// suffix, the Applications entry's file name), so the shell groups under its installed entry.
pub const APPLICATION_ID: &str = "org.sylin.ghostlight";

/// The environment variable that turns off the WebKitGTK DMA-BUF renderer.
const RENDERER_VARIABLE: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// The value that turns it off.
const RENDERER_VALUE: &str = "1";

/// Paths whose presence means the proprietary NVIDIA driver is loaded.
#[cfg(target_os = "linux")]
const NVIDIA_PATHS: [&str; 2] = ["/sys/module/nvidia", "/proc/driver/nvidia/version"];

/// The taskbar icon on Windows, the product icon at 128 pixels.
#[cfg(windows)]
const TASKBAR_ICON_PNG: &[u8] = include_bytes!("../../../extension/icons/icon128.png");

/// Why the window could not be prepared or built.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WindowError {
    /// The session has no display, so no window can ever be shown.
    #[error("no display is available: {detail}")]
    NoDisplay {
        /// What the toolkit reported.
        detail: String,
    },
    /// The operating system or the webview refused an operation.
    #[error("window failed: {detail}")]
    Native {
        /// What the toolkit reported.
        detail: String,
    },
}

fn native(error: impl std::fmt::Display) -> WindowError {
    WindowError::Native {
        detail: error.to_string(),
    }
}

/// Whether the shell has a window, and whether it is being replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// There is no window.
    Absent,
    /// There is a window.
    Open,
    /// The window is being destroyed so a fresh one replaces it (Linux only).
    Replacing,
}

/// Something that happened to the window or was asked of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// The service or the tray asked to show the window.
    OpenRequested,
    /// The user closed the window.
    CloseRequested,
    /// The window has been destroyed.
    Destroyed,
    /// The webview's renderer process ended.
    RendererLost,
}

/// What the shell must do to the window now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAct {
    /// Build a new window and show it.
    Create,
    /// Restore, show and focus the existing window.
    Focus,
    /// Destroy the window. [`LifecycleEvent::Destroyed`] follows once it is gone.
    Destroy,
}

/// The pure rules for one window.
///
/// Feed it every [`LifecycleEvent`] and carry out the [`WindowAct`] it returns. If a
/// [`WindowAct::Create`] fails, feed [`LifecycleEvent::Destroyed`] so the state returns to
/// [`WindowState::Absent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowLifecycle {
    state: WindowState,
    rebuild_on_open: bool,
}

impl WindowLifecycle {
    /// A lifecycle with no window. With `rebuild_on_open` an Open request replaces an existing
    /// window instead of focusing it.
    pub fn new(rebuild_on_open: bool) -> WindowLifecycle {
        WindowLifecycle {
            state: WindowState::Absent,
            rebuild_on_open,
        }
    }

    /// The lifecycle of this platform: replace on Open on Linux, focus on Windows.
    pub fn for_platform() -> WindowLifecycle {
        WindowLifecycle::new(cfg!(target_os = "linux"))
    }

    /// The current state.
    pub fn state(&self) -> WindowState {
        self.state
    }

    /// Whether a window exists and is not being replaced.
    pub fn is_open(&self) -> bool {
        self.state == WindowState::Open
    }

    /// Applies one event and returns what to do about it, if anything.
    pub fn on(&mut self, event: LifecycleEvent) -> Option<WindowAct> {
        match (self.state, event) {
            (WindowState::Absent, LifecycleEvent::OpenRequested) => {
                self.state = WindowState::Open;
                Some(WindowAct::Create)
            }
            (WindowState::Open, LifecycleEvent::OpenRequested) => {
                if self.rebuild_on_open {
                    self.state = WindowState::Replacing;
                    Some(WindowAct::Destroy)
                } else {
                    Some(WindowAct::Focus)
                }
            }
            (WindowState::Open, LifecycleEvent::CloseRequested | LifecycleEvent::RendererLost) => {
                Some(WindowAct::Destroy)
            }
            (WindowState::Open, LifecycleEvent::Destroyed) => {
                self.state = WindowState::Absent;
                None
            }
            (WindowState::Replacing, LifecycleEvent::Destroyed) => {
                self.state = WindowState::Open;
                Some(WindowAct::Create)
            }
            _ => None,
        }
    }
}

/// The environment variable to set before GTK starts, if any.
///
/// WebKitGTK's DMA-BUF renderer is unreliable with the proprietary NVIDIA stack, so with that
/// driver present the renderer is turned off, unless the user already set the variable either way.
pub fn renderer_policy(
    nvidia: bool,
    explicit: Option<&OsStr>,
) -> Option<(&'static str, &'static str)> {
    (nvidia && explicit.is_none()).then_some((RENDERER_VARIABLE, RENDERER_VALUE))
}

/// Applies [`renderer_policy`] to this process. Call before GTK starts. A no-op on Windows.
pub fn apply_renderer_policy() {
    #[cfg(target_os = "linux")]
    {
        let nvidia = NVIDIA_PATHS
            .iter()
            .any(|path| std::path::Path::new(path).exists());
        let explicit = std::env::var_os(RENDERER_VARIABLE);
        if let Some((name, value)) = renderer_policy(nvidia, explicit.as_deref()) {
            std::env::set_var(name, value);
        }
    }
}

/// Sets the program name to [`APPLICATION_ID`], so the Wayland app id and the X11 class match
/// the installed `org.sylin.ghostlight.desktop` entry. A no-op on Windows.
pub fn prepare_identity() {
    #[cfg(target_os = "linux")]
    gtk::glib::set_prgname(Some(APPLICATION_ID));
}

/// Initializes the display toolkit. An error means the session has no display and the caller
/// exits, because `tao` would otherwise panic when it creates the event loop. Always `Ok` on
/// Windows.
pub fn init_display() -> Result<(), WindowError> {
    #[cfg(target_os = "linux")]
    gtk::init().map_err(|error| WindowError::NoDisplay {
        detail: error.to_string(),
    })?;
    Ok(())
}

/// The top-left corner that centers a window of `window_size` on a monitor, in physical pixels.
///
/// A window larger than the monitor is placed at the monitor's corner.
pub fn centered_origin(
    monitor_position: (i32, i32),
    monitor_size: (u32, u32),
    window_size: (u32, u32),
) -> (i32, i32) {
    let offset = |monitor: u32, window: u32| (i64::from(monitor) - i64::from(window)).max(0) / 2;
    let x = i64::from(monitor_position.0) + offset(monitor_size.0, window_size.0);
    let y = i64::from(monitor_position.1) + offset(monitor_size.1, window_size.1);
    (
        i32::try_from(x).unwrap_or(monitor_position.0),
        i32::try_from(y).unwrap_or(monitor_position.1),
    )
}

/// What a window shows and how it hears the page.
pub struct ShellView {
    /// The first URL to load, a page of the private scheme.
    pub start_url: String,
    /// The pages the scheme serves. The service replaces its bundle through this handle.
    pub bundles: Rc<RefCell<BundleSet>>,
    /// Receives each message a page posts, as the page URL and the message body.
    pub on_ipc: Box<dyn Fn(String, String)>,
}

/// The shell's one window with its webview.
pub struct ShellWindow {
    // The webview drops before the window it is attached to.
    webview: WebView,
    window: Window,
}

fn window_icon(rgba: Vec<u8>, width: u32, height: u32) -> Result<Icon, WindowError> {
    Icon::from_rgba(rgba, width, height).map_err(native)
}

impl ShellWindow {
    /// Builds the window and its webview, shows the window centered on its monitor and loads
    /// the view's start URL. Call on the event-loop thread.
    pub fn create<T: 'static>(
        target: &EventLoopWindowTarget<T>,
        view: ShellView,
    ) -> Result<ShellWindow, WindowError> {
        let (rgba, width, height) = icon_image().map_err(native)?;
        let builder = WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1))
            .with_min_inner_size(LogicalSize::new(WINDOW_MIN_SIZE.0, WINDOW_MIN_SIZE.1))
            .with_window_icon(Some(window_icon(rgba, width, height)?))
            .with_visible(false);
        #[cfg(windows)]
        let builder = {
            use crate::tray::decode_rgba;
            use tao::platform::windows::WindowBuilderExtWindows;
            let (rgba, width, height) = decode_rgba(TASKBAR_ICON_PNG).map_err(native)?;
            builder
                .with_window_classname(WINDOW_CLASS)
                .with_taskbar_icon(Some(window_icon(rgba, width, height)?))
        };
        let window = builder.build(target).map_err(native)?;
        center(&window);

        let ShellView {
            start_url,
            bundles,
            on_ipc,
        } = view;
        let webview_builder = harden(WebViewBuilder::new())
            .with_custom_protocol(SCHEME.into(), move |_, request| {
                respond(
                    &bundles.borrow(),
                    request.method().as_str(),
                    &request.uri().to_string(),
                )
            })
            .with_ipc_handler(move |request| {
                on_ipc(request.uri().to_string(), request.into_body());
            })
            .with_initialization_script_for_main_only(BRIDGE_SCRIPT, true)
            .with_url(start_url);
        let webview = build_webview(webview_builder, &window)?;

        window.set_visible(true);
        Ok(ShellWindow { webview, window })
    }

    /// Restores the window from minimized, shows it and gives it focus.
    pub fn focus(&self) {
        self.window.set_minimized(false);
        self.window.set_visible(true);
        self.window.set_focus();
    }

    /// Loads the bundle page at `path`.
    pub fn navigate(&self, path: &str) -> Result<(), WindowError> {
        self.webview.load_url(&page_url(path)).map_err(native)
    }

    /// Delivers `message` to the page's listeners.
    pub fn deliver(&self, message: &Value) -> Result<(), WindowError> {
        self.webview
            .evaluate_script(&delivery_script(message))
            .map_err(native)
    }

    /// The window's id in the event loop.
    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    /// Calls `callback` whenever the webview's renderer process ends.
    ///
    /// The callback runs inside a WebKitGTK signal. It must only post an event to the loop and
    /// never drop this window there.
    #[cfg(target_os = "linux")]
    pub fn on_renderer_lost(&self, callback: impl Fn() + 'static) {
        use webkit2gtk::WebViewExt;
        use wry::WebViewExtUnix;
        self.webview
            .webview()
            .connect_web_process_terminated(move |_, _| callback());
    }
}

fn center(window: &Window) {
    let Some(monitor) = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
    else {
        return;
    };
    let position = monitor.position();
    let size = monitor.size();
    let outer = window.outer_size();
    let (x, y) = centered_origin(
        (position.x, position.y),
        (size.width, size.height),
        (outer.width, outer.height),
    );
    window.set_outer_position(PhysicalPosition::new(x, y));
}

#[cfg(windows)]
fn build_webview(builder: WebViewBuilder<'_>, window: &Window) -> Result<WebView, WindowError> {
    builder.build(window).map_err(native)
}

#[cfg(not(windows))]
fn build_webview(builder: WebViewBuilder<'_>, window: &Window) -> Result<WebView, WindowError> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    let container = window.default_vbox().ok_or_else(|| WindowError::Native {
        detail: "the window has no container for the webview".to_string(),
    })?;
    builder.build_gtk(container).map_err(native)
}

#[cfg(test)]
mod tests {
    use super::*;
    use LifecycleEvent::{CloseRequested, Destroyed, OpenRequested, RendererLost};

    fn linux() -> WindowLifecycle {
        WindowLifecycle::new(true)
    }

    fn windows() -> WindowLifecycle {
        WindowLifecycle::new(false)
    }

    #[test]
    fn open_close_destroyed_open_creates_twice_in_both_flavors() {
        for mut lifecycle in [linux(), windows()] {
            assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Create));
            assert!(lifecycle.is_open());
            assert_eq!(lifecycle.on(CloseRequested), Some(WindowAct::Destroy));
            assert_eq!(lifecycle.on(Destroyed), None);
            assert_eq!(lifecycle.state(), WindowState::Absent);
            assert!(!lifecycle.is_open());
            assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Create));
            assert!(lifecycle.is_open());
        }
    }

    #[test]
    fn windows_focuses_an_open_window_and_linux_replaces_it() {
        let mut lifecycle = windows();
        lifecycle.on(OpenRequested);
        assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Focus));
        assert_eq!(lifecycle.state(), WindowState::Open);

        let mut lifecycle = linux();
        lifecycle.on(OpenRequested);
        assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Destroy));
        assert_eq!(lifecycle.state(), WindowState::Replacing);
        assert!(!lifecycle.is_open());
    }

    #[test]
    fn focus_appears_only_in_the_windows_flavor() {
        let mut seen = Vec::new();
        for mut lifecycle in [linux(), windows()] {
            let mut acts = vec![lifecycle.on(OpenRequested)];
            acts.push(lifecycle.on(OpenRequested));
            acts.push(lifecycle.on(Destroyed));
            acts.push(lifecycle.on(OpenRequested));
            seen.push(acts.contains(&Some(WindowAct::Focus)));
        }
        assert_eq!(seen, vec![false, true]);
    }

    #[test]
    fn opens_during_a_pending_replacement_are_coalesced_into_one_create() {
        let mut lifecycle = linux();
        lifecycle.on(OpenRequested);
        assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Destroy));
        for _ in 0..5 {
            assert_eq!(lifecycle.on(OpenRequested), None);
            assert_eq!(lifecycle.state(), WindowState::Replacing);
        }
        assert_eq!(lifecycle.on(Destroyed), Some(WindowAct::Create));
        assert_eq!(lifecycle.state(), WindowState::Open);
        assert_eq!(lifecycle.on(Destroyed), None);
    }

    #[test]
    fn a_lost_renderer_discards_the_window_and_creates_nothing() {
        for mut lifecycle in [linux(), windows()] {
            lifecycle.on(OpenRequested);
            assert_eq!(lifecycle.on(RendererLost), Some(WindowAct::Destroy));
            assert_eq!(lifecycle.on(Destroyed), None);
            assert_eq!(lifecycle.state(), WindowState::Absent);
            assert_eq!(lifecycle.on(RendererLost), None);
            assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Create));
        }
    }

    #[test]
    fn close_and_destroyed_while_absent_do_nothing() {
        for mut lifecycle in [linux(), windows()] {
            assert_eq!(lifecycle.on(CloseRequested), None);
            assert_eq!(lifecycle.on(Destroyed), None);
            assert_eq!(lifecycle.on(RendererLost), None);
            assert_eq!(lifecycle.state(), WindowState::Absent);
        }
    }

    #[test]
    fn close_and_renderer_loss_during_a_replacement_change_nothing() {
        let mut lifecycle = linux();
        lifecycle.on(OpenRequested);
        lifecycle.on(OpenRequested);
        assert_eq!(lifecycle.on(CloseRequested), None);
        assert_eq!(lifecycle.on(RendererLost), None);
        assert_eq!(lifecycle.state(), WindowState::Replacing);
    }

    #[test]
    fn a_failed_create_returns_to_absent_when_destroyed_is_fed() {
        let mut lifecycle = windows();
        assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Create));
        lifecycle.on(Destroyed);
        assert_eq!(lifecycle.state(), WindowState::Absent);
        assert_eq!(lifecycle.on(OpenRequested), Some(WindowAct::Create));
    }

    #[test]
    fn the_platform_lifecycle_replaces_only_on_linux() {
        let mut lifecycle = WindowLifecycle::for_platform();
        lifecycle.on(OpenRequested);
        let expected = if cfg!(target_os = "linux") {
            WindowAct::Destroy
        } else {
            WindowAct::Focus
        };
        assert_eq!(lifecycle.on(OpenRequested), Some(expected));
    }

    #[test]
    fn nvidia_without_an_explicit_value_turns_the_renderer_off() {
        assert_eq!(
            renderer_policy(true, None),
            Some(("WEBKIT_DISABLE_DMABUF_RENDERER", "1"))
        );
    }

    #[test]
    fn an_explicit_value_wins_whatever_it_is() {
        assert_eq!(renderer_policy(true, Some(OsStr::new("0"))), None);
        assert_eq!(renderer_policy(true, Some(OsStr::new(""))), None);
        assert_eq!(renderer_policy(true, Some(OsStr::new("1"))), None);
    }

    #[test]
    fn without_nvidia_the_environment_is_left_alone() {
        assert_eq!(renderer_policy(false, None), None);
        assert_eq!(renderer_policy(false, Some(OsStr::new("1"))), None);
    }

    #[test]
    fn the_window_is_centered_on_its_monitor() {
        assert_eq!(
            centered_origin((0, 0), (1920, 1080), (1180, 760)),
            (370, 160)
        );
        assert_eq!(
            centered_origin((-1920, 0), (1920, 1080), (1180, 760)),
            (-1550, 160)
        );
    }

    #[test]
    fn a_window_larger_than_the_monitor_sits_at_its_corner() {
        assert_eq!(
            centered_origin((100, 50), (800, 600), (1180, 760)),
            (100, 50)
        );
    }

    #[test]
    fn the_window_constants_hold_the_product_sizes() {
        assert_eq!(WINDOW_TITLE, "Ghostlight");
        assert_eq!(WINDOW_SIZE, (1180.0, 760.0));
        assert_eq!(WINDOW_MIN_SIZE, (880.0, 620.0));
        assert_eq!(WINDOW_CLASS, "Ghostlight Window");
        assert_eq!(APPLICATION_ID, "org.sylin.ghostlight");
    }

    #[test]
    fn display_and_identity_setup_is_inert_on_windows() {
        if cfg!(windows) {
            apply_renderer_policy();
            prepare_identity();
            assert_eq!(init_display(), Ok(()));
        }
    }
}
