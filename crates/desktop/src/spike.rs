//! The standalone spike window (ADR-0199 Decisions 1 and 7).
//!
//! The spike composes the shell's modules into one running program so they can be checked live
//! before the shell is wired to the service: a tray, one window and an echo through the bridge.
//! Its words and files are test content. They live only here and in `crates/desktop/spike/`, and
//! the shell lane deletes them when it wires the service.
//!
//! The pure parts (the tray model, the bundle, the reply to a page, the tray action) are unit
//! tested. [`run`] is native glue that opens a window, so it is checked live and never in
//! `cargo test`.

use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Mutex;

use serde_json::Value;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

use crate::bridge::{decode_page_message, page_origin, PageOrigin};
use crate::bundle::{Bundle, BundleError, BundleSet, SHELL_PREFIX};
use crate::scheme::page_url;
use crate::session::{self, SessionSignal, SessionState, FALLBACK_EXIT};
use crate::tray::{Tray, TrayEvent, TrayItem, TrayModel};
use crate::window::{
    self, LifecycleEvent, ShellView, ShellWindow, WindowAct, WindowError, WindowLifecycle,
};

/// The exit code of a session with no display.
pub const EXIT_NO_DISPLAY: u8 = 3;

/// The exit code of a failure that is not a missing display.
const EXIT_FAILURE: u8 = 1;

/// The tray tooltip.
const TOOLTIP: &str = "Ghostlight";

/// The tray item that opens the window.
const ITEM_OPEN: &str = "open";

/// The tray item that closes the window.
const ITEM_CLOSE: &str = "close";

/// The tray item that ends the spike.
const ITEM_QUIT: &str = "quit";

/// The page the window starts on.
const INDEX_PAGE: &str = "index.html";

/// The shell bundle's placeholder page.
const SHELL_PAGE: &str = "index.html";

/// The field added to every message the shell echoes.
const FIELD_FROM: &str = "from";

/// The value of [`FIELD_FROM`].
const SOURCE_SHELL: &str = "shell";

/// The one page of the shell bundle. The spike has no shell page of its own to show.
const SHELL_PLACEHOLDER: &str = "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Ghostlight</title></head>\n<body></body>\n</html>\n";

/// The files of the spike page, by bundle path.
const SPIKE_FILES: [(&str, &[u8]); 4] = [
    ("index.html", include_bytes!("../spike/index.html")),
    ("spike.css", include_bytes!("../spike/spike.css")),
    ("spike.js", include_bytes!("../spike/spike.js")),
    (
        "mascot.png",
        include_bytes!("../../../extension/icons/icon128.png"),
    ),
];

/// What a tray choice asks the spike to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpikeAction {
    /// Show the window.
    Open,
    /// Close the window.
    Close,
    /// End the spike.
    Quit,
}

/// The tray of the spike: an Open, a Close and a Quit item.
pub fn tray_model() -> TrayModel {
    let item = |id: &str, label: &str| TrayItem {
        id: id.to_string(),
        label: label.to_string(),
        enabled: true,
    };
    TrayModel {
        tooltip: TOOLTIP.to_string(),
        items: vec![
            item(ITEM_OPEN, "Open window"),
            item(ITEM_CLOSE, "Close window"),
            item(ITEM_QUIT, "Quit spike"),
        ],
    }
}

/// What a tray event asks for, if anything. A left click on the icon opens the window.
pub fn tray_action(event: &TrayEvent) -> Option<SpikeAction> {
    match event {
        TrayEvent::Activate => Some(SpikeAction::Open),
        TrayEvent::Item(id) => match id.as_str() {
            ITEM_OPEN => Some(SpikeAction::Open),
            ITEM_CLOSE => Some(SpikeAction::Close),
            ITEM_QUIT => Some(SpikeAction::Quit),
            _ => None,
        },
    }
}

/// The service bundle of the spike: its page, style, script and mascot.
pub fn spike_bundle() -> Result<Bundle, BundleError> {
    Bundle::service(
        SPIKE_FILES
            .iter()
            .map(|(path, bytes)| (path.to_string(), bytes.to_vec()))
            .collect(),
    )
}

