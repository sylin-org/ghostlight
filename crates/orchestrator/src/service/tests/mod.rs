//! The service's test fixtures, and its tests by concern.

mod browser;
mod desktop;
mod idle;
mod quit;
mod sessions;
mod start;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::intake::session::{IntakeChannel, SessionMarker};
use crate::provenance::{ConnectionEvidence, Observation, Source, SourceProcess, SourceReader};
use ghostlight_relay::endpoint::{pair, Connection, Listener};
use ghostlight_relay::frame;
use ghostlight_relay::protocol::{
    self, Control, Edge, GoodbyeReason, McpFrame, Opening as RelayOpening, Program, QuitReason,
    Reply, Role, StartReason,
};
use serde_json::{json, Value};

use super::lifetime::HandClock;
use super::presence::testing::Fake;
use super::test_home::TestHome;
use crate::browser::adapter::BrowserCommand;
use crate::browser::adapter::RuntimeControlState;
use crate::browser::testing::{FakeBrowser, FAKE_BROWSER};
use crate::cli::client as cli_client;
use crate::install::layout::Layout;
use crate::intake::desktop::{
    ActOutcome, DesktopFrame, NativeAct, ShellView, TrayItemId, TrayModel,
};
use crate::intake::listener::control;
use crate::intake::mcp::failure_templates;
use crate::intake::mcp::ResumeState;
use crate::language::outcome::{CancelCause, QuitCause, Refusal};
use crate::scratch::Scratch;
use crate::work::result::{Effect, Status};
use crate::workbench::{
    PageAddress, WorkbenchNativeActs, WorkbenchNotification, WorkbenchPresentationError,
    WorkbenchPresentationPort,
};
use crate::workspace::ReleasedTabs;

use super::lifetime::QUIT_GRACE;
use super::release::cleanup_released_tabs;
use super::{QuitLatched, ServiceAlreadyRunning, ServiceHost, StartTerms, World};

/// The MCP revision the test clients speak and the resume tokens name.
const REVISION: &str = "2025-06-18";

/// The name the client of an ordinary test session claims.
const CLIENT_NAME: &str = "relay test";

/// The terms of an agent start, which asks for the lock once.
fn agent() -> StartTerms {
    StartTerms::asking_once(StartReason::Agent)
}

/// A fresh directory to serve from, removed when the returned guard drops.
fn temp_home(name: &str) -> Scratch {
    Scratch::named(&format!("service-{name}"))
}

/// The instance every test service reports.
const TEST_INSTANCE: &str = "instance_test";

/// How long a test waits for the service to answer.
const PATIENCE: Duration = Duration::from_secs(10);

/// A relay listener over a fake browser, bound at the test home's endpoint.
struct Served {
    context: Arc<super::ServiceContext>,
    relay: Arc<crate::browser::RelayBrowserPort>,
    listener: Arc<Listener>,
    thread: Option<thread::JoinHandle<()>>,
    quit_requested: mpsc::Receiver<()>,
}

