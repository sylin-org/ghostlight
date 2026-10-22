//! The relay endpoint's accept loop, and what each judged connection is served by its role: the
//! browser, `control`, `mcp` and `desktop` streams. [`ServiceContext`] is everything a connection
//! needs from the service; the `control` stream's answers are here too.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ghostlight_relay::endpoint::{Connection, Listener};
use ghostlight_relay::protocol::{Control, Reply, Role, StartReason, Templates};
use serde_json::Value;

use crate::browser::{BrowserPort, RelayBrowserPort};
use crate::diagnostics::DiagnosticsHub;
use crate::governance::GovernanceFacade;
use crate::install;
use crate::intake::desktop::ShellView;
use crate::intake::listener::{self, control, Judgement};
use crate::language::intake::OpeningRefusal;
use crate::provenance::SourceReader;
use crate::work::ApplicationExecutor;
use crate::workbench::web::WorkbenchServer;
use crate::workbench::WorkbenchFacade;
use crate::workspace::WorkspaceStore;

use super::admission::{self, Capacity, QueueBudget};
use super::idle::Streams;
use super::lifetime::QUIT_GRACE;
use super::mcp_stream::serve_mcp;
use super::presence::Presence;
use super::quit::Quit;
use super::shell::serve_desktop;
use super::toolset::Toolsets;

/// How long the accept loop rests after a failed accept before it asks again.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

/// Everything a relay connection needs from the service.
pub(super) struct ServiceContext {
    /// The value that changes on every start of this service.
    pub(super) instance: String,
    /// Why this service was started.
    pub(super) started_by: StartReason,
    /// The failure sentences every accepted `mcp` stream is handed.
    pub(super) templates: Templates,
    pub(super) executor: Arc<ApplicationExecutor>,
    pub(super) workspaces: WorkspaceStore,
    pub(super) browser_port: Arc<dyn BrowserPort>,
    pub(super) browser: Arc<RelayBrowserPort>,
    pub(super) workbench: WorkbenchFacade,
    pub(super) governance: GovernanceFacade,
    pub(super) diagnostics: Arc<DiagnosticsHub>,
    pub(super) quit: Quit,
    pub(super) sessions: Capacity,
    /// The places for `desktop` streams.
    pub(super) desktops: Capacity,
    pub(super) budget: QueueBudget,
    pub(super) stop: Arc<AtomicBool>,
    /// The workbench's server, once `publish_ready` has started it. The desktop role reads its
    /// address and bearer from here.
    pub(super) workbench_server: OnceLock<Arc<WorkbenchServer>>,
    /// Keeps the shell running whenever the desktop can show it.
    pub(super) presence: Arc<Presence>,
    /// The open streams of every role, which the idle exit counts.
    pub(super) streams: Arc<Streams>,
    /// Where the installation keeps its files.
    pub(super) layout: install::layout::Layout,
    /// How each accepted `mcp` stream's source is read from its peer process.
    pub(super) sources: SourceReader,
    /// Every live MCP connection, and which offers each agent the tools.
    pub(super) toolsets: Arc<Toolsets>,
}

impl ServiceContext {
    /// The reply that accepts an opening for `role`. Only an `mcp` stream is handed the failure
    /// templates.
    pub(super) fn accepted(&self, role: Role) -> Reply {
        Reply::accepted(
            env!("CARGO_PKG_VERSION").into(),
            self.instance.clone(),
            (role == Role::Mcp).then(|| self.templates.clone()),
        )
    }
}

/// Accept relay connections until the service stops, each on its own thread.
///
/// A connection holds one of a few pre-judgement slots until its opening is judged, so local
/// processes that connect and say nothing cannot exhaust the service.
pub(super) fn spawn_listener(
    listener: Arc<Listener>,
    context: Arc<ServiceContext>,
) -> JoinHandle<()> {
    let handshakes = Capacity::new(admission::HANDSHAKES);
    thread::Builder::new()
        .name("ghostlight-relay-listener".into())
        .spawn(move || {
            while !context.stop.load(Ordering::SeqCst) {
                let connection = match listener.accept() {
                    Ok(connection) => connection,
                    Err(_) if context.stop.load(Ordering::SeqCst) => break,
                    Err(_) => {
                        thread::sleep(ACCEPT_RETRY);
                        continue;
                    }
                };
                if context.stop.load(Ordering::SeqCst) {
                    break;
                }
                let Some(handshake) = handshakes.acquire(1) else {
                    continue;
                };
                let context = Arc::clone(&context);
                let _ = thread::Builder::new()
                    .name("ghostlight-relay-connection".into())
                    .spawn(move || {
                        let judgement =
                            listener::judge(&connection, context.quit.drain.is_quitting());
                        drop(handshake);
                        serve_connection(connection, judgement, &context);
                    });
            }
        })
        .expect("relay listener thread starts")
}

