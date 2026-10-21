//! Tests of the workbench server over real loopback sockets.
//!
//! These are the only tests that put a socket against the loopback server: every journey under
//! `tests/` talks to the preview server instead. Each test starts a server over its own facade and
//! temporary home, and talks to it the way a browser tab (no bearer) or the window (this start's
//! bearer) would. A server takes the default port of its home or, when another test holds it, a
//! port the system picks; no test depends on which.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::http::{header, HEAD_LIMIT, READ_DEADLINE};
use super::routes::{
    API_CACHE, BEARER_SCHEME, DENY_FRAMES, IMAGE_POLICY, NOSNIFF, NO_REFERRER, PAGE_POLICY,
    POLICY_BODY, SAME_ORIGIN_ONLY, SMALL_BODY,
};
use super::*;
use crate::browser::adapter::RuntimeControlState;
use crate::governance::GovernanceFacade;
use crate::install::HarnessRegistry;
use crate::language::coverage::{Coverage, HumanCoverage};
use crate::language::workbench as words;
use crate::scratch::Scratch;
use crate::workbench::context::{WebAct, WebContext};
use crate::workbench::parts::Part;
use crate::workbench::setup::test_support::Computer;
use crate::workbench::tests::RecordingActs;
use crate::workbench::PageAddress;
use crate::workbench::WorkbenchDestination;
use crate::workbench::WorkbenchPresentationPort;
use crate::workbench::WorkbenchQuitPort;
use crate::workbench::POLICY_DOCUMENT_LIMIT;
use crate::workbench::{WorkbenchNotification, WorkbenchPresentationError, WorkbenchProjection};

const EXCLUDED_HOST: &str = "private-frame.invalid";
const SACRED_HOST: &str = "never-touch.invalid";
/// Marks a fixture that starts with no user policy file.
const NO_POLICY: &str = "";
const READ_POLICY: &str = r#"{"schema":4,"name":"read","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[]}"#;

/// The page's transport: every route the page calls and the names it shares with the service.
const PAGE_TRANSPORT: &str = include_str!("../../../ui/lib/transport.js");

/// The text of the page's `const <name> = "...";` in transport.js.
fn page_constant(name: &str) -> &'static str {
    let opening = format!("const {name} = \"");
    let (_, rest) = PAGE_TRANSPORT
        .split_once(&opening)
        .unwrap_or_else(|| panic!("transport.js has no {name}"));
    rest.split_once('"').expect("the constant is quoted").0
}

/// One parsed response.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!(
                "not JSON ({error}): {}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    /// The words a refusal says.
    fn message(&self) -> Value {
        self.json()["error"]["message"].clone()
    }

    /// The code a refusal carries, for the one code the page reads.
    fn code(&self) -> Value {
        self.json()["error"]["code"].clone()
    }
}

/// How long a test waits for an answer the server owes it. Only a broken server reaches it.
const ANSWER_DEADLINE: Duration = Duration::from_secs(15);

/// Send exactly `text` and read the answer until the server closes.
fn raw(port: u16, text: &str) -> Reply {
    raw_within(port, text, ANSWER_DEADLINE)
}

/// Send exactly `text` and read the answer until the server closes, waiting at most `within`.
fn raw_within(port: u16, text: &str, within: Duration) -> Reply {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream.set_read_timeout(Some(within)).unwrap();
    stream.write_all(text.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    let _ = stream.read_to_end(&mut bytes);
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no response: {}", String::from_utf8_lossy(&bytes)));
    let head = String::from_utf8(bytes[..split].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(": ").unwrap();
            (name.to_owned(), value.to_owned())
        })
        .collect();
    Reply {
        status,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

/// A request builder: the host is the server's own unless a test says otherwise.
struct Call {
    method: &'static str,
    path: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    host: Option<String>,
    content_type: Option<String>,
}

impl Call {
    fn new(method: &'static str, path: &str) -> Self {
        Self {
            method,
            path: path.to_owned(),
            headers: Vec::new(),
            body: None,
            host: None,
            content_type: Some("application/json".to_owned()),
        }
    }

    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_owned());
        self
    }

    fn host(mut self, host: &str) -> Self {
        self.host = Some(host.to_owned());
        self
    }

    fn content_type(mut self, content_type: &str) -> Self {
        self.content_type = Some(content_type.to_owned());
        self
    }

    fn bearer(self, bearer: &str) -> Self {
        self.header(header::AUTHORIZATION, &format!("Bearer {bearer}"))
    }

    /// The request as it goes on the wire to the server at `port`.
    fn text(&self, port: u16) -> String {
        let host = self
            .host
            .clone()
            .unwrap_or_else(|| format!("127.0.0.1:{port}"));
        let mut text = format!("{} {} HTTP/1.1\r\nHost: {host}\r\n", self.method, self.path);
        for (name, value) in &self.headers {
            text.push_str(&format!("{name}: {value}\r\n"));
        }
        if let Some(body) = &self.body {
            if let Some(content_type) = &self.content_type {
                text.push_str(&format!("Content-Type: {content_type}\r\n"));
            }
            text.push_str(&format!("Content-Length: {}\r\n", body.len()));
        }
        text.push_str("\r\n");
        text.push_str(self.body.as_deref().unwrap_or_default());
        text
    }

    fn send(&self, port: u16) -> Reply {
        raw(port, &self.text(port))
    }
}

/// A presentation port that records each time the window was asked to come forward, and where.
#[derive(Default)]
struct CountingWindow(std::sync::Mutex<Vec<Option<PageAddress>>>);

impl CountingWindow {
    fn asked(&self) -> Vec<Option<PageAddress>> {
        self.0.lock().unwrap().clone()
    }
}

impl WorkbenchPresentationPort for CountingWindow {
    fn reveal(&self, address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
        self.0.lock().unwrap().push(address);
        Ok(())
    }

    fn notify(&self, _: WorkbenchNotification) -> Result<(), WorkbenchPresentationError> {
        Ok(())
    }
}

/// A quit port that counts how often the service was asked to end.
#[derive(Default)]
struct CountingQuit(std::sync::atomic::AtomicUsize);