impl Drop for Served {
    fn drop(&mut self) {
        self.context.stop.store(true, Ordering::SeqCst);
        self.listener.shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Page assets a protocol-4 fake can be accepted with.
fn test_assets() -> Arc<crate::browser::adapter::chromium::protocol4::wire::Assets> {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let source = "\"use strict\";\n".to_owned();
    let mut id = String::new();
    for byte in Sha256::digest(source.as_bytes()) {
        write!(&mut id, "{byte:02x}").unwrap();
    }
    Arc::new(crate::browser::adapter::chromium::protocol4::wire::Assets { id, source })
}

/// Serve the relay listener over a fake browser and publish it in `home`.
fn serve_fake(home: &TestHome) -> Served {
    serve(home, false, SourceReader::system(), None)
}

/// Serve the relay listener with the relay port as the session's browser port, so sessions
/// meet browsers only as protocol-4 extensions attach, for their calls as well as their
/// resumed tabs.
fn serve_relayed(home: &TestHome) -> Served {
    serve(home, true, SourceReader::system(), None)
}

/// Serve the relay listener in `home`; `relayed` puts the relay port behind the sessions,
/// `sources` reads each session's source, and `policy` is the user policy, if any.
fn serve(home: &TestHome, relayed: bool, sources: SourceReader, policy: Option<PathBuf>) -> Served {
    use crate::audit::{AuditRecorder, JsonlAuditSink};
    use crate::governance::GovernanceFacade;
    use crate::presentation::{BrowserPresentation, PresentationReactor};
    use crate::work::ApplicationExecutor;
    use crate::workbench::{WorkbenchFacade, WorkbenchProjection};
    use crate::workspace::WorkspaceStore;

    let relay = Arc::new(crate::browser::RelayBrowserPort::new(
        "service_test".into(),
        test_assets(),
    ));
    let port: Arc<dyn crate::browser::BrowserPort> = if relayed {
        relay.clone()
    } else {
        Arc::new(FakeBrowser::default())
    };
    let workspaces = WorkspaceStore::default();
    let governance = GovernanceFacade::new(policy);
    let diagnostics = crate::diagnostics::DiagnosticsHub::for_tests();
    let projection = WorkbenchProjection::default();
    let audit = Arc::new(AuditRecorder::new(
        Arc::new(JsonlAuditSink::new(&home.path().join("audit.jsonl"))),
        projection.clone(),
    ));
    let executor = Arc::new(ApplicationExecutor::new(
        governance.clone(),
        workspaces.clone(),
        port.clone(),
        PresentationReactor::new(Arc::new(BrowserPresentation::new(
            port.clone(),
            workspaces.clone(),
        ))),
        projection.clone(),
        audit,
        diagnostics.clone(),
    ));
    let workbench = WorkbenchFacade::new(
        projection,
        workspaces.clone(),
        governance.clone(),
        relay.clone(),
        diagnostics.clone(),
    );
    let quit = super::Quit::new(
        diagnostics.clone(),
        home.path().join("state").join("quit"),
        relay.clone(),
        workspaces.clone(),
        port.clone(),
    );
    let (signal, quit_requested) = mpsc::channel();
    *super::lock(&quit.signal) = Some(Arc::new(move || {
        let _ = signal.send(());
    }));
    let toolsets = super::Toolsets::new(Arc::clone(&quit.drain));
    let context = Arc::new(super::ServiceContext {
        instance: TEST_INSTANCE.into(),
        started_by: StartReason::Agent,
        templates: failure_templates(),
        executor,
        workspaces,
        browser_port: port,
        browser: relay.clone(),
        workbench,
        governance,
        diagnostics,
        quit,
        sessions: super::Capacity::new(super::admission::SESSIONS),
        desktops: super::Capacity::new(super::admission::DESKTOPS),
        budget: super::QueueBudget::default(),
        stop: Arc::default(),
        workbench_server: std::sync::OnceLock::new(),
        presence: Arc::new(super::Presence::new(Arc::new(Fake::default()).world())),
        streams: super::Streams::new(Arc::new(super::SystemClock), super::idle::IDLE_EXIT),
        layout: Layout::new(home.path()),
        sources,
        toolsets,
    });
    let listener = Arc::new(Listener::bind_first(home.path()).unwrap());
    let thread = super::spawn_listener(Arc::clone(&listener), context.clone());
    Served {
        context,
        relay,
        listener,
        thread: Some(thread),
        quit_requested,
    }
}

/// An opening for `role` from an edge.
fn opening(role: Role) -> RelayOpening {
    RelayOpening::new(role, Edge::new(Program::Edge, "2.0.0".into()))
}

/// An `mcp` opening the way the MCP edge presents it.
fn mcp_opening(resume: Option<String>) -> RelayOpening {
    let mut opening = opening(Role::Mcp);
    opening.channel = Some(protocol::MCP_CHANNEL.into());
    opening.resume = resume;
    opening
}

/// Open `opening` on the service of the test home.
fn dial(opening: &RelayOpening) -> (Connection, Reply) {
    let home = ghostlight_relay::home::home().unwrap();
    let (connection, reply) = protocol::connect(&home, opening, Instant::now() + PATIENCE).unwrap();
    connection.set_read_timeout(Some(PATIENCE)).unwrap();
    (connection, reply)
}

/// Open an `mcp` stream the way the MCP edge does.
fn open_mcp(resume: Option<String>) -> Connection {
    let (connection, reply) = dial(&mcp_opening(resume));
    assert!(reply.accepted, "{reply:?}");
    connection
}

fn send_line(socket: &mut Connection, line: Value) {
    frame::write_frame(
        socket,
        &McpFrame::Message {
            line: line.to_string(),
        },
    )
    .unwrap();
}

/// Read frames until the response to `id`, collecting the resume tokens on the way.
fn response(socket: &mut Connection, id: u64) -> (Value, Vec<String>) {
    let mut tokens = Vec::new();
    loop {
        match frame::read_frame::<McpFrame>(socket).unwrap().unwrap() {
            McpFrame::Message { line } => {
                let message: Value = serde_json::from_str(&line).unwrap();
                if message["id"] == json!(id) {
                    return (message, tokens);
                }
            }
            McpFrame::Resume { token } => tokens.push(token),
            McpFrame::Goodbye { .. } | McpFrame::Unknown => {}
        }
    }
}

fn initialize(socket: &mut Connection) -> (Value, Vec<String>) {
    initialize_as(socket, CLIENT_NAME)
}

/// Initialize a session whose client claims to be `claim`.
fn initialize_as(socket: &mut Connection, claim: &str) -> (Value, Vec<String>) {
    send_line(
        socket,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": REVISION,
                "capabilities": {},
                "clientInfo": {"name": claim, "version": "1"}
            }
        }),
    );
    let answered = response(socket, 1);
    send_line(
        socket,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );
    answered
}