/// The bundles the spike serves: a placeholder shell page and the spike bundle.
pub fn spike_bundles() -> Result<BundleSet, BundleError> {
    let shell = Bundle::shell(vec![(
        format!("{SHELL_PREFIX}/{SHELL_PAGE}"),
        SHELL_PLACEHOLDER.as_bytes().to_vec(),
    )])?;
    let mut bundles = BundleSet::new(shell);
    bundles.set_service(spike_bundle()?);
    Ok(bundles)
}

/// The reply to a message a page posted from `url`, or `None` when the message is dropped.
///
/// A message from a foreign page is dropped, and so is one that is not a JSON object. Every other
/// message comes back as it was sent, marked as coming from the shell.
pub fn reply_to_page(url: &str, body: &str) -> Option<Value> {
    if page_origin(url) == PageOrigin::Foreign {
        return None;
    }
    let mut message = decode_page_message(body).ok()?;
    message.insert(
        FIELD_FROM.to_string(),
        Value::String(SOURCE_SHELL.to_string()),
    );
    Some(Value::Object(message))
}

/// What reaches the event loop from outside it.
#[derive(Debug)]
enum SpikeEvent {
    /// The user used the tray.
    Tray(TrayEvent),
    /// A page posted a message, and this is the reply.
    Reply(Value),
    /// The webview's renderer process ended.
    #[cfg(target_os = "linux")]
    RendererLost,
}

fn post(proxy: &EventLoopProxy<SpikeEvent>, event: SpikeEvent) {
    // A closed loop has nothing left to tell.
    let _ = proxy.send_event(event);
}

/// Ends the process on the final signal of the session, arming the fallback exit first.
fn end_session(state: &SessionState, signal: SessionSignal) {
    session::handle(
        state,
        signal,
        || session::arm_fallback_exit(FALLBACK_EXIT),
        || std::process::exit(0),
    );
}

/// The state the event loop carries.
struct Spike {
    lifecycle: WindowLifecycle,
    window: Option<ShellWindow>,
    // Held so the icon stays in the notification area.
    _tray: Option<Tray>,
    bundles: Rc<RefCell<BundleSet>>,
    proxy: EventLoopProxy<SpikeEvent>,
    session: SessionState,
}

impl Spike {
    fn start(&mut self, target: &EventLoopWindowTarget<SpikeEvent>) {
        match Tray::build(&tray_model()) {
            Ok(tray) => self._tray = Some(tray),
            Err(error) => eprintln!("ghostlight-desktop: {error}"),
        }
        #[cfg(target_os = "linux")]
        {
            let state = self.session.clone();
            session::install_unix_signals(move |signal| end_session(&state, signal));
        }
        self.apply(LifecycleEvent::OpenRequested, target);
    }

    fn apply(&mut self, event: LifecycleEvent, target: &EventLoopWindowTarget<SpikeEvent>) {
        match self.lifecycle.on(event) {
            Some(WindowAct::Create) => self.create_window(target),
            Some(WindowAct::Focus) => {
                if let Some(window) = &self.window {
                    window.focus();
                }
            }
            Some(WindowAct::Destroy) => self.window = None,
            None => {}
        }
    }

    fn create_window(&mut self, target: &EventLoopWindowTarget<SpikeEvent>) {
        let proxy = self.proxy.clone();
        let view = ShellView {
            start_url: page_url(INDEX_PAGE),
            bundles: Rc::clone(&self.bundles),
            on_ipc: Box::new(move |url, body| {
                if let Some(reply) = reply_to_page(&url, &body) {
                    post(&proxy, SpikeEvent::Reply(reply));
                }
            }),
        };
        match ShellWindow::create(target, view) {
            Ok(window) => {
                #[cfg(target_os = "linux")]
                {
                    let proxy = self.proxy.clone();
                    window.on_renderer_lost(move || post(&proxy, SpikeEvent::RendererLost));
                }
                window.focus();
                self.window = Some(window);
            }
            Err(error) => {
                eprintln!("ghostlight-desktop: {error}");
                self.apply(LifecycleEvent::Destroyed, target);
            }
        }
    }