impl CountingQuit {
    fn asked(&self) -> usize {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl WorkbenchQuitPort for CountingQuit {
    fn quit(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The ports a test watches for any act that reached them.
struct Watched {
    acts: Arc<RecordingActs>,
    quit: Arc<CountingQuit>,
    window: Arc<CountingWindow>,
}

/// A server over its own facade and temporary home.
struct Fixture {
    server: WorkbenchServer,
    facade: WorkbenchFacade,
    /// The temporary home, removed after the server and the facade drop.
    root: Scratch,
    /// The fake computer an unfinished setup is read from, when the fixture tracks setup.
    computer: Option<Arc<Computer>>,
}

impl Fixture {
    fn start() -> Self {
        Self::build(true, NO_POLICY, false)
    }

    /// A fixture whose setup is unfinished and read from a fake computer that counts its reads.
    fn setting_up() -> Self {
        Self::build(true, NO_POLICY, true)
    }

    fn closed() -> Self {
        Self::build(false, NO_POLICY, false)
    }

    fn with_policy(policy: &str) -> Self {
        Self::build(true, policy, false)
    }

    fn build(watching: bool, policy: &str, setting_up: bool) -> Self {
        let root = Scratch::named("web");
        let state = root.join("state");
        std::fs::create_dir_all(&state).unwrap();
        let marker = state.join("watch.off");
        if !watching {
            std::fs::write(&marker, b"").unwrap();
        }
        let policy_path = root.join("user-policy.json");
        if policy != NO_POLICY {
            std::fs::write(&policy_path, policy).unwrap();
        }
        let facade = WorkbenchFacade::new(
            WorkbenchProjection::default(),
            crate::workspace::WorkspaceStore::default(),
            GovernanceFacade::owning_user_policy(policy_path),
            Arc::new(crate::browser::RelayBrowserPort::new(
                "epoch".into(),
                crate::browser::adapter::chromium::protocol4::assets::page_assets(),
            )),
            crate::diagnostics::DiagnosticsHub::birth(&root),
        )
        .with_harnesses(HarnessRegistry::isolated(
            &root.join("clients"),
            &root.join("ghostlight"),
            &state,
        ));
        let computer = setting_up.then(|| Arc::new(Computer::default()));
        let facade = match &computer {
            Some(computer) => {
                facade.attach_setup(crate::workbench::setup::SetupProgress::load(
                    &state.join("setup.json"),
                ));
                facade.with_setup_machine(computer.clone())
            }
            None => facade,
        };
        let server = PendingWorkbench::bind(&root).unwrap().serve(
            &facade,
            "instance_test".into(),
            Arc::new(Watching::load(marker)),
        );
        Self {
            server,
            facade,
            root,
            computer,
        }
    }

    /// The fake computer of a fixture that tracks setup.
    fn computer(&self) -> Arc<Computer> {
        self.computer.clone().expect("a fixture that tracks setup")
    }

    /// How many times the fake computer was read.
    fn reads(&self) -> usize {
        self.computer
            .as_ref()
            .map_or(0, |computer| computer.reads())
    }

    /// Attach ports that record every act, quit and window request that reaches them.
    fn watched(&self) -> Watched {
        let watched = Watched {
            acts: Arc::new(RecordingActs::default()),
            quit: Arc::new(CountingQuit::default()),
            window: Arc::new(CountingWindow::default()),
        };
        self.facade.attach_native_acts(watched.acts.clone());
        self.facade.attach_quit(watched.quit.clone());
        self.facade.attach_presentation(watched.window.clone());
        watched
    }

    fn user_policy_exists(&self) -> bool {
        self.root.join("user-policy.json").exists()
    }

    fn port(&self) -> u16 {
        self.server.address().port()
    }

    fn bearer(&self) -> String {
        self.server.bearer().to_owned()
    }

    /// A call with the right bearer: the window.
    fn control(&self, call: Call) -> Reply {
        call.bearer(&self.bearer()).send(self.port())
    }

    /// A call with no bearer: a browser tab.
    fn watch(&self, call: Call) -> Reply {
        call.send(self.port())
    }

    /// Publish a document coverage that excluded `host`.
    fn cover(&self, host: &str) {
        self.facade.projection.document_coverage(HumanCoverage {
            summary: "One frame was excluded.".into(),
            explanation: "Excluded by policy.".into(),
            invocation: "invocation_1".into(),
            coverage: Coverage::default(),
            excluded_hosts: vec![host.into()],
            proactive: false,
        });
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.stop();
    }
}

fn get(path: &str) -> Call {
    Call::new("GET", path)
}

fn post(path: &str, body: &str) -> Call {
    Call::new("POST", path).body(body)
}

fn runtime(intent: &str) -> Call {
    post("/api/runtime", &json!({ "intent": intent }).to_string())
}

fn runtime_state(fixture: &Fixture) -> RuntimeControlState {
    fixture.facade.look().status().service.runtime_state
}

fn set_viewing(fixture: &Fixture, watch: bool) -> Reply {
    fixture.control(
        Call::new("PUT", "/api/controls/viewing").body(&json!({ "watch": watch }).to_string()),
    )
}

/// The request that does `act`, so a new act must name its route here.
fn act_call(act: WebAct) -> Call {
    let policy = json!({ "document": READ_POLICY }).to_string();
    match act {
        WebAct::Search => post("/api/search", r#"{"query":"status"}"#),
        WebAct::CheckPolicy => post("/api/policy/preview", &policy),
        WebAct::Rescan => post("/api/integrations/rescan", "{}"),
        WebAct::ContinueInWindow => post("/api/window", r#"{"view":"workbench"}"#),
        WebAct::Hold => runtime("hold"),
        WebAct::Resume => runtime("resume"),
        WebAct::StartSession => runtime("start_session"),
        WebAct::EndSession => runtime("end_session"),
        WebAct::ApplyPolicy => Call::new("PUT", "/api/policy/user")
            .header(header::IF_MATCH, "none")
            .body(&policy),
        WebAct::RemovePolicy => {
            Call::new("DELETE", "/api/policy/user").header(header::IF_MATCH, "none")
        }
        WebAct::SetDiagnostics => {
            Call::new("PUT", "/api/controls/diagnostics").body(r#"{"on":true}"#)
        }
        WebAct::SetWatching => Call::new("PUT", "/api/controls/viewing").body(r#"{"watch":false}"#),
        WebAct::SetupAll => post("/api/integrations/setup-all", "{}"),
        WebAct::HarnessAct => post(
            "/api/integrations/act",
            r#"{"id":"cursor","action":"install","expected_state":"not_detected"}"#,
        ),
        WebAct::TestNotification => post("/api/notifications/test", "{}"),
        WebAct::Open => post("/api/open", r#"{"destination":"home"}"#),
        WebAct::RevealDiagnostics => post("/api/diagnostics/reveal", "{}"),
        WebAct::Locate => post("/api/integrations/locate", r#"{"id":"goose"}"#),
        WebAct::SetupAct => post(
            "/api/setup/act",
            r#"{"kind":"add_in_browser","browser":"chrome"}"#,
        ),
        WebAct::Quit => post("/api/quit", "{}"),
    }
}

/// A cached icon that starts like a PNG and cannot be decoded, so it is served as cached.
const UNREADABLE_PNG: &[u8] = b"\x89PNG\r\n\x1a\nzed";

/// A cache under `fixture` that has found Zed's icon, `bytes`, on a Linux computer.
fn zed_icon_cache(fixture: &Fixture, bytes: &[u8]) -> crate::install::icons::IconCache {
    use crate::install::icons::{IconCache, IconPlaces};

    let share = fixture.root.join("share");
    let entry = share.join("applications").join("dev.zed.Zed.desktop");
    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(&entry, "[Desktop Entry]\nName=Zed\nIcon=zed\n").unwrap();
    let png = share.join("icons/hicolor/64x64/apps/zed.png");
    std::fs::create_dir_all(png.parent().unwrap()).unwrap();
    std::fs::write(&png, bytes).unwrap();
    let icons = fixture.root.join("state").join("icons");
    let cache = IconCache::open(icons, IconPlaces::linux(vec![share]), None);
    assert!(cache.refresh());
    cache
}

/// Contract with the shell (tests/golden/desktop/workbench.json, which the shell's
/// `well_formed_bearer` reads in its golden test) and the bearer rule of docs/trust
/// security-overview "Window on loopback" (ADR-0199 amendment item 7): each start mints its own
/// bearer in the golden's shape, and the bearer appears in no Debug print of the server, its
/// shared state or a request head (f1b471a4), and in no response.
#[test]
fn each_start_makes_a_fresh_bearer_in_the_shells_shape_that_nothing_prints() {
    let one = Fixture::start();
    let two = Fixture::start();
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/desktop/workbench.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let shape = |bearer: &str| {
        (
            bearer.len(),
            bearer
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'),
        )
    };
    let pinned = shape(golden["bearer"].as_str().expect("the golden's bearer"));
    assert_eq!(shape(&one.bearer()), pinned);
    assert_eq!(shape(&two.bearer()), pinned);
    assert_ne!(one.bearer(), two.bearer());

    let bearer = one.bearer();
    let server = format!("{:?}", one.server);
    assert!(!server.contains(&bearer), "{server}");
    assert!(server.contains("[redacted]"), "{server}");
    let shared = format!("{:?}", one.server.shared);
    assert!(!shared.contains(&bearer), "{shared}");

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    peer.write_all(
        get("/api/context")
            .bearer(&bearer)
            .text(one.port())
            .as_bytes(),
    )
    .unwrap();
    let (accepted, _) = listener.accept().unwrap();
    let (_, head) = super::http::Incoming::read_head(accepted, READ_DEADLINE);
    let head = format!("{:?}", head.expect("a well-formed request"));
    assert!(!head.contains(&bearer), "{head}");
    assert!(head.contains(header::AUTHORIZATION), "{head}");

    for path in ["/api/context", "/api/snapshot", "/api/policy"] {
        let reply = one.control(get(path));
        assert_eq!(reply.status, 200, "{path}");
        assert!(
            !String::from_utf8_lossy(&reply.body).contains(&bearer),
            "{path}"
        );
    }
}

/// Protects access in three contexts (docs/1.0/ARCHITECTURE: Control with the per-start bearer,
/// Watch, Closed; docs/trust/security-overview "Window on loopback"; ADR-0199 amendment items 2,
/// 3 and 8): this start's bearer is the window; no bearer is a watcher while Others can watch is
/// on and Closed while it is off, read live at each request; any other Authorization is refused
/// on every path and never becomes a watcher. The context document tells the window it may do
/// every act and opens links through the service, and tells a watcher it may do exactly search,
/// check a policy, re-check clients, bring the window forward and pause, and opens links as
/// anchors. Each act's route answers a watcher exactly when that document says it may (SURF-8),
/// the controls part each context draws offers the same, a refused act changes nothing, and a
/// watcher's pause says it came from a watching page. While watching is off a tab gets the pages
/// and a context that holds only the product, and every data route, the stream and every agent
/// image answer that watching is off, identically for any id (7267812a), while the window keeps
/// control.
#[test]
fn access_is_decided_per_route_by_the_bearer_and_the_live_watching_switch() {
    let fixture = Fixture::setting_up();
    let computer = fixture.computer();
    computer.install("chrome");
    let watched = fixture.watched();
    let port = fixture.port();
    let links: serde_json::Map<String, Value> = WorkbenchDestination::all()
        .into_iter()
        .map(|destination| (destination.key().to_owned(), json!(destination.url())))
        .collect();
    assert!(links
        .values()
        .all(|url| url.as_str().is_some_and(|url| url.starts_with("https://"))));

    let window = fixture.control(get("/api/context")).json();
    assert_eq!(window["context"], WebContext::Control.token());
    assert_eq!(window["opener"], page_constant("OPENER_SERVICE"));
    assert_eq!(window["links"], Value::Object(links.clone()));
    for act in WebAct::ALL {
        assert_eq!(window["acts"][act.key()]["available"], true, "{act:?}");
    }

    // The trust page's list of what a watcher may do, written as the promise.
    let watcher_may = [
        WebAct::Search,
        WebAct::CheckPolicy,
        WebAct::Rescan,
        WebAct::ContinueInWindow,
        WebAct::Hold,
    ];
    let watcher = fixture.watch(get("/api/context")).json();
    assert_eq!(watcher["context"], WebContext::Watch.token());
    assert_ne!(watcher["opener"], page_constant("OPENER_SERVICE"));
    assert_eq!(watcher["links"], Value::Object(links.clone()));
    for act in WebAct::ALL {
        let offered = &watcher["acts"][act.key()];
        assert_eq!(offered["available"], watcher_may.contains(&act), "{act:?}");
        if !watcher_may.contains(&act) {
            assert_eq!(offered["why_not"], words::CHANGE_IN_WINDOW, "{act:?}");
        }
    }

    // The controls part Settings is drawn from offers what the context document does.
    let controls = format!("/api/snapshot?parts={}", Part::Controls.token());
    for (document, reply) in [
        (&window, fixture.control(get(&controls))),
        (&watcher, fixture.watch(get(&controls))),
    ] {
        let part = reply.json()["parts"][Part::Controls.token()].clone();
        let mut offered = 0;
        for section in part.as_object().expect("the controls part").values() {
            for (key, availability) in section
                .get("acts")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                assert_eq!(availability, &document["acts"][key.as_str()], "{key}");
                offered += 1;
            }
        }
        assert!(offered > 0, "the controls part offers acts");
    }

    // Any other Authorization is refused, everywhere, and never becomes a watcher.
    let bearer = fixture.bearer();
    let mut same_length = bearer.clone();
    let last = same_length.pop().unwrap();
    same_length.push(if last == 'A' { 'B' } else { 'A' });
    for path in [
        "/",
        "/app.js",
        "/api/context",
        "/api/snapshot",
        "/api/events",
    ] {
        for authorization in [
            "Bearer not-the-key".to_owned(),
            format!("Bearer {same_length}"),
            format!("Basic {bearer}"),
        ] {
            let reply = get(path)
                .header(header::AUTHORIZATION, &authorization)
                .send(port);
            let label = format!("{path} {authorization}");
            assert_eq!(reply.status, 401, "{label}");
            assert_eq!(
                reply.header(header::WWW_AUTHENTICATE),
                Some(BEARER_SCHEME),
                "{label}"
            );
            assert_eq!(reply.message(), words::WRONG_KEY, "{label}");
        }
    }

    // Each act's route answers a watcher exactly when the context document says it may.
    for act in WebAct::ALL {
        let reply = fixture.watch(act_call(act));
        if watcher_may.contains(&act) {
            assert_eq!(reply.status, 200, "{act:?}");
        } else {
            assert_eq!(reply.status, 401, "{act:?}");
            assert_eq!(reply.message(), watcher["acts"][act.key()]["why_not"]);
            assert_eq!(
                reply.header(header::WWW_AUTHENTICATE),
                Some(BEARER_SCHEME),
                "{act:?}"
            );
        }
    }
    assert_eq!(watched.acts.asked(), 0);
    assert_eq!(watched.quit.asked(), 0);
    assert_eq!(computer.started(), [], "a watcher starts no browser");
    assert!(fixture.facade.watching());
    assert_eq!(fixture.facade.diagnostics.directory(), None);
    assert!(!fixture.user_policy_exists());
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Held);
    assert_eq!(
        fixture.facade.look().status().runtime_note.as_deref(),
        Some(words::PAUSED_FROM_WATCHER)
    );

    // Watching goes off: from the next request on, a tab is closed and the window is not.
    let cache = zed_icon_cache(&fixture, UNREADABLE_PNG);
    fixture.facade.attach_icons(cache);
    assert_eq!(set_viewing(&fixture, false).status, 200);
    let closed = fixture.watch(get("/api/context"));
    assert_eq!(closed.status, 200);
    let closed = closed.json();
    assert_eq!(closed["context"], page_constant("CONTEXT_CLOSED"));
    assert_eq!(closed["context"], WebContext::Closed.token());
    let mut keys: Vec<&String> = closed.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["context", "links", "product", "sentence", "word"]);
    assert_eq!(closed["product"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(closed["links"], Value::Object(links));
    assert_eq!(
        fixture.control(get("/api/context")).json()["context"],
        WebContext::Control.token()
    );
    for path in ["/", "/app.js"] {
        assert_eq!(fixture.watch(get(path)).status, 200, "{path}");
    }
    let reads = [
        get("/api/snapshot"),
        get("/api/snapshot?parts=setup"),
        get("/api/policy"),
        get("/api/events"),
    ];
    for call in WebAct::ALL.into_iter().map(act_call).chain(reads) {
        let label = format!("{} {}", call.method, call.path);
        let reply = fixture.watch(call);
        assert_eq!(reply.status, 403, "{label}");
        assert_eq!(reply.message(), words::WATCHING_OFF, "{label}");
    }
    let images: Vec<Reply> = [
        "/agent-icons/zed",
        "/agent-icons/codex",
        "/agent-icons/not-an-agent",
    ]
    .into_iter()
    .map(|path| fixture.watch(get(path)))
    .collect();
    for image in &images {
        assert_eq!(image.status, 403);
        assert_eq!(image.message(), words::WATCHING_OFF);
    }
    assert!(
        images.windows(2).all(|pair| pair[0].body == pair[1].body),
        "every id is refused alike, so a refusal says nothing of what is installed"
    );
    assert_eq!(
        fixture.control(get("/agent-icons/zed")).body,
        UNREADABLE_PNG,
        "the window draws its agents either way"
    );
    assert_eq!(watched.acts.asked(), 0);
    assert_eq!(watched.quit.asked(), 0);
    assert_eq!(computer.started(), []);
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Held);

    // And on again: a tab watches from the next request.
    assert_eq!(set_viewing(&fixture, true).status, 200);
    assert_eq!(
        fixture.watch(get("/api/context")).json()["context"],
        WebContext::Watch.token()
    );
}

/// One block of a stream: an event's name and data, or a comment.
#[derive(Debug, Default)]
struct Block {
    event: Option<String>,
    data: Option<String>,
}

impl Block {
    fn json(&self) -> Value {
        serde_json::from_str(self.data.as_deref().expect("the block carries data"))
            .expect("the data is JSON")
    }

    /// The kind of change a block carries, when it carries one.
    fn kind(&self) -> Option<String> {
        self.json()["change"]["kind"].as_str().map(str::to_owned)
    }

    fn is(&self, kind: &str) -> bool {
        self.data.is_some() && self.event.is_none() && self.kind().as_deref() == Some(kind)
    }
}

/// An open `GET /api/events`, read the way a page reads it.
struct Stream {
    reader: std::io::BufReader<TcpStream>,
}

impl Stream {
    /// Open a stream that must be accepted, and read its head.
    fn open(port: u16, bearer: Option<&str>) -> Self {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream.set_read_timeout(Some(ANSWER_DEADLINE)).unwrap();
        let mut request = format!("GET /api/events HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
        if let Some(bearer) = bearer {
            request.push_str(&format!("Authorization: Bearer {bearer}\r\n"));
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut reader = std::io::BufReader::new(stream);
        let mut status = String::new();
        std::io::BufRead::read_line(&mut reader, &mut status).unwrap();
        assert!(status.starts_with("HTTP/1.1 200"), "{status}");
        loop {
            let mut line = String::new();
            std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
            if line.trim_end().is_empty() {
                break;
            }
        }
        Self { reader }
    }

    /// The next block, or none when the stream ended or stayed quiet for the read timeout.
    fn next(&mut self) -> Option<Block> {
        let mut block = Block::default();
        let mut any = false;
        loop {
            let mut line = String::new();
            match std::io::BufRead::read_line(&mut self.reader, &mut line) {
                Ok(0) | Err(_) => return None,
                Ok(_) => {}
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if any {
                    return Some(block);
                }
                continue;
            }
            any = true;
            if let Some(value) = line.strip_prefix("event: ") {
                block.event = Some(value.to_owned());
            } else if let Some(value) = line.strip_prefix("data: ") {
                block.data = Some(value.to_owned());
            } else if !line.starts_with(':') && !line.starts_with("id: ") {
                panic!("not a stream line: {line}");
            }
        }
    }

    /// Read until a block satisfies `wanted`, and return it.
    fn until(&mut self, wanted: impl Fn(&Block) -> bool) -> Block {
        loop {
            let block = self.next().expect("the stream ended before the block came");
            if wanted(&block) {
                return block;
            }
        }
    }

    fn hello(&mut self) -> Value {
        let block = self.next().expect("a stream opens with a hello");
        assert_eq!(block.event.as_deref(), Some("hello"));
        block.json()
    }
}

/// Protects the projection at the wire (docs/1.0/ARCHITECTURE "projects each part for its
/// audience in the service"; docs/trust/security-overview: the never-touch list and excluded
/// hosts show only as counts) and reproduces f1b471a4 (a watcher was given the policy's tag to
/// test guesses against): the snapshot, the policy, search and the live stream each hand a
/// watcher the watcher's projection, with no excluded or never-touched host, no policy text and
/// no tag, while the window is given them.
#[test]
fn every_read_a_watcher_makes_is_projected_for_a_watcher() {
    let policy = format!(
        r#"{{"schema":4,"name":"mine","version":"1","grants":[{{"id":"read","hosts":{{"allow":["*"]}},"allowed":["read"]}}],"config":[{{"key":"content.security.sacred_domains","value":["{SACRED_HOST}","second.invalid"],"level":"mandatory"}}]}}"#
    );
    let fixture = Fixture::with_policy(&policy);
    let mut tab = Stream::open(fixture.port(), None);
    tab.hello();
    fixture.cover(EXCLUDED_HOST);

    let streamed = tab.until(|block| block.is("document_coverage_changed"));
    assert!(!streamed.data.as_deref().unwrap().contains(EXCLUDED_HOST));
    assert_eq!(
        streamed.json()["change"]["details"]["excluded_host_count"],
        1
    );

    let activity = format!("/api/snapshot?parts={}", Part::Activity.token());
    let seen = fixture.watch(get(&activity));
    assert!(!String::from_utf8_lossy(&seen.body).contains(EXCLUDED_HOST));
    assert_eq!(
        seen.json()["parts"]["activity"]["document_coverage"][0]["excluded_host_count"],
        1
    );
    let shown = fixture.control(get(&activity));
    assert!(String::from_utf8_lossy(&shown.body).contains(EXCLUDED_HOST));

    let seen = fixture.watch(get("/api/policy"));
    let text = String::from_utf8_lossy(&seen.body);
    assert!(!text.contains(SACRED_HOST) && !text.contains("second.invalid"));
    let seen = seen.json();
    assert_eq!(seen["layers"][0]["document"], Value::Null);
    assert!(seen.get("etag").is_none());
    assert_eq!(seen["layers"][0]["settings"][0]["count"], 2);
    assert_eq!(
        seen["ceilings"].as_array().unwrap().last().unwrap(),
        &json!(words::never_touch_count(2))
    );
    let shown = fixture.control(get("/api/policy")).json();
    let tag = shown["etag"].as_str().expect("the window's tag").to_owned();
    assert!(shown["layers"][0]["document"]
        .as_str()
        .unwrap()
        .contains(SACRED_HOST));

    let found = fixture.watch(post(
        "/api/search",
        &json!({ "query": EXCLUDED_HOST }).to_string(),
    ));
    assert_eq!(found.status, 200);
    assert_eq!(found.json(), json!([]));

    let written = fixture.control(
        Call::new("PUT", "/api/policy/user")
            .header(header::IF_MATCH, &format!("\"{tag}\""))
            .body(&json!({ "document": READ_POLICY }).to_string()),
    );
    assert_eq!(written.status, 200);
    let hint =
        tab.until(|block| block.is("part_changed") && block.json()["change"]["part"] == "policy");
    assert!(hint.json()["change"].get("etag").is_none());
}

/// Reproduces ec0f61f4: every GET /api/snapshot built the setup part, so any loopback caller
/// could repeat the native-host and profile reads without a limit. The route builds the part
/// only for a request that names it, and never for a watcher.
#[test]
fn only_a_request_for_the_setup_part_reads_this_computer() {
    let fixture = Fixture::setting_up();
    let before = fixture.reads();
    let without_setup: Vec<&str> = Part::ALL
        .into_iter()
        .filter(|part| *part != Part::Setup)
        .map(Part::token)
        .collect();
    let setup = format!("/api/snapshot?parts={}", Part::Setup.token());
    for call in [
        format!("/api/snapshot?parts={}", Part::Status.token()),
        format!("/api/snapshot?parts={}", without_setup.join(",")),
    ] {
        assert_eq!(fixture.control(get(&call)).status, 200, "{call}");
    }
    assert_eq!(
        fixture.watch(get("/api/snapshot")).status,
        200,
        "a watcher is shown no setup part"
    );
    let hidden = fixture.watch(get(&setup));
    assert_eq!(hidden.status, 200);
    assert!(hidden.json()["parts"].as_object().unwrap().is_empty());
    assert_eq!(
        fixture.reads(),
        before,
        "no snapshot that leaves the setup part out, or hides it, reads the computer"
    );

    let asked = fixture.control(get(&setup)).json();
    assert!(asked["parts"][Part::Setup.token()].is_object());
    assert!(
        fixture.reads() > before,
        "asking for the setup part reads it"
    );
}

/// What a request the gate refuses must be answered with.
enum Answer {
    /// A refusal that says these words.
    Says(&'static str),
    /// A redirect to this address.
    MovesTo(String),
    /// The request passes the gate.
    Passes,
}

/// Protects the loopback gate (docs/trust/security-overview "Window on loopback"; ADR-0199
/// amendment items 2 and 7; ADR-0077 Decision 3): a request the gate does not admit gets the
/// gate's answer and no route runs. Another Host is misdirected, which defeats DNS rebinding, and
/// a page read for localhost:<port> moves to the same path on the loopback address; a request the
/// server cannot read exactly (an upgrade, a chunked body, another HTTP version, an absolute
/// target, a repeated access header, a malformed line) is a bad request; a path or method outside
/// the table is not found or not allowed, whatever its encoding; a change that is not JSON or not
/// from this origin is refused even with the bearer; a head over its bound is refused, and a
/// declared body over its route's cap is refused from the head alone. A typed same-origin change
/// and a body-less DELETE pass. And the table is exactly the routes the page calls (transport.js
/// `ROUTES` and `EVENTS_PATH`), each with its method, so a route the page calls is never outside
/// the gate and the gate admits no route the page does not call.
#[test]
fn a_request_the_gate_does_not_admit_is_answered_before_any_route_runs() {
    let fixture = Fixture::start();
    let watched = fixture.watched();
    let port = fixture.port();
    let bearer = fixture.bearer();
    let own = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    let other_port = port.wrapping_add(1);
    let hold = || runtime("hold");
    let mut rows: Vec<(String, u16, Answer)> = Vec::new();

    for path in ["/", "/lib/view.js"] {
        rows.push((
            get(path).host(&localhost).text(port),
            308,
            Answer::MovesTo(format!("http://{own}{path}")),
        ));
    }
    let mut misdirected = vec![
        get("/api/context").host(&localhost).text(port),
        post("/api/search", r#"{"query":"x"}"#)
            .host(&localhost)
            .text(port),
        "GET /api/context HTTP/1.1\r\nConnection: close\r\n\r\n".to_owned(),
        "GET / HTTP/1.1\r\n\r\n".to_owned(),
    ];
    for host in [
        format!("127.0.0.1:{other_port}"),
        "evil.example".to_owned(),
        format!("evil.example:{port}"),
        "127.0.0.1".to_owned(),
        format!("[::1]:{port}"),
    ] {
        misdirected.push(get("/").host(&host).text(port));
        misdirected.push(get("/api/context").host(&host).text(port));
        misdirected.push(hold().host(&host).bearer(&bearer).text(port));
    }
    rows.extend(
        misdirected
            .into_iter()
            .map(|text| (text, 421, Answer::Says(words::MISDIRECTED))),
    );

    let unreadable = [
        get("/api/context")
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .text(port),
        post("/api/search", "0\r\n\r\n")
            .header("Transfer-Encoding", "chunked")
            .bearer(&bearer)
            .text(port),
        "NOT AN HTTP REQUEST\r\n\r\n".to_owned(),
        format!("GET / HTTP/2\r\nHost: {own}\r\n\r\n"),
        format!("GET\r\nHost: {own}\r\n\r\n"),
        format!("GET / HTTP/1.1 extra\r\nHost: {own}\r\n\r\n"),
        format!("GET http://evil.example/ HTTP/1.1\r\nHost: {own}\r\n\r\n"),
        format!("GET /a b HTTP/1.1\r\nHost: {own}\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {own}\r\nNo-Colon\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {own}\r\n folded: a\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {own}\r\nHost: {own}\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {own}\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {own}\r\nContent-Length: -1\r\n\r\n"),
        format!(
            "GET /api/context HTTP/1.1\r\nHost: {own}\r\nAuthorization: Bearer {bearer}\r\nAuthorization: Bearer {bearer}\r\n\r\n"
        ),
        format!("GET / HTTP/1.1\r\nHost: {own}\r\nX-A: a\u{0001}b\r\n\r\n"),
        format!(
            "GET /api/context HTTP/1.1\r\nHost: {own}\r\nX-Padding: {}\r\n\r\n",
            "a".repeat(HEAD_LIMIT + 1024)
        ),
    ];
    rows.extend(
        unreadable
            .into_iter()
            .map(|text| (text, 400, Answer::Says(words::BAD_REQUEST))),
    );

    for path in [
        "/nothing",
        "/lib/../app.js",
        "/lib/%2e%2e/app.js",
        "/api/other",
        "/app.js%00",
        "/lib\\view.js",
        "/index.html",
        "/integrations/codex.svg",
        "/agent-icons/not-an-agent",
        "/agent-icons/command_line",
        "/agent-icons/",
        "/agent-icons/zed/more",
        "/agent-icons/..%2fzed",
        "/agent-icons/zed.json",
    ] {
        rows.push((get(path).text(port), 404, Answer::Says(words::NOT_FOUND)));
    }
    for call in [
        post("/api/context", "{}"),
        get("/api/search"),
        get("/api/policy/user"),
        post("/app.js", "{}"),
        Call::new("PATCH", "/api/context"),
        Call::new("OPTIONS", "/api/context")
            .header(header::ORIGIN, "https://evil.example")
            .header("Access-Control-Request-Method", "POST"),
        post("/agent-icons/zed", "{}"),
    ] {
        rows.push((
            call.text(port),
            405,
            Answer::Says(words::METHOD_NOT_ALLOWED),
        ));
    }

    rows.push((
        hold().content_type("text/plain").bearer(&bearer).text(port),
        415,
        Answer::Says(words::UNSUPPORTED_TYPE),
    ));
    for (name, value) in [
        (header::ORIGIN, "https://evil.example"),
        (header::ORIGIN, "http://localhost:41000"),
        (header::ORIGIN, "null"),
        (header::SEC_FETCH_SITE, "cross-site"),
        (header::SEC_FETCH_SITE, "same-site"),
    ] {
        for call in [hold(), hold().bearer(&bearer)] {
            rows.push((
                call.header(name, value).text(port),
                403,
                Answer::Says(words::CROSS_SITE),
            ));
        }
    }
    rows.push((
        post("/api/quit", "{}")
            .header(header::ORIGIN, "http://evil.example")
            .bearer(&bearer)
            .text(port),
        403,
        Answer::Says(words::CROSS_SITE),
    ));
    let search = || post("/api/search", r#"{"query":"x"}"#).bearer(&bearer);
    for call in [
        search().content_type("application/json; charset=utf-8"),
        search()
            .header(header::ORIGIN, &format!("http://{own}"))
            .header(header::SEC_FETCH_SITE, "same-origin"),
        search().header(header::SEC_FETCH_SITE, "none"),
        Call::new("DELETE", "/api/policy/user")
            .header(header::IF_MATCH, "none")
            .bearer(&bearer),
    ] {
        rows.push((call.text(port), 200, Answer::Passes));
    }

    for (text, status, answer) in &rows {
        let reply = raw(port, text);
        let label = text.lines().next().unwrap_or_default();
        assert_eq!(reply.status, *status, "{label}");
        match answer {
            Answer::Says(words) => assert_eq!(reply.message(), *words, "{label}"),
            Answer::MovesTo(location) => {
                assert_eq!(reply.header(header::LOCATION), Some(location.as_str()));
            }
            Answer::Passes => {}
        }
    }

    // A body over its cap is refused from the head: if the server waited for the body, the read
    // would outlast its bound.
    for (path, cap) in [
        ("/api/search", SMALL_BODY),
        ("/api/policy/preview", POLICY_BODY),
    ] {
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: {own}\r\nAuthorization: Bearer {bearer}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            cap + 1
        );
        let reply = raw_within(port, &head, READ_DEADLINE / 2);
        assert_eq!(reply.status, 413, "{path}");
        assert_eq!(reply.message(), words::TOO_LARGE, "{path}");
    }

    // U-D1: a policy document is refused for its size by its own limit and sentence, which a
    // document a byte over the limit reaches even with its quotes escaped.
    let over = "\"".repeat(POLICY_DOCUMENT_LIMIT + 1);
    let reply = fixture.control(post(
        "/api/policy/preview",
        &json!({ "document": over }).to_string(),
    ));
    assert_eq!(
        reply.message(),
        words::policy_too_large(POLICY_DOCUMENT_LIMIT).as_str()
    );

    assert_eq!(runtime_state(&fixture), RuntimeControlState::Active);
    assert_eq!(watched.quit.asked(), 0);
    assert_eq!(watched.acts.asked(), 0);
    assert!(!fixture.user_policy_exists());

    let mut called = page_routes();
    called.sort_unstable();
    called.dedup();
    let mut table: Vec<(&str, &str)> = super::routes::table()
        .into_iter()
        .map(|(method, path)| (method.token(), path))
        .collect();
    table.sort_unstable();
    assert_eq!(called, table, "the routes the page calls are the table");
}

/// Every route the page's transport calls, as (method, path without its query): each entry of
/// `ROUTES`, whose method names one of the page's method constants, and the stream at
/// `EVENTS_PATH`.
fn page_routes() -> Vec<(&'static str, &'static str)> {
    const METHOD: &str = "method: ";
    const PATH: &str = "path: ";
    let (_, table) = PAGE_TRANSPORT
        .split_once("const ROUTES = Object.freeze({")
        .expect("transport.js has ROUTES");
    let (mut table, _) = table.split_once("\n  });").expect("ROUTES ends");
    let mut routes = vec![(page_constant("GET"), page_constant("EVENTS_PATH"))];
    while let Some((_, rest)) = table.split_once(METHOD) {
        let (method, rest) = rest.split_once(',').expect("a method ends with a comma");
        let (_, rest) = rest
            .split_once(PATH)
            .expect("a method is followed by its path");
        let quote = rest.chars().next().expect("a quoted path");
        let (path, rest) = rest[1..].split_once(quote).expect("the path is quoted");
        let path = path.split_once('?').map_or(path, |(path, _)| path);
        routes.push((page_constant(method.trim()), path));
        table = rest;
    }
    routes
}

/// Protects ACCEPTANCE desktop gate 11 (the page loads under the service's restrictive CSP) and
/// the trust claims "sends no CORS header" and "refuses embedding": every response, refused ones
/// included, carries no `access-control-*` header, nosniff, no-referrer, a same-origin resource
/// policy and `Connection: close` with a length; the page refuses framing under its CSP; a data
/// response is not stored; an agent image runs nothing.
#[test]
fn no_response_can_be_read_by_another_site_and_nothing_served_can_be_framed_or_run() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let replies = [
        get("/").send(port),
        get("/app.js").send(port),
        get("/api/context").send(port),
        get("/agent-icons/zed").send(port),
        get("/nothing").send(port),
        Call::new("OPTIONS", "/").send(port),
        post("/api/search", r#"{"query":"x"}"#)
            .header(header::ORIGIN, "https://evil.example")
            .send(port),
        get("/api/context").host("evil.example").send(port),
        raw(port, "garbage\r\n\r\n"),
    ];
    for reply in &replies {
        for (name, _) in &reply.headers {
            assert!(
                !name.to_ascii_lowercase().starts_with("access-control-"),
                "{name}"
            );
        }
        assert_eq!(reply.header(header::NOSNIFF), Some(NOSNIFF));
        assert_eq!(reply.header(header::REFERRER_POLICY), Some(NO_REFERRER));
        assert_eq!(
            reply.header(header::RESOURCE_POLICY),
            Some(SAME_ORIGIN_ONLY)
        );
        assert_eq!(reply.header(header::CONNECTION), Some("close"));
        assert_eq!(
            reply.header(header::CONTENT_LENGTH),
            Some(reply.body.len().to_string().as_str())
        );
    }

    let [index, _, api, image, ..] = &replies;
    assert_eq!(index.header(header::FRAME_OPTIONS), Some(DENY_FRAMES));
    let policy = index.header(header::CONTENT_SECURITY_POLICY).unwrap();
    assert_eq!(policy, PAGE_POLICY);
    for directive in [
        "frame-ancestors 'none'",
        "object-src 'none'",
        "script-src 'self'",
    ] {
        assert!(policy.contains(directive), "{directive}");
    }
    assert!(!policy.contains("unsafe-inline"));
    assert_eq!(api.header(header::CACHE_CONTROL), Some(API_CACHE));
    assert_eq!(image.status, 200);
    let policy = image.header(header::CONTENT_SECURITY_POLICY).unwrap();
    assert_eq!(policy, IMAGE_POLICY);
    assert!(policy.contains("sandbox"));
}

/// Protects the act-to-stream numbering the page waits on (transport.js `SEQ_HEADER`, app.js
/// waiting for that number, store.js treating any jump as a gap): a stream's hello names the
/// current number, every change after it is numbered one higher than the last, and an act that
/// happened answers in `Ghostlight-Seq` the number of its last change; a read and a refused act
/// answer none.
#[test]
fn an_act_answers_with_the_number_of_the_last_change_it_caused() {
    assert_eq!(header::SEQ, page_constant("SEQ_HEADER"));
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    let mut last = window.hello()["seq"].as_u64().unwrap();
    assert_eq!(last, fixture.facade.current_seq());

    let reply = fixture.control(runtime("hold"));
    assert_eq!(reply.status, 200);
    let caused: u64 = reply.header(header::SEQ).unwrap().parse().unwrap();
    assert_eq!(caused, fixture.facade.current_seq());
    while last < caused {
        let seq = window.next().expect("the act's changes arrive").json()["seq"]
            .as_u64()
            .unwrap();
        assert_eq!(seq, last + 1, "the numbers have no gap");
        last = seq;
    }
    assert_eq!(
        fixture.facade.current_seq(),
        caused,
        "the act caused no later change"
    );

    assert!(fixture
        .control(get("/api/snapshot"))
        .header(header::SEQ)
        .is_none());
    let refused = fixture.control(runtime("nonsense"));
    assert_eq!(refused.status, 400);
    assert!(refused.header(header::SEQ).is_none());
}

/// Protects the stale contract with the page (app.js reads transport.js `STALE`): a rules write
/// without the tag the caller read is 428 and with another tag 412, and a client act against a
/// state the page did not draw is 409, each with the page's stale code and nothing written; a
/// write carrying the tag just read, quoted as the page sends it, succeeds.
#[test]
fn a_change_drawn_from_a_stale_view_is_refused_as_stale_and_changes_nothing() {
    let stale = page_constant("STALE");
    let fixture = Fixture::start();
    let document = json!({ "document": READ_POLICY }).to_string();
    let quoted = |tag: &str| format!("\"{tag}\"");
    let put = |tag: Option<&str>| {
        let mut call = Call::new("PUT", "/api/policy/user").body(&document);
        if let Some(tag) = tag {
            call = call.header(header::IF_MATCH, tag);
        }
        fixture.control(call)
    };
    let remove = |tag: Option<&str>| {
        let mut call = Call::new("DELETE", "/api/policy/user");
        if let Some(tag) = tag {
            call = call.header(header::IF_MATCH, tag);
        }
        fixture.control(call)
    };
    let refused = |reply: Reply, status: u16| {
        assert_eq!(reply.status, status);
        assert_eq!(reply.code(), stale);
        assert_eq!(reply.message(), words::STALE_POLICY);
    };

    refused(put(None), 428);
    refused(put(Some(&quoted("0123"))), 412);
    assert!(!fixture.user_policy_exists());

    let read = fixture.control(get("/api/policy")).json()["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    let written = put(Some(&quoted(&read)));
    assert_eq!(
        written.status,
        200,
        "{}",
        String::from_utf8_lossy(&written.body)
    );
    assert!(fixture.user_policy_exists());
    let now = fixture.control(get("/api/policy")).json()["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(now, read);

    refused(put(Some(&quoted(&read))), 412);
    refused(remove(None), 428);
    refused(remove(Some(&quoted(&read))), 412);
    assert!(fixture.user_policy_exists());
    assert_eq!(remove(Some(&quoted(&now))).status, 200);
    assert!(!fixture.user_policy_exists());

    let first = fixture.facade.cached_harnesses().remove(0);
    let drawn = if serde_json::to_value(first.state).unwrap() == "installed" {
        "available"
    } else {
        "installed"
    };
    let reply = fixture.control(post(
        "/api/integrations/act",
        &json!({ "id": first.id, "action": "install", "expected_state": drawn }).to_string(),
    ));
    assert_eq!(reply.status, 409);
    assert_eq!(reply.code(), stale);
    assert_eq!(reply.message(), words::STALE_HARNESS);
    assert_eq!(fixture.facade.cached_harnesses()[0].state, first.state);
}

/// Reproduces f1b471a4 (the rescan route took any body) and guards the rule it restored: every
/// route decodes its body as exactly its own arguments, so anything else is a bad request that
/// opens no URL, reaches no window, starts no browser and changes no state.
#[test]
fn every_route_takes_exactly_its_arguments_and_anything_else_changes_nothing() {
    let fixture = Fixture::setting_up();
    let computer = fixture.computer();
    computer.install("chrome");
    let watched = fixture.watched();
    let mut calls: Vec<Call> = Vec::new();
    for body in [
        "",
        "not json",
        "[]",
        "\"query\"",
        r#"{"query":1}"#,
        r#"{"query":"x","extra":1}"#,
        "{}",
    ] {
        calls.push(post("/api/search", body));
    }
    for body in ["", "not json", "[]", "\"x\"", r#"{"x":1}"#] {
        calls.push(post("/api/integrations/rescan", body));
        calls.push(post("/api/integrations/setup-all", body));
    }
    for route in [
        "/api/quit",
        "/api/notifications/test",
        "/api/diagnostics/reveal",
    ] {
        calls.push(post(route, r#"{"x":1}"#));
    }
    for body in [r#"{"watch":"yes"}"#, "{}", r#"{"watch":false,"extra":1}"#] {
        calls.push(Call::new("PUT", "/api/controls/viewing").body(body));
    }
    calls.push(Call::new("PUT", "/api/controls/diagnostics").body(r#"{"on":"yes"}"#));
    calls.push(
        Call::new("PUT", "/api/policy/user")
            .header(header::IF_MATCH, "none")
            .body("{}"),
    );
    calls.push(post("/api/policy/preview", r#"{"document":1}"#));
    calls.push(post("/api/window", r#"{"view":"setup"}"#));
    for address in [
        json!("settings/nowhere"),
        json!("#settings"),
        json!(""),
        json!("monitor\"; alert(1); \""),
        json!(7),
        json!(["monitor"]),
    ] {
        calls.push(post(
            "/api/window",
            &json!({ "view": "workbench", "address": address }).to_string(),
        ));
    }
    for body in [
        r#"{"destination":"https://example.com"}"#,
        r#"{"destination":"home","harness":"x"}"#,
        "{}",
        r#"{"url":"https://example.com"}"#,
        r#"{"product":"cline"}"#,
        r#"{"destination":null,"harness":null}"#,
    ] {
        calls.push(post("/api/open", body));
    }
    calls.push(post(
        "/api/integrations/act",
        r#"{"id":"cursor","action":"check","expected_state":"x"}"#,
    ));
    calls.push(post("/api/integrations/locate", "{}"));
    for body in [
        "",
        "[]",
        "{}",
        r#"{"kind":"nothing"}"#,
        r#"{"kind":"add_in_browser"}"#,
    ] {
        calls.push(post("/api/setup/act", body));
    }
    calls.push(runtime("nonsense"));

    for call in calls {
        let label = format!(
            "{} {} {}",
            call.method,
            call.path,
            call.body.as_deref().unwrap_or_default()
        );
        let reply = fixture.control(call);
        assert_eq!(reply.status, 400, "{label}");
        assert_eq!(reply.message(), words::BAD_REQUEST, "{label}");
    }
    assert_eq!(watched.acts.asked(), 0);
    assert_eq!(watched.window.asked(), []);
    assert_eq!(watched.quit.asked(), 0);
    assert_eq!(computer.started(), []);
    assert!(fixture.facade.watching());
    assert_eq!(fixture.facade.diagnostics.directory(), None);
    assert!(!fixture.user_policy_exists());
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Active);
}

/// Every file below `directory`, at any depth.
fn files_below(directory: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_below(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Protects ACCEPTANCE release gate 10 (the bundled HTML, CSS and JS render): every file under
/// `crates/orchestrator/ui`, and each extension file the setup view shares, is served at its
/// path byte for byte with the media type its name says (a script or stylesheet served under
/// nosniff with another type does not run) and revalidates by its tag; the table serves nothing
/// else.
#[test]
fn every_file_the_page_ships_is_served_whole_with_the_type_its_name_says() {
    // The setup view shares these with the extension's own pages: served path, extension file.
    const SHARED: [(&str, &str); 2] = [
        ("/extension/kit.css", "kit.css"),
        (
            "/extension/ghostlight-mascot.png",
            "icons/ghostlight-mascot.png",
        ),
    ];
    const TYPES: [(&str, &str); 4] = [
        ("html", "text/html; charset=utf-8"),
        ("css", "text/css; charset=utf-8"),
        ("js", "text/javascript; charset=utf-8"),
        ("png", "image/png"),
    ];

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ui = manifest.join("ui");
    let mut files = Vec::new();
    files_below(&ui, &mut files);
    let mut shipped: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|file| {
            let parts: Vec<String> = file
                .strip_prefix(&ui)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            let path = format!("/{}", parts.join("/"));
            let path = if path == "/index.html" {
                "/".to_owned()
            } else {
                path
            };
            (path, file)
        })
        .chain(
            SHARED
                .map(|(path, file)| (path.to_owned(), manifest.join("../../extension").join(file))),
        )
        .collect();
    shipped.sort();
    let mut served: Vec<&str> = super::assets::ASSETS
        .iter()
        .map(|asset| asset.path)
        .collect();
    served.sort_unstable();
    assert_eq!(
        served,
        shipped
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>()
    );

    let fixture = Fixture::start();
    let port = fixture.port();
    for (path, file) in &shipped {
        let extension = file.extension().unwrap().to_string_lossy();
        let kind = TYPES
            .iter()
            .find(|(name, _)| *name == extension)
            .unwrap_or_else(|| panic!("{path}: no type for .{extension}"))
            .1;
        let reply = get(path).send(port);
        assert_eq!(reply.status, 200, "{path}");
        assert_eq!(reply.header(header::CONTENT_TYPE), Some(kind), "{path}");
        assert_eq!(reply.body, std::fs::read(file).unwrap(), "{path}");
        let tag = reply.header(header::ETAG).unwrap().to_owned();
        let again = get(path).header(header::IF_NONE_MATCH, &tag).send(port);
        assert_eq!(again.status, 304, "{path}");
        assert!(again.body.is_empty(), "{path}");
    }
}

/// U-D6 (dc1c4fbf; RFC 9110 9.3.2): a HEAD answer carries no body, a refusal's included. A page
/// file and an agent image answer a HEAD with the status and length their read has. Each refused
/// read here carries its refusal in a JSON body; the HEAD is refused too (a data route takes no
/// HEAD), and only its headers go out.
#[test]
fn a_head_answer_carries_no_body_a_refusals_included() {
    let fixture = Fixture::closed();
    let port = fixture.port();
    let calls = [
        Call::new("GET", "/app.js"),
        Call::new("GET", "/agent-icons/zed").bearer(&fixture.bearer()),
        Call::new("GET", "/api/snapshot"),
        Call::new("GET", "/api/context").bearer("not-this-start"),
        Call::new("GET", "/nothing").bearer(&fixture.bearer()),
        Call::new("GET", "/api/context").host("example.invalid"),
        Call::new("GET", "/api/search").bearer(&fixture.bearer()),
    ];
    for read in calls {
        let path = read.path.clone();
        let full = read.send(port);
        let head = Call {
            method: "HEAD",
            ..read
        }
        .send(port);
        assert!(
            head.body.is_empty(),
            "{path}: {}",
            String::from_utf8_lossy(&head.body)
        );
        assert!(!full.body.is_empty(), "{path}: the read carries a body");
        if full.status == 200 {
            assert_eq!(head.status, 200, "{path}");
            assert_eq!(
                head.header(header::CONTENT_LENGTH),
                Some(full.body.len().to_string().as_str()),
                "{path}"
            );
        } else {
            assert!(full.status >= 400, "{path}: {}", full.status);
            assert!(head.status >= 400, "{path}: {}", head.status);
        }
    }
}

/// U-D5 (9db6e896): peers that only hold connections open never lock the window out. Idle peers
/// fill the share without a bearer; past it, a request without the bearer is closed unanswered
/// while the window's is answered, and once the idle peers go a tab is let in again.
#[test]
fn idle_peers_never_lock_the_window_out() {
    use std::sync::atomic::Ordering;

    let fixture = Fixture::start();
    let port = fixture.port();
    let places = Arc::clone(&fixture.server.shared.places);
    let settled = |served: usize, without_bearer: usize| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while places.served.load(Ordering::Acquire) != served
            || places.without_bearer.load(Ordering::Acquire) != without_bearer
        {
            assert!(Instant::now() < deadline, "{places:?}");
            std::thread::yield_now();
        }
    };
    let idle: Vec<TcpStream> = (0..super::MAX_WITHOUT_BEARER)
        .map(|_| TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap())
        .collect();
    settled(super::MAX_WITHOUT_BEARER, super::MAX_WITHOUT_BEARER);

    let mut stranger = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stranger
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let _ = stranger.write_all(
        format!("GET /api/context HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes(),
    );
    let mut answer = Vec::new();
    let read = stranger.read_to_end(&mut answer);
    assert!(answer.is_empty(), "{}", String::from_utf8_lossy(&answer));
    // Closed, not merely kept waiting past the read's bound.
    assert!(
        !read.as_ref().is_err_and(|error| matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        )),
        "{read:?}"
    );

    assert_eq!(fixture.control(get("/api/context")).status, 200);
    drop(idle);
    settled(0, 0);
    assert_eq!(fixture.watch(get("/api/context")).status, 200);
}