/// Wait until `condition` holds or the patience runs out.
fn eventually(condition: impl Fn() -> bool) {
    let until = Instant::now() + PATIENCE;
    while !condition() {
        assert!(Instant::now() < until, "the condition never held");
        thread::sleep(Duration::from_millis(10));
    }
}

/// What a source read sees: the installed app of `agent`, running as process `id`.
fn running(agent: &str, id: u32) -> Option<Observation> {
    Some(Observation {
        source: Source {
            agent: Some(agent.into()),
            image: "claude".into(),
        },
        process: SourceProcess { id, started: 7 },
    })
}

/// Read frames until the next resume token and decode it.
fn next_resume(socket: &mut Connection) -> ResumeState {
    loop {
        if let McpFrame::Resume { token } = frame::read_frame::<McpFrame>(socket).unwrap().unwrap()
        {
            return ResumeState::decode(&token).unwrap();
        }
    }
}

/// The browser identity of an extension that returns after its session resumed.
const RETURNING_BROWSER: &str = "browser_returning";

/// The tab the returning browser still holds.
const RETURNING_TAB: u64 = 12;

/// The address of the returning browser's tab.
const RETURNING_URL: &str = "https://example.com/";

/// What a page at `url` answers to one world message.
fn world_value_at(message: &Value, url: &str) -> Value {
    if message["kind"] == "read_text" {
        json!({"text": "Example Domain", "truncated": false, "title": "Example",
            "url": url})
    } else {
        Value::Null
    }
}

/// Attach a protocol-4 extension for the returning browser that still lists its tab.
fn attach_returning_browser(
    served: &Served,
) -> crate::browser::adapter::chromium::protocol4::fake::FakeExtension {
    attach_returning_browser_in(served, None, RETURNING_URL)
}

/// Attach a protocol-4 extension for the returning browser whose hello names `life`, or no
/// life for `None`, and that lists its tab at `url`.
fn attach_returning_browser_in(
    served: &Served,
    life: Option<&str>,
    url: &'static str,
) -> crate::browser::adapter::chromium::protocol4::fake::FakeExtension {
    use crate::browser::adapter::chromium::protocol4::fake::{FakeExtension, Reply as FakeReply};
    use crate::browser::adapter::chromium::protocol4::wire::Primitive;

    let mut hello = FakeExtension::hello(RETURNING_BROWSER);
    if let Some(life) = life {
        hello["life"] = json!(life);
    }
    let (service, extension) = pair().unwrap();
    let fake = FakeExtension::connect(
        extension,
        &hello,
        Box::new(move |correlation, primitive| {
            let outcome = match primitive {
                Primitive::ListTabs(_) => json!({"tabs": [{"tab_id": RETURNING_TAB,
                    "title": "Example", "url": url, "active": true,
                    "readiness": "complete"}]}),
                Primitive::Frames(_) => json!({"frames": [{"frame_id": "T",
                    "loader_id": "t1", "parent_frame_id": null, "url": url}],
                    "complete": true, "unseen": []}),
                Primitive::Call(call) => json!({"answers": call.calls.iter().map(|world| {
                    json!({"status": "returned", "envelope": {"ok": true,
                        "value": world_value_at(&world.message, url)}})
                }).collect::<Vec<_>>()}),
                _ => Value::Null,
            };
            FakeReply::Now(FakeExtension::ok(correlation, outcome))
        }),
    );
    served.relay.attach(service).unwrap();
    fake
}