/// Serve one judged connection to its end.
fn serve_connection(connection: Connection, judgement: Judgement, context: &Arc<ServiceContext>) {
    match judgement {
        Judgement::Drop => {}
        Judgement::Refuse(refusal) => listener::refuse(&connection, refusal),
        Judgement::Admit(opening) => {
            match opening.role {
                Role::Browser => {
                    // Only the opening is held here; the port counts the stream while it lives.
                    let Some(_opening) = context.streams.admit() else {
                        return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                    };
                    if listener::accept(&connection, &context.accepted(Role::Browser)).is_ok() {
                        // The first frame on the stream is the extension's own hello.
                        let _ = context.browser.attach(connection);
                        context.workbench.refresh_live_parts();
                    }
                }
                Role::Control => {
                    let Some(_stream) = context.streams.admit() else {
                        return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                    };
                    serve_control(connection, opening.control, context);
                }
                Role::Mcp => {
                    let Some(_stream) = context.streams.admit() else {
                        return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                    };
                    if let Err(error) = serve_mcp(connection, context, &opening) {
                        context.diagnostics.sink().emit(
                        ghostlight_bridge::diagnostics::event::HARNESS_DETACHED,
                        ghostlight_bridge::diagnostics::Level::Warn,
                        None,
                        &format!("the session ended after an invalid or interrupted exchange: {error}"),
                    );
                    }
                }
                Role::Desktop => {
                    let Some(_stream) = context.streams.admit() else {
                        return listener::refuse(&connection, OpeningRefusal::ShuttingDown);
                    };
                    // A shell that goes away abruptly or sends a frame this build cannot read ends
                    // its own stream and nothing else.
                    let view = ShellView::from_channel(opening.channel.as_deref());
                    let _ = serve_desktop(connection, context, view);
                }
                Role::Command | Role::Unknown => {
                    listener::refuse(&connection, OpeningRefusal::Unsupported);
                }
            }
        }
    }
}

/// The answer to a control `status`: who this service is, what is connected and where the
/// workbench is. The workbench's bearer is never part of it.
fn control_status(context: &ServiceContext) -> control::Status {
    control::Status {
        service: env!("CARGO_PKG_VERSION").into(),
        instance: context.instance.clone(),
        pid: std::process::id(),
        started_by: context.started_by,
        connections: Some(context.workbench.connection_counts()),
        workbench: context
            .workbench_server
            .get()
            .map(|server| control::WorkbenchStatus {
                url: server.address().url().to_owned(),
                usual_port: server.address().usual(),
                watching: context.workbench.watching(),
            }),
        window: context.presence.window_reason(),
    }
}

/// Answer one control operation and close.
fn serve_control(connection: Connection, operation: Option<Control>, context: &ServiceContext) {
    let value = match operation {
        Some(Control::Readiness) => {
            serde_json::to_value(context.workbench.readiness()).unwrap_or(Value::Null)
        }
        Some(Control::Status) => {
            serde_json::to_value(control_status(context)).unwrap_or(Value::Null)
        }
        Some(Control::Show) => serde_json::to_value(control::Shown {
            shown: context
                .workbench
                .reveal(context.workbench.show_address())
                .is_ok(),
        })
        .unwrap_or(Value::Null),
        Some(Control::Quit { reason }) => {
            context.quit.begin(reason, QUIT_GRACE);
            serde_json::to_value(control::Quitting { quitting: true }).unwrap_or(Value::Null)
        }
        Some(Control::Unknown) | None => {
            listener::refuse(&connection, OpeningRefusal::Unsupported);
            return;
        }
    };
    if listener::accept(&connection, &context.accepted(Role::Control)).is_ok() {
        let _ = listener::answer_control(&connection, &value);
    }
}