    fn deliver(&self, reply: &Value) {
        if let Some(window) = &self.window {
            if let Err(error) = window.deliver(reply) {
                eprintln!("ghostlight-desktop: {error}");
            }
        }
    }

    fn handle(
        &mut self,
        event: Event<'_, SpikeEvent>,
        target: &EventLoopWindowTarget<SpikeEvent>,
        flow: &mut ControlFlow,
    ) {
        *flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => self.start(target),
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if self.window.as_ref().is_some_and(|w| w.id() == window_id) => {
                self.apply(LifecycleEvent::CloseRequested, target);
            }
            Event::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            } => self.apply(LifecycleEvent::Destroyed, target),
            Event::UserEvent(SpikeEvent::Tray(event)) => match tray_action(&event) {
                Some(SpikeAction::Open) => self.apply(LifecycleEvent::OpenRequested, target),
                Some(SpikeAction::Close) => self.apply(LifecycleEvent::CloseRequested, target),
                Some(SpikeAction::Quit) => *flow = ControlFlow::Exit,
                None => {}
            },
            Event::UserEvent(SpikeEvent::Reply(reply)) => self.deliver(&reply),
            #[cfg(target_os = "linux")]
            Event::UserEvent(SpikeEvent::RendererLost) => {
                self.apply(LifecycleEvent::RendererLost, target);
            }
            Event::LoopDestroyed => end_session(&self.session, SessionSignal::Final),
            _ => {}
        }
    }
}

/// Runs the spike: prepares the display, installs the tray and opens the window. Returns only
/// when the spike cannot start; otherwise the process ends from the event loop.
pub fn run() -> ExitCode {
    window::apply_renderer_policy();
    window::prepare_identity();
    if let Err(error) = window::init_display() {
        eprintln!("ghostlight-desktop: {error}");
        return ExitCode::from(exit_code_of(&error));
    }
    let bundles = match spike_bundles() {
        Ok(bundles) => bundles,
        Err(error) => {
            eprintln!("ghostlight-desktop: {error}");
            return ExitCode::from(EXIT_FAILURE);
        }
    };
    let event_loop = EventLoopBuilder::<SpikeEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let tray_proxy = Mutex::new(proxy.clone());
    crate::tray::install_events(move |event| {
        if let Ok(proxy) = tray_proxy.lock() {
            post(&proxy, SpikeEvent::Tray(event));
        }
    });
    let mut spike = Spike {
        lifecycle: WindowLifecycle::for_platform(),
        window: None,
        _tray: None,
        bundles: Rc::new(RefCell::new(bundles)),
        proxy,
        session: SessionState::new(),
    };
    event_loop.run(move |event, target, flow| spike.handle(event, target, flow))
}