/// The confirmed tabs the one session holds in the workspace store.
fn bound_tabs(served: &Served) -> Vec<(String, u64)> {
    let summaries = served.context.workspaces.summaries();
    assert_eq!(summaries.len(), 1);
    served
        .context
        .workspaces
        .tab_bindings(&summaries[0].id)
        .into_iter()
        .filter(|binding| !binding.pending)
        .map(|binding| (binding.handle, binding.physical_id))
        .collect()
}

/// Start for `terms` in `directory` with a token that is not elevated.
fn start_for(directory: &std::path::Path, terms: StartTerms) -> anyhow::Result<ServiceHost> {
    ServiceHost::start(directory, terms, World::test())
}

/// A host that signals through a channel instead of a desktop.
fn quittable_host(name: &str) -> (TestHome, ServiceHost, mpsc::Receiver<()>) {
    let home = TestHome::new(name);
    let host = ServiceHost::start_for_test(home.path()).unwrap();
    host.publish_ready().unwrap();
    let (quit, quit_requested) = mpsc::channel();
    host.install_quit_signal(Arc::new(move || {
        let _ = quit.send(());
    }));
    (home, host, quit_requested)
}

/// Read frames from a desktop `stream` until a goodbye arrives and return its reason.
fn desktop_goodbye(stream: &mut Connection) -> GoodbyeReason {
    loop {
        match frame::read_frame::<DesktopFrame>(stream).unwrap() {
            Some(DesktopFrame::Goodbye { reason }) => return reason,
            Some(_) => {}
            None => panic!("the desktop stream closed with no goodbye"),
        }
    }
}

/// Where in what an extension saw each request for `primitive` came, with its tab.
fn requests_at(
    seen: &crate::browser::adapter::chromium::protocol4::fake::Seen,
    primitive: &str,
) -> Vec<(usize, u64)> {
    seen.frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| frame["kind"] == "request")
        .filter(|(_, frame)| frame["primitive"]["name"] == primitive)
        .map(|(at, frame)| {
            (
                at,
                frame["primitive"]["args"]["tab_id"]
                    .as_u64()
                    .unwrap_or_default(),
            )
        })
        .collect()
}

/// An opening for the shell of this build.
fn desktop_opening() -> RelayOpening {
    RelayOpening::new(
        Role::Desktop,
        Edge::new(Program::Desktop, env!("CARGO_PKG_VERSION").into()),
    )
}

/// Open a `desktop` stream and read the frame that hands over the workbench.
fn open_desktop() -> (Connection, DesktopFrame) {
    let (mut connection, reply) = dial(&desktop_opening());
    assert!(reply.accepted, "{reply:?}");
    let handed = frame::read_frame::<DesktopFrame>(&mut connection)
        .unwrap()
        .expect("the first frame hands over the workbench");
    (connection, handed)
}

/// Open a `desktop` stream and wait until the service has attached it, so a request made
/// next goes to it.
fn attached_shell(host: &ServiceHost) -> Connection {
    let before = host.quit.shells().attached();
    let (stream, _) = open_desktop();
    eventually(|| host.quit.shells().attached() > before);
    stream
}

/// What a shell answers.
fn answer(stream: &mut Connection, id: u64, outcome: ActOutcome) {
    frame::write_frame(stream, &DesktopFrame::ActResult { id, outcome }).unwrap();
}