fn exit_code_of(error: &WindowError) -> u8 {
    match error {
        WindowError::NoDisplay { .. } => EXIT_NO_DISPLAY,
        WindowError::Native { .. } => EXIT_FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheme::respond;
    use wry::http::StatusCode;

    const CONTENT_POLICY_HEADER: &str = "Content-Security-Policy";

    /// Every value of a `src="..."` or `href="..."` attribute in `html`.
    fn references(html: &str) -> Vec<String> {
        let mut found = Vec::new();
        for attribute in ["src=\"", "href=\""] {
            let mut rest = html;
            while let Some(start) = rest.find(attribute) {
                rest = &rest[start + attribute.len()..];
                if let Some(end) = rest.find('"') {
                    found.push(rest[..end].to_string());
                }
            }
        }
        found
    }

    #[test]
    fn the_tray_model_is_valid_and_names_open_close_and_quit() {
        let model = tray_model();
        assert_eq!(model.validate(), Ok(()));
        assert_eq!(model.tooltip, "Ghostlight");
        let labels: Vec<&str> = model.items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Open window", "Close window", "Quit spike"]);
    }

    #[test]
    fn every_tray_item_and_a_left_click_map_to_an_action() {
        let ids: Vec<String> = tray_model().items.into_iter().map(|i| i.id).collect();
        let actions: Vec<Option<SpikeAction>> = ids
            .into_iter()
            .map(|id| tray_action(&TrayEvent::Item(id)))
            .collect();
        assert_eq!(
            actions,
            [
                Some(SpikeAction::Open),
                Some(SpikeAction::Close),
                Some(SpikeAction::Quit)
            ]
        );
        assert_eq!(tray_action(&TrayEvent::Activate), Some(SpikeAction::Open));
        assert_eq!(tray_action(&TrayEvent::Item("other".to_string())), None);
    }

    #[test]
    fn the_spike_bundle_holds_nothing_under_the_shell_prefix() {
        let bundle = spike_bundle().expect("the spike files make a bundle");
        assert!(bundle
            .paths()
            .all(|path| !path.starts_with(&format!("{SHELL_PREFIX}/"))));
    }

    #[test]
    fn the_spike_page_references_only_files_of_the_spike_bundle() {
        let bundle = spike_bundle().expect("the spike files make a bundle");
        let index = bundle.get(INDEX_PAGE).expect("the bundle has an index");
        let html = String::from_utf8(index.bytes.to_vec()).expect("the index is text");
        let found = references(&html);
        assert!(
            found.len() >= 3,
            "the page links its style, script and image"
        );
        for reference in found {
            assert!(
                bundle.get(&reference).is_some(),
                "{reference} is not in the spike bundle"
            );
        }
    }

    #[test]
    fn every_file_of_the_bundles_is_served_with_the_content_policy() {
        let bundles = spike_bundles().expect("the spike bundles build");
        let mut paths: Vec<String> = spike_bundle()
            .expect("the spike files make a bundle")
            .paths()
            .map(str::to_string)
            .collect();
        paths.push(format!("{SHELL_PREFIX}/{SHELL_PAGE}"));
        for path in paths {
            let response = respond(&bundles, "GET", &page_url(&path));
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert!(
                response.headers().contains_key(CONTENT_POLICY_HEADER),
                "{path}"
            );
        }
    }

    #[test]
    fn a_message_from_a_spike_page_comes_back_marked_from_the_shell() {
        let reply = reply_to_page(&page_url(INDEX_PAGE), r#"{"kind":"echo","n":3}"#)
            .expect("an object from a service page is echoed");
        assert_eq!(reply["kind"], "echo");
        assert_eq!(reply["n"], 3);
        assert_eq!(reply["from"], "shell");
    }

    #[test]
    fn a_message_from_a_shell_page_is_echoed_too() {
        let url = page_url(&format!("{SHELL_PREFIX}/{SHELL_PAGE}"));
        assert!(reply_to_page(&url, "{}").is_some());
    }

    #[test]
    fn a_foreign_page_and_a_body_that_is_not_an_object_get_no_reply() {
        assert_eq!(reply_to_page("https://example.com/", "{}"), None);
        assert_eq!(reply_to_page("about:blank", "{}"), None);
        assert_eq!(reply_to_page(&page_url(INDEX_PAGE), "[1]"), None);
        assert_eq!(reply_to_page(&page_url(INDEX_PAGE), "not json"), None);
    }

    #[test]
    fn a_posted_from_field_is_replaced_by_the_shell_marker() {
        let reply = reply_to_page(&page_url(INDEX_PAGE), r#"{"from":"page"}"#)
            .expect("an object is echoed");
        assert_eq!(reply["from"], "shell");
    }

    #[test]
    fn no_display_is_exit_code_three_and_other_failures_are_one() {
        let no_display = WindowError::NoDisplay {
            detail: String::new(),
        };
        let native = WindowError::Native {
            detail: String::new(),
        };
        assert_eq!(exit_code_of(&no_display), EXIT_NO_DISPLAY);
        assert_eq!(exit_code_of(&native), EXIT_FAILURE);
    }
}
