//! Tests of the workbench server over real loopback sockets.
//!
//! Each test starts a server over its own facade and temporary home, and talks to it the way a
//! browser tab or the window would. A server takes the default port of its home or, when another
//! test holds it, a port the system picks; no test depends on which.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};

use super::*;
use crate::browser::adapter::RuntimeControlState;
use crate::governance::GovernanceFacade;
use crate::install::HarnessRegistry;
use crate::language::coverage::{Coverage, HumanCoverage};
use crate::language::workbench as words;
use crate::scratch::Scratch;
use crate::workbench::context::WebContext;
use crate::workbench::parts::{Part, USER_HOME_VARIABLE};
use crate::workbench::tests::RecordingActs;
use crate::workbench::PageAddress;
use crate::workbench::WorkbenchDestination;
use crate::workbench::WorkbenchPresentationPort;
use crate::workbench::WorkbenchQuitPort;
use crate::workbench::{WorkbenchNotification, WorkbenchPresentationError, WorkbenchProjection};

const EXCLUDED_HOST: &str = "private-frame.invalid";
const SACRED_HOST: &str = "never-touch.invalid";
/// Marks a fixture that starts with no user policy file.
const NO_POLICY: &str = "";
const READ_POLICY: &str = r#"{"schema":4,"name":"read","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[]}"#;

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

    fn code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

/// Send exactly `text` and read the answer until the server closes.
fn raw(port: u16, text: &str) -> Reply {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
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
        self.header("Authorization", &format!("Bearer {bearer}"))
    }

    fn send(&self, port: u16) -> Reply {
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
        raw(port, &text)
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

/// A server over its own facade and temporary home.
struct Fixture {
    server: WorkbenchServer,
    facade: WorkbenchFacade,
    /// The temporary home, removed after the server and the facade drop.
    root: Scratch,
    /// The fake computer an unfinished setup is read from, when the fixture tracks setup.
    computer: Option<Arc<crate::workbench::setup::test_support::Computer>>,
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
            GovernanceFacade::owning_user_policy(policy_path, None),
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
        let computer = setting_up
            .then(|| Arc::new(crate::workbench::setup::test_support::Computer::default()));
        let facade = match &computer {
            Some(computer) => {
                facade.attach_setup(crate::workbench::setup::SetupProgress::load(
                    &state.join("setup.json"),
                ));
                facade.with_setup_machine(computer.clone())
            }
            None => facade,
        };
        let server = WorkbenchServer::start(
            &root,
            &facade,
            "instance_test".into(),
            Arc::new(Watching::load(marker)),
        )
        .unwrap();
        Self {
            server,
            facade,
            root,
            computer,
        }
    }

    /// How many times the fake computer was read.
    fn reads(&self) -> usize {
        self.computer
            .as_ref()
            .map_or(0, |computer| computer.reads())
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

fn runtime_state(fixture: &Fixture) -> RuntimeControlState {
    fixture.facade.snapshot().service.runtime_state
}

#[test]
fn the_default_port_comes_from_the_home_and_never_takes_the_preview_port() {
    for name in [
        "a",
        "b",
        "C:\\Users\\Ann\\.ghostlight",
        "/home/ann/.ghostlight",
    ] {
        let home = Path::new(name);
        let port = default_port(home);
        assert!((41_000..=41_999).contains(&port), "{name}: {port}");
        assert_ne!(port, 41_737);
        assert_eq!(port, default_port(home));
    }
    assert_eq!(
        default_port(Path::new("C:\\USERS\\ANN\\.GHOSTLIGHT")),
        default_port(Path::new("c:\\users\\ann\\.ghostlight"))
    );
    let mut seen = std::collections::HashSet::new();
    for index in 0..200 {
        let port = default_port(Path::new(&format!("/home/user{index}/.ghostlight")));
        assert_ne!(port, 41_737);
        seen.insert(port);
    }
    assert!(seen.len() > 100, "the homes should spread over the range");
}

#[test]
fn a_taken_default_port_falls_back_to_one_the_system_picks() {
    let holder = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let taken = holder.local_addr().unwrap().port();
    let pending = PendingWorkbench::bind_preferring(taken).unwrap();
    assert!(!pending.address().usual());
    assert_ne!(pending.address().port(), taken);
    assert_eq!(
        pending.address().url(),
        format!("http://127.0.0.1:{}/", pending.address().port())
    );
}

#[test]
fn a_free_default_port_is_the_usual_one() {
    let free = {
        let probe = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        probe.local_addr().unwrap().port()
    };
    let pending = PendingWorkbench::bind_preferring(free).unwrap();
    assert!(pending.address().usual());
    assert_eq!(pending.address().port(), free);
}

#[test]
fn a_stopped_server_can_start_again_on_its_port_after_answering() {
    let fixture = Fixture::start();
    let port = fixture.port();
    assert_eq!(get("/api/context").send(port).status, 200);
    fixture.server.stop();
    let pending = PendingWorkbench::bind_preferring(port).unwrap();
    assert!(pending.address().usual());
    assert_eq!(pending.address().port(), port);
}

#[test]
fn secrets_compare_by_value_and_length() {
    assert!(same_secret(b"abc", b"abc"));
    assert!(!same_secret(b"abc", b"abd"));
    assert!(!same_secret(b"abc", b"ab"));
    assert!(!same_secret(b"", b"a"));
    assert!(same_secret(b"", b""));
}

#[test]
fn every_start_makes_its_own_bearer_of_forty_three_characters() {
    let one = Fixture::start();
    let two = Fixture::start();
    assert_eq!(one.bearer().len(), 43);
    assert_eq!(two.bearer().len(), 43);
    assert_ne!(one.bearer(), two.bearer());
    assert!(one
        .bearer()
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'));
}

#[test]
fn the_bearer_never_appears_in_what_the_server_prints() {
    let fixture = Fixture::start();
    let bearer = fixture.bearer();
    let printed = format!("{:?}", fixture.server);
    assert!(!printed.contains(&bearer));
    assert!(printed.contains("[redacted]"));
    let shared = format!("{:?}", fixture.server.shared);
    assert!(!shared.contains(&bearer));
}

/// The command table of the page's transport: every route the page calls, in the page's words.
const PAGE_TRANSPORT: &str = include_str!("../../../ui/lib/transport.js");

#[test]
fn the_route_table_is_closed_and_no_route_clears_history() {
    use super::http::Method::{Delete, Get, Post, Put};

    let mut table = routes::table();
    table.sort_by_key(|(method, path)| (*path, format!("{method:?}")));
    let mut expected = vec![
        (Get, "/api/context"),
        (Get, "/api/events"),
        (Get, "/api/policy"),
        (Get, "/api/snapshot"),
        (Post, "/api/diagnostics/reveal"),
        (Post, "/api/integrations/act"),
        (Post, "/api/integrations/locate"),
        (Post, "/api/integrations/rescan"),
        (Post, "/api/integrations/setup-all"),
        (Post, "/api/notifications/test"),
        (Post, "/api/open"),
        (Post, "/api/policy/preview"),
        (Post, "/api/quit"),
        (Post, "/api/runtime"),
        (Post, "/api/search"),
        (Post, "/api/setup/act"),
        (Post, "/api/window"),
        (Put, "/api/controls/diagnostics"),
        (Put, "/api/controls/viewing"),
        (Put, "/api/policy/user"),
        (Delete, "/api/policy/user"),
    ];
    expected.sort_by_key(|(method, path)| (*path, format!("{method:?}")));
    assert_eq!(table, expected, "a route was added or removed; say so here");
    for (_, path) in &table {
        for word in ["clear", "history", "audit", "purge", "wipe"] {
            assert!(
                !path.contains(word),
                "{path}: clearing the monitor is the page's own view state, never a route"
            );
        }
    }
}

#[test]
fn every_route_the_page_calls_is_in_the_table_and_the_table_serves_nothing_else_to_the_page() {
    let table = routes::table();
    let called: Vec<&str> = PAGE_TRANSPORT
        .split(['"', '`'])
        .filter(|text| text.starts_with("/api/"))
        .collect();
    assert!(!called.is_empty());
    for path in &called {
        let path = path.split('?').next().unwrap_or(path);
        assert!(
            table.iter().any(|(_, served)| *served == path),
            "the page calls {path}, which the server does not serve"
        );
    }
}

#[test]
fn the_right_bearer_makes_every_route_answer_as_control() {
    let fixture = Fixture::start();
    let context = fixture.control(get("/api/context")).json();
    assert_eq!(context["context"], "control");
    assert_eq!(context["acts"]["resume"]["available"], true);
    assert_eq!(context["opener"], "service");
    let snapshot = fixture.control(get("/api/snapshot"));
    assert_eq!(snapshot.status, 200);
    assert_eq!(fixture.control(get("/api/policy")).status, 200);
    assert_eq!(fixture.control(get("/app.js")).status, 200);
}

#[test]
fn a_wrong_bearer_is_refused_everywhere_and_never_becomes_a_watcher() {
    let fixture = Fixture::start();
    let before = fixture.server.wrong_key_count();
    let paths = ["/", "/app.js", "/api/context", "/api/snapshot"];
    for path in paths {
        let reply = get(path)
            .header("Authorization", "Bearer not-the-key")
            .send(fixture.port());
        assert_eq!(reply.status, 401, "{path}");
        assert_eq!(reply.header("WWW-Authenticate"), Some("Bearer"));
        assert_eq!(reply.code(), "wrong_key");
        assert_eq!(reply.json()["error"]["message"], words::WRONG_KEY);
    }
    let other_scheme = get("/api/context")
        .header("Authorization", &format!("Basic {}", fixture.bearer()))
        .send(fixture.port());
    assert_eq!(other_scheme.status, 401);
    assert_eq!(
        fixture.server.wrong_key_count(),
        before + paths.len() as u64 + 1
    );
}

#[test]
fn without_a_bearer_a_tab_watches_while_watching_is_on() {
    let fixture = Fixture::start();
    let context = fixture.watch(get("/api/context")).json();
    assert_eq!(context["context"], "watch");
    assert_eq!(context["sentence"], words::SENTENCE_WATCH);
    assert_eq!(context["acts"]["hold"]["available"], true);
    assert_eq!(context["acts"]["resume"]["available"], false);
    assert_eq!(
        context["acts"]["resume"]["why_not"],
        words::CHANGE_IN_WINDOW
    );
    assert_eq!(context["opener"], "anchor");
}

#[test]
fn a_watcher_reads_projected_state_and_may_only_hold() {
    let fixture = Fixture::start();
    let snapshot = fixture.watch(get("/api/snapshot"));
    assert_eq!(snapshot.status, 200);
    let body = snapshot.json();
    assert_eq!(body["instance"], "instance_test");
    for part in Part::ALL {
        if part == Part::Setup {
            continue;
        }
        assert!(body["parts"][part.token()].is_object(), "{}", part.token());
    }
    assert!(
        body["parts"].get(Part::Setup.token()).is_none(),
        "a watcher is shown the ordinary workbench while setup is under way"
    );
    let asked = fixture.watch(get("/api/snapshot?parts=setup")).json();
    assert!(asked["parts"].as_object().unwrap().is_empty());
    assert_eq!(fixture.watch(get("/api/policy")).status, 200);

    let held = fixture.watch(post("/api/runtime", r#"{"intent":"hold"}"#));
    assert_eq!(held.status, 200);
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Held);

    let resume = fixture.watch(post("/api/runtime", r#"{"intent":"resume"}"#));
    assert_eq!(resume.status, 401);
    assert_eq!(resume.code(), "needs_window");
    assert_eq!(resume.header("WWW-Authenticate"), Some("Bearer"));
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Held);
    for intent in ["end_session", "start_session"] {
        let reply = fixture.watch(post("/api/runtime", &format!(r#"{{"intent":"{intent}"}}"#)));
        assert_eq!(reply.status, 401, "{intent}");
    }
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Held);
}

#[test]
fn every_control_only_route_refuses_a_watcher_and_changes_nothing() {
    let fixture = Fixture::setting_up();
    let computer = fixture
        .computer
        .clone()
        .expect("a fixture that tracks setup");
    computer.install("chrome");
    let tag = "none";
    let calls = [
        Call::new("PUT", "/api/policy/user")
            .header("If-Match", tag)
            .body(&json!({ "document": READ_POLICY }).to_string()),
        Call::new("DELETE", "/api/policy/user").header("If-Match", tag),
        Call::new("PUT", "/api/controls/diagnostics").body(r#"{"on":true}"#),
        Call::new("PUT", "/api/controls/viewing").body(r#"{"watch":false}"#),
        post("/api/integrations/setup-all", "{}"),
        post(
            "/api/integrations/act",
            r#"{"id":"cursor","action":"install","expected_state":"not_detected"}"#,
        ),
        post("/api/notifications/test", "{}"),
        post("/api/open", r#"{"destination":"home"}"#),
        post("/api/diagnostics/reveal", "{}"),
        post("/api/integrations/locate", r#"{"id":"goose"}"#),
        post(
            "/api/setup/act",
            r#"{"kind":"add_in_browser","browser":"chrome"}"#,
        ),
        post("/api/quit", "{}"),
    ];
    let acts = Arc::new(RecordingActs::default());
    fixture.facade.attach_native_acts(acts.clone());
    let quit = Arc::new(CountingQuit::default());
    fixture.facade.attach_quit(quit.clone());
    for call in calls {
        let label = format!("{} {}", call.method, call.path);
        let reply = fixture.watch(call);
        assert_eq!(reply.status, 401, "{label}");
        assert_eq!(reply.code(), "needs_window", "{label}");
    }
    assert_eq!(acts.asked(), 0);
    assert_eq!(quit.asked(), 0);
    assert_eq!(computer.started(), [], "a watcher starts no browser");
    assert!(fixture.facade.watching());
    assert!(!fixture.root.join("state").join("watch.off").exists());
    assert_eq!(fixture.facade.policy().layers.len(), 0);
    assert!(fixture.facade.diagnostics_report().directory.is_some());
    assert_eq!(fixture.facade.diagnostics.directory(), None);
    assert!(!fixture.root.join("user-policy.json").exists());
}

#[test]
fn closed_serves_the_pages_and_the_product_and_nothing_else() {
    let fixture = Fixture::closed();
    for path in ["/", "/app.js", "/styles.css", "/lib/view.js"] {
        assert_eq!(fixture.watch(get(path)).status, 200, "{path}");
    }
    let context = fixture.watch(get("/api/context"));
    assert_eq!(context.status, 200);
    let body = context.json();
    let mut keys: Vec<&String> = body.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["context", "links", "product", "sentence", "word"]);
    assert_eq!(body["context"], "closed");
    assert_eq!(body["product"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["product"]["license"], words::PRODUCT_LICENSE);
    assert_eq!(body["sentence"], words::WATCHING_OFF);

    let data_routes = [
        get("/api/snapshot"),
        get("/api/snapshot?parts=setup"),
        get("/api/policy"),
        post("/api/search", r#"{"query":"x"}"#),
        post(
            "/api/policy/preview",
            &json!({ "document": READ_POLICY }).to_string(),
        ),
        post("/api/integrations/rescan", "{}"),
        post("/api/window", r#"{"view":"workbench"}"#),
        post("/api/runtime", r#"{"intent":"hold"}"#),
        Call::new("PUT", "/api/policy/user").body("{}"),
        Call::new("DELETE", "/api/policy/user"),
        Call::new("PUT", "/api/controls/diagnostics").body(r#"{"on":true}"#),
        post("/api/integrations/setup-all", "{}"),
        post("/api/integrations/act", "{}"),
        post("/api/notifications/test", "{}"),
        post("/api/open", r#"{"destination":"home"}"#),
        post("/api/diagnostics/reveal", "{}"),
        post("/api/integrations/locate", r#"{"id":"goose"}"#),
        post("/api/quit", "{}"),
    ];
    let acts = Arc::new(RecordingActs::default());
    fixture.facade.attach_native_acts(acts.clone());
    for call in data_routes {
        let label = format!("{} {}", call.method, call.path);
        let reply = fixture.watch(call);
        assert_eq!(reply.status, 403, "{label}");
        assert_eq!(reply.code(), "watching_off", "{label}");
        let text = String::from_utf8(reply.body).unwrap();
        assert!(!text.contains("readiness"), "{label}");
        assert!(text.contains(words::WATCHING_OFF));
    }
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Active);
    assert_eq!(acts.asked(), 0);
}

#[test]
fn the_window_still_controls_while_watching_is_off() {
    let fixture = Fixture::closed();
    let context = fixture.control(get("/api/context")).json();
    assert_eq!(context["context"], "control");
    assert_eq!(context["sentence"], words::SENTENCE_CONTROL_PRIVATE);
    assert_eq!(fixture.control(get("/api/snapshot")).status, 200);
}

#[test]
fn the_window_holds_and_resumes_through_the_runtime_route() {
    let fixture = Fixture::start();
    let reply = fixture.control(post("/api/runtime", r#"{"intent":"hold"}"#));
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json()["runtime_state"], "held");
    let snapshot = fixture.control(get("/api/snapshot?parts=status")).json();
    assert_eq!(
        snapshot["parts"]["status"]["service"]["runtime_state"],
        "held"
    );
    fixture.control(post("/api/runtime", r#"{"intent":"resume"}"#));
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Active);
}

#[test]
fn a_snapshot_holds_the_parts_it_is_asked_for() {
    let fixture = Fixture::start();
    let body = fixture
        .control(get("/api/snapshot?parts=status,connections"))
        .json();
    let parts = body["parts"].as_object().unwrap();
    let mut keys: Vec<&String> = parts.keys().collect();
    keys.sort();
    assert_eq!(keys, ["connections", "status"]);
    for query in [
        "parts=nonsense",
        "parts=status,status",
        "parts=",
        "parts=status,",
    ] {
        let reply = fixture.control(get(&format!("/api/snapshot?{query}")));
        assert_eq!(reply.status, 400, "{query}");
        assert_eq!(reply.code(), "bad_request");
    }
    let everything = fixture.control(get("/api/snapshot")).json();
    assert_eq!(
        everything["parts"].as_object().unwrap().len(),
        Part::ALL.len()
    );
    let setup = fixture.control(get("/api/snapshot?parts=setup")).json();
    assert_eq!(setup["parts"]["setup"]["stage"], "retired");
}

#[test]
fn only_a_request_for_the_setup_part_reads_this_computer() {
    let fixture = Fixture::setting_up();
    let before = fixture.reads();
    for call in [
        "/api/snapshot?parts=status",
        "/api/snapshot?parts=status,connections,activity,integrations,diagnostics,controls",
    ] {
        assert_eq!(fixture.control(get(call)).status, 200, "{call}");
    }
    assert_eq!(
        fixture.watch(get("/api/snapshot")).status,
        200,
        "a watcher is shown no setup part"
    );
    assert_eq!(fixture.watch(get("/api/snapshot?parts=setup")).status, 200);
    assert_eq!(
        fixture.reads(),
        before,
        "no snapshot that leaves the setup part out, or hides it, reads the computer"
    );

    let asked = fixture.control(get("/api/snapshot?parts=setup")).json();
    assert_eq!(asked["parts"]["setup"]["stage"], "browser");
    assert!(
        fixture.reads() > before,
        "asking for the setup part reads it"
    );
}

#[test]
fn rules_are_written_only_against_the_tag_the_caller_read() {
    let fixture = Fixture::start();
    let document = json!({ "document": READ_POLICY }).to_string();
    let put = |tag: Option<&str>| {
        let mut call = Call::new("PUT", "/api/policy/user").body(&document);
        if let Some(tag) = tag {
            call = call.header("If-Match", tag);
        }
        fixture.control(call)
    };
    let unconditional = put(None);
    assert_eq!(unconditional.status, 428);
    assert_eq!(unconditional.code(), "stale");
    assert_eq!(
        unconditional.json()["error"]["message"],
        words::STALE_POLICY
    );
    assert!(!fixture.root.join("user-policy.json").exists());

    let wrong = put(Some("\"0123\""));
    assert_eq!(wrong.status, 412);
    assert!(!fixture.root.join("user-policy.json").exists());

    let tag = fixture.control(get("/api/policy")).json()["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(tag, "none");
    let written = put(Some(&format!("\"{tag}\"")));
    assert_eq!(
        written.status,
        200,
        "{}",
        String::from_utf8_lossy(&written.body)
    );
    assert!(fixture.root.join("user-policy.json").exists());

    let after = fixture.control(get("/api/policy")).json();
    assert_ne!(after["etag"], "none");
    assert!(after["layers"][0]["document"]
        .as_str()
        .unwrap()
        .contains("\"name\": \"read\""));

    // The tag from before is now stale, and the removal needs the new one.
    assert_eq!(put(Some(&tag)).status, 412);
    let removal = |tag: Option<&str>| {
        let mut call = Call::new("DELETE", "/api/policy/user");
        if let Some(tag) = tag {
            call = call.header("If-Match", tag);
        }
        fixture.control(call)
    };
    assert_eq!(removal(None).status, 428);
    assert_eq!(removal(Some(&tag)).status, 412);
    assert!(fixture.root.join("user-policy.json").exists());
    assert_eq!(removal(after["etag"].as_str()).status, 200);
    assert!(!fixture.root.join("user-policy.json").exists());
}

#[test]
fn a_rules_document_that_is_not_a_policy_is_refused_with_the_authoritys_words() {
    let fixture = Fixture::start();
    let reply = fixture.control(
        Call::new("PUT", "/api/policy/user")
            .header("If-Match", "none")
            .body(r#"{"document":"not a policy"}"#),
    );
    assert_eq!(reply.status, 422);
    assert_eq!(reply.code(), "refused");
    assert!(!fixture.root.join("user-policy.json").exists());
}

#[test]
fn diagnostics_are_set_not_flipped() {
    let fixture = Fixture::start();
    let set = |on: bool| {
        fixture.control(
            Call::new("PUT", "/api/controls/diagnostics").body(&json!({ "on": on }).to_string()),
        )
    };
    assert_eq!(set(true).json()["layer"], "marker");
    assert_eq!(set(true).json()["layer"], "marker", "a retry leaves it on");
    assert!(fixture.facade.diagnostics.directory().is_some());
    assert_eq!(set(false).json()["layer"], "off");
    assert_eq!(set(false).json()["layer"], "off", "a retry leaves it off");
    assert!(fixture.facade.diagnostics.directory().is_none());
    let bad =
        fixture.control(Call::new("PUT", "/api/controls/diagnostics").body(r#"{"on":"yes"}"#));
    assert_eq!(bad.status, 400);
}

#[test]
fn a_client_act_against_a_state_the_page_did_not_draw_is_refused() {
    let fixture = Fixture::start();
    let roster = fixture.facade.cached_harnesses();
    let first = &roster[0];
    let wrong_state = if serde_json::to_value(first.state).unwrap() == "installed" {
        "available"
    } else {
        "installed"
    };
    let reply = fixture.control(post(
        "/api/integrations/act",
        &json!({ "id": first.id, "action": "install", "expected_state": wrong_state }).to_string(),
    ));
    assert_eq!(reply.status, 409);
    assert_eq!(reply.code(), "stale");
    assert_eq!(reply.json()["error"]["message"], words::STALE_HARNESS);
    let unchanged = fixture.facade.cached_harnesses();
    assert_eq!(unchanged[0].state, first.state);

    let check = fixture.control(post(
        "/api/integrations/act",
        &json!({ "id": first.id, "action": "check", "expected_state": "x" }).to_string(),
    ));
    assert_eq!(check.status, 400, "a check is not a routed act");
}

#[test]
fn the_window_asks_for_the_window_once_per_five_seconds_when_it_is_a_watcher() {
    let fixture = Fixture::start();
    let window = Arc::new(CountingWindow::default());
    fixture.facade.attach_presentation(window.clone());
    let first = fixture.watch(post("/api/window", r#"{"view":"workbench"}"#));
    assert_eq!(first.status, 200);
    let second = fixture.watch(post("/api/window", r#"{"view":"workbench"}"#));
    assert_eq!(second.status, 429);
    assert!(
        second
            .header("Retry-After")
            .unwrap()
            .parse::<u64>()
            .unwrap()
            >= 1
    );
    assert_eq!(second.code(), "too_many");
    // The window itself is never limited.
    for _ in 0..3 {
        assert_eq!(
            fixture
                .control(post("/api/window", r#"{"view":"workbench"}"#))
                .status,
            200
        );
    }
    assert_eq!(window.asked().len(), 4);
    let unknown = fixture.control(post("/api/window", r#"{"view":"setup"}"#));
    assert_eq!(unknown.status, 400);
}

#[test]
fn the_window_comes_forward_at_every_address_of_the_page() {
    let fixture = Fixture::start();
    let window = Arc::new(CountingWindow::default());
    fixture.facade.attach_presentation(window.clone());
    for address in PageAddress::ALL {
        let body = json!({ "view": "workbench", "address": address.as_str() }).to_string();
        assert_eq!(fixture.control(post("/api/window", &body)).status, 200);
    }
    assert_eq!(
        fixture
            .control(post("/api/window", r#"{"view":"workbench"}"#))
            .status,
        200,
        "no address leaves the window where it was"
    );
    assert_eq!(
        fixture
            .control(post(
                "/api/window",
                r#"{"view":"workbench","address":null}"#
            ))
            .status,
        200
    );
    let mut wanted: Vec<Option<PageAddress>> = PageAddress::ALL.into_iter().map(Some).collect();
    wanted.extend([None, None]);
    assert_eq!(window.asked(), wanted);
}

#[test]
fn an_address_the_page_does_not_have_is_a_bad_request_and_reaches_no_window() {
    let fixture = Fixture::start();
    let window = Arc::new(CountingWindow::default());
    fixture.facade.attach_presentation(window.clone());
    for address in [
        json!("settings/nowhere"),
        json!("#settings"),
        json!(""),
        json!("monitor\"; alert(1); \""),
        json!(7),
        json!(["monitor"]),
    ] {
        let body = json!({ "view": "workbench", "address": address }).to_string();
        let reply = fixture.control(post("/api/window", &body));
        assert_eq!(reply.status, 400, "{address}");
    }
    assert!(window.asked().is_empty());
}

#[test]
fn a_watchers_search_is_limited_and_the_windows_is_not() {
    let fixture = Fixture::start();
    for _ in 0..5 {
        let reply = fixture.watch(post("/api/search", r#"{"query":"status"}"#));
        assert_eq!(reply.status, 200);
        assert!(reply.json().is_array());
    }
    let sixth = fixture.watch(post("/api/search", r#"{"query":"status"}"#));
    assert_eq!(sixth.status, 429);
    assert!(sixth.header("Retry-After").is_some());
    assert_eq!(sixth.json()["error"]["message"], words::TOO_MANY);
    for _ in 0..12 {
        assert_eq!(
            fixture
                .control(post("/api/search", r#"{"query":"status"}"#))
                .status,
            200
        );
    }
    let long = fixture.control(post(
        "/api/search",
        &json!({ "query": "x".repeat(121) }).to_string(),
    ));
    assert_eq!(long.status, 422);
    assert_eq!(long.code(), "refused");
}

#[test]
fn a_watchers_search_does_not_find_the_sites_it_may_not_see() {
    let fixture = Fixture::start();
    fixture.facade.projection.document_coverage(HumanCoverage {
        summary: "One frame was excluded.".into(),
        explanation: "Excluded by policy.".into(),
        invocation: "invocation_1".into(),
        coverage: Coverage::default(),
        excluded_hosts: vec![EXCLUDED_HOST.into()],
        proactive: false,
    });
    let reply = fixture.watch(post(
        "/api/search",
        &json!({ "query": EXCLUDED_HOST }).to_string(),
    ));
    assert_eq!(reply.status, 200);
    assert_eq!(reply.json(), json!([]));
}

#[test]
fn a_watcher_sees_excluded_sites_only_as_a_count() {
    let fixture = Fixture::start();
    fixture.facade.projection.document_coverage(HumanCoverage {
        summary: "One frame was excluded.".into(),
        explanation: "Excluded by policy.".into(),
        invocation: "invocation_1".into(),
        coverage: Coverage::default(),
        excluded_hosts: vec![EXCLUDED_HOST.into()],
        proactive: false,
    });
    let watcher = fixture.watch(get("/api/snapshot?parts=activity"));
    let text = String::from_utf8(watcher.body.clone()).unwrap();
    assert!(!text.contains(EXCLUDED_HOST));
    let coverage = &watcher.json()["parts"]["activity"]["document_coverage"][0];
    assert_eq!(coverage["excluded_host_count"], 1);
    assert!(coverage.get("excluded_hosts").is_none());

    let window = fixture.control(get("/api/snapshot?parts=activity")).json();
    let coverage = &window["parts"]["activity"]["document_coverage"][0];
    assert_eq!(coverage["excluded_hosts"], json!([EXCLUDED_HOST]));
    assert_eq!(coverage["excluded_host_count"], 1);
}

#[test]
fn a_watcher_sees_the_never_touch_list_only_as_a_count_and_no_policy_file() {
    let policy = format!(
        r#"{{"schema":4,"name":"mine","version":"1","grants":[{{"id":"read","hosts":{{"allow":["*"]}},"allowed":["read"]}}],"config":[{{"key":"content.security.sacred_domains","value":["{SACRED_HOST}","second.invalid"],"level":"mandatory"}}]}}"#
    );
    let fixture = Fixture::with_policy(&policy);
    let watcher = fixture.watch(get("/api/policy"));
    let text = String::from_utf8(watcher.body.clone()).unwrap();
    assert!(!text.contains(SACRED_HOST));
    assert!(!text.contains("second.invalid"));
    let body = watcher.json();
    assert_eq!(body["layers"][0]["document"], Value::Null);
    let setting = &body["layers"][0]["settings"][0];
    assert_eq!(setting["key"], "content.security.sacred_domains");
    assert_eq!(setting["value"], "2");
    assert_eq!(setting["count"], 2);
    let ceilings = body["ceilings"].as_array().unwrap();
    assert_eq!(
        ceilings.last().unwrap(),
        &json!(words::never_touch_count(2))
    );
    assert!(ceilings
        .iter()
        .all(|line| !line.as_str().unwrap().contains(".invalid")));

    let window = fixture.control(get("/api/policy")).json();
    assert!(window["layers"][0]["document"]
        .as_str()
        .unwrap()
        .contains(SACRED_HOST));
    assert!(window["layers"][0]["settings"][0]["value"]
        .as_str()
        .unwrap()
        .contains(SACRED_HOST));
    assert!(window["layers"][0]["settings"][0].get("count").is_none());
    assert!(window["ceilings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line.as_str().unwrap().contains(SACRED_HOST)));
}

#[test]
fn paths_are_home_relative_for_both_audiences_and_manual_setup_stays_exact() {
    let fixture = Fixture::start();
    let roster = fixture.facade.cached_harnesses();
    let user_home = std::env::var_os(USER_HOME_VARIABLE).map(PathBuf::from);
    for audience in [true, false] {
        let body = if audience {
            fixture.control(get("/api/snapshot?parts=integrations,diagnostics"))
        } else {
            fixture.watch(get("/api/snapshot?parts=integrations,diagnostics"))
        }
        .json();
        let harnesses = body["parts"]["integrations"]["harnesses"]
            .as_array()
            .unwrap();
        assert_eq!(harnesses.len(), roster.len());
        for (shown, actual) in harnesses.iter().zip(&roster) {
            assert_eq!(shown["manual_setup"], actual.manual_setup);
            assert_eq!(shown["connector_command"], actual.connector_command);
            assert!(shown.get("private_view").is_none());
            if let Some(home) = &user_home {
                let path = shown["config_path"].as_str().unwrap();
                let home = home.to_string_lossy();
                assert!(
                    !path
                        .to_ascii_lowercase()
                        .starts_with(&home.to_ascii_lowercase()),
                    "{path} is not home-relative"
                );
            }
        }
        let directory = body["parts"]["controls"]["diagnostics"]["folder"]
            .as_str()
            .unwrap_or_default();
        if let Some(home) = &user_home {
            let home = home.to_string_lossy().to_ascii_lowercase();
            assert!(!directory.to_ascii_lowercase().starts_with(&home));
        }
        let text = body.to_string();
        for dropped in ["overview", "generated_at_ms", "held_tab_count", "leased"] {
            assert!(!text.contains(&format!("\"{dropped}\"")), "{dropped}");
        }
    }
}

#[test]
fn two_rescans_by_a_watcher_inside_five_seconds_scan_once() {
    let fixture = Fixture::start();
    let before = fixture.server.scan_count();
    let first = fixture.watch(post("/api/integrations/rescan", "{}"));
    let second = fixture.watch(post("/api/integrations/rescan", "{}"));
    assert_eq!((first.status, second.status), (200, 200));
    assert_eq!(first.json(), second.json());
    assert_eq!(fixture.server.scan_count(), before + 1);
    // The window always scans.
    assert_eq!(
        fixture
            .control(post("/api/integrations/rescan", "{}"))
            .status,
        200
    );
    assert_eq!(
        fixture
            .control(post("/api/integrations/rescan", "{}"))
            .status,
        200
    );
    assert_eq!(fixture.server.scan_count(), before + 3);
}

#[test]
fn a_watchers_policy_check_runs_one_at_a_time_and_is_answered() {
    let fixture = Fixture::start();
    let reply = fixture.watch(post(
        "/api/policy/preview",
        &json!({ "document": READ_POLICY }).to_string(),
    ));
    assert_eq!(reply.status, 200);
    assert!(reply.json()["summary"].is_string());
    let bad = fixture.watch(post("/api/policy/preview", r#"{"document":"nope"}"#));
    assert_eq!(bad.status, 422);
    let empty = fixture.watch(post("/api/policy/preview", r#"{"document":"  "}"#));
    assert_eq!(empty.status, 422);
}

#[test]
fn a_page_for_localhost_moves_to_the_loopback_address_and_the_api_does_not() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let page = get("/").host(&format!("localhost:{port}")).send(port);
    assert_eq!(page.status, 308);
    assert_eq!(
        page.header("Location"),
        Some(format!("http://127.0.0.1:{port}/").as_str())
    );
    let deeper = get("/lib/view.js")
        .host(&format!("localhost:{port}"))
        .send(port);
    assert_eq!(deeper.status, 308);
    assert_eq!(
        deeper.header("Location"),
        Some(format!("http://127.0.0.1:{port}/lib/view.js").as_str())
    );
    let api = get("/api/context")
        .host(&format!("localhost:{port}"))
        .send(port);
    assert_eq!(api.status, 421);
    assert_eq!(api.code(), "misdirected");
    let write = post("/api/search", r#"{"query":"x"}"#)
        .host(&format!("localhost:{port}"))
        .send(port);
    assert_eq!(write.status, 421);
}

#[test]
fn any_other_host_is_misdirected_and_runs_no_route() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let other_port = if port == 41_999 { port - 1 } else { port + 1 };
    for host in [
        format!("127.0.0.1:{other_port}"),
        "evil.example".to_owned(),
        format!("evil.example:{port}"),
        "127.0.0.1".to_owned(),
        format!("[::1]:{port}"),
    ] {
        for path in ["/", "/api/context"] {
            let reply = get(path).host(&host).send(port);
            assert_eq!(reply.status, 421, "{host} {path}");
        }
        let held = post("/api/runtime", r#"{"intent":"hold"}"#)
            .host(&host)
            .bearer(&fixture.bearer())
            .send(port);
        assert_eq!(held.status, 421, "{host}");
    }
    let missing = raw(
        port,
        "GET /api/context HTTP/1.1\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(missing.status, 421);
    let missing_page = raw(port, "GET / HTTP/1.1\r\n\r\n");
    assert_eq!(missing_page.status, 421);
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Active);
}

#[test]
fn methods_and_paths_are_closed() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let wrong = Call::new("POST", "/api/context").body("{}").send(port);
    assert_eq!(wrong.status, 405);
    assert_eq!(wrong.header("Allow"), Some("GET"));
    let wrong = get("/api/search").send(port);
    assert_eq!(wrong.status, 405);
    assert_eq!(wrong.header("Allow"), Some("POST"));
    let wrong = get("/api/policy/user").send(port);
    assert_eq!(wrong.status, 405);
    assert_eq!(wrong.header("Allow"), Some("PUT, DELETE"));
    assert_eq!(
        Call::new("POST", "/app.js").body("{}").send(port).status,
        405
    );
    assert_eq!(Call::new("HEAD", "/api/context").send(port).status, 405);

    for path in [
        "/nothing",
        "/lib/../app.js",
        "/lib/%2e%2e/app.js",
        "/api/other",
        "/app.js%00",
        "/lib\\view.js",
        "/index.html",
        "/integrations/codex.svg",
    ] {
        let reply = get(path).send(port);
        assert_eq!(reply.status, 404, "{path}");
    }
    let options = Call::new("OPTIONS", "/api/context")
        .header("Origin", "https://evil.example")
        .header("Access-Control-Request-Method", "POST")
        .send(port);
    assert_eq!(options.status, 405);
    assert!(options
        .headers
        .iter()
        .all(|(name, _)| !name.to_ascii_lowercase().starts_with("access-control-")));
    assert_eq!(Call::new("PATCH", "/api/context").send(port).status, 405);
}

#[test]
fn upgrades_and_other_framings_are_refused() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let upgrade = get("/api/context")
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .send(port);
    assert_eq!(upgrade.status, 400);
    let chunked = Call::new("POST", "/api/search")
        .header("Transfer-Encoding", "chunked")
        .body("0\r\n\r\n")
        .send(port);
    assert_eq!(chunked.status, 400);
    let garbled = raw(port, "NOT AN HTTP REQUEST\r\n\r\n");
    assert_eq!(garbled.status, 400);
    let versioned = raw(port, "GET / HTTP/2\r\nHost: x\r\n\r\n");
    assert_eq!(versioned.status, 400);
}

#[test]
fn a_change_must_be_json_and_from_this_origin_even_with_the_bearer() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let bearer = fixture.bearer();
    let plain = post("/api/runtime", r#"{"intent":"hold"}"#)
        .content_type("text/plain")
        .bearer(&bearer)
        .send(port);
    assert_eq!(plain.status, 415);
    assert_eq!(plain.code(), "unsupported_type");
    let charset = post("/api/search", r#"{"query":"x"}"#)
        .content_type("application/json; charset=utf-8")
        .send(port);
    assert_eq!(charset.status, 200);
    for (name, value) in [
        ("Origin", "https://evil.example"),
        ("Origin", "http://localhost:41000"),
        ("Origin", "null"),
        ("Sec-Fetch-Site", "cross-site"),
        ("Sec-Fetch-Site", "same-site"),
    ] {
        for with_bearer in [false, true] {
            let mut call = post("/api/runtime", r#"{"intent":"hold"}"#).header(name, value);
            if with_bearer {
                call = call.bearer(&bearer);
            }
            let reply = call.send(port);
            assert_eq!(reply.status, 403, "{name}: {value}");
            assert_eq!(reply.code(), "cross_site");
        }
    }
    assert_eq!(runtime_state(&fixture), RuntimeControlState::Active);
    let same = post("/api/search", r#"{"query":"x"}"#)
        .header("Origin", &format!("http://127.0.0.1:{port}"))
        .header("Sec-Fetch-Site", "same-origin")
        .send(port);
    assert_eq!(same.status, 200);
    let typed = post("/api/search", r#"{"query":"x"}"#)
        .header("Sec-Fetch-Site", "none")
        .send(port);
    assert_eq!(typed.status, 200);
    let empty_delete = Call::new("DELETE", "/api/policy/user")
        .header("If-Match", "none")
        .bearer(&bearer)
        .send(port);
    assert_eq!(empty_delete.status, 200);
}

#[test]
fn a_body_over_its_cap_is_refused_before_it_is_read() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let big = format!(r#"{{"query":"{}"}}"#, "x".repeat(5_000));
    let reply = post("/api/search", &big).send(port);
    assert_eq!(reply.status, 413);
    assert_eq!(reply.code(), "too_large");
    let policy = json!({ "document": "x".repeat(300 * 1024) }).to_string();
    assert_eq!(post("/api/policy/preview", &policy).send(port).status, 413);
    let within = json!({ "document": "x".repeat(100 * 1024) }).to_string();
    assert_eq!(
        post("/api/policy/preview", &within).send(port).status,
        422,
        "a document under the cap reaches the validator"
    );
}

#[test]
fn a_body_that_is_not_the_routes_arguments_is_a_bad_request() {
    let fixture = Fixture::start();
    for body in [
        "",
        "not json",
        "[]",
        "\"query\"",
        r#"{"query":1}"#,
        r#"{"query":"x","extra":1}"#,
        "{}",
    ] {
        let reply = fixture.control(post("/api/search", body));
        assert_eq!(reply.status, 400, "{body}");
        assert_eq!(reply.code(), "bad_request");
    }
    for body in ["", "not json", "[]", "\"x\"", r#"{"x":1}"#] {
        for route in ["/api/integrations/rescan", "/api/integrations/setup-all"] {
            let reply = fixture.control(post(route, body));
            assert_eq!(reply.status, 400, "{route} {body}");
            assert_eq!(reply.code(), "bad_request", "{route} {body}");
        }
    }
}

#[test]
fn a_watcher_is_given_no_policy_etag_and_the_window_is() {
    let fixture = Fixture::start();
    assert!(fixture.control(get("/api/policy")).json()["etag"].is_string());
    assert!(fixture
        .watch(get("/api/policy"))
        .json()
        .get("etag")
        .is_none());
}

#[test]
fn no_response_allows_another_site_and_the_page_refuses_framing() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let mut replies = vec![
        get("/").send(port),
        get("/app.js").send(port),
        get("/api/context").send(port),
        get("/nothing").send(port),
        Call::new("OPTIONS", "/").send(port),
        post("/api/search", r#"{"query":"x"}"#)
            .header("Origin", "https://evil.example")
            .send(port),
        get("/api/context").host("evil.example").send(port),
    ];
    replies.push(raw(port, "garbage\r\n\r\n"));
    for reply in &replies {
        for (name, _) in &reply.headers {
            assert!(
                !name.to_ascii_lowercase().starts_with("access-control-"),
                "{name}"
            );
        }
        assert_eq!(reply.header("X-Content-Type-Options"), Some("nosniff"));
        assert_eq!(reply.header("Referrer-Policy"), Some("no-referrer"));
        assert_eq!(
            reply.header("Cross-Origin-Resource-Policy"),
            Some("same-origin")
        );
        assert_eq!(reply.header("Connection"), Some("close"));
        assert!(reply.header("Content-Length").is_some());
    }
    let index = &replies[0];
    assert_eq!(index.header("X-Frame-Options"), Some("DENY"));
    let policy = index.header("Content-Security-Policy").unwrap();
    assert!(policy.contains("default-src 'self'"));
    assert!(policy.contains("frame-ancestors 'none'"));
    assert!(policy.contains("object-src 'none'"));
    assert!(!policy.contains("unsafe-inline"));
    assert_eq!(
        index.header("Content-Type"),
        Some("text/html; charset=utf-8")
    );
    assert!(replies[1].header("X-Frame-Options").is_none());
    assert_eq!(replies[2].header("Cache-Control"), Some("no-store"));
}

#[test]
fn every_page_file_is_served_with_its_closed_type() {
    let fixture = Fixture::start();
    let port = fixture.port();
    for asset in super::assets::ASSETS {
        let reply = get(asset.path).send(port);
        assert_eq!(reply.status, 200, "{}", asset.path);
        assert_eq!(
            reply.header("Content-Type"),
            Some(asset.kind.media_type()),
            "{}",
            asset.path
        );
        assert_eq!(reply.body, asset.bytes, "{}", asset.path);
        let tag = reply.header("ETag").unwrap().to_owned();
        let again = get(asset.path).header("If-None-Match", &tag).send(port);
        assert_eq!(again.status, 304, "{}", asset.path);
        assert!(again.body.is_empty());
    }
    let head = Call::new("HEAD", "/app.js").send(port);
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
    assert!(
        head.header("Content-Length")
            .unwrap()
            .parse::<usize>()
            .unwrap()
            > 0
    );
}

#[test]
fn the_page_files_are_never_rate_limited() {
    let fixture = Fixture::start();
    for _ in 0..30 {
        assert_eq!(get("/").send(fixture.port()).status, 200);
    }
}

#[test]
fn a_server_stops_when_it_is_dropped() {
    let fixture = Fixture::start();
    let port = fixture.port();
    fixture.server.stop();
    fixture.server.stop();
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
}

#[test]
fn a_busy_server_serves_a_connection_that_says_nothing_without_blocking_others() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let _idle: Vec<TcpStream> = (0..8)
        .map(|_| TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap())
        .collect();
    assert_eq!(get("/api/context").send(port).status, 200);
}

#[test]
fn the_snapshot_of_a_watcher_equals_the_windows_apart_from_what_it_may_not_see() {
    let fixture = Fixture::start();
    let window = fixture.control(get("/api/snapshot?parts=status,connections,diagnostics"));
    let watcher = fixture.watch(get("/api/snapshot?parts=status,connections,diagnostics"));
    let mut window = window.json();
    let mut watcher = watcher.json();
    window["parts"]["status"]["service"]["started_at_ms"] = Value::Null;
    watcher["parts"]["status"]["service"]["started_at_ms"] = Value::Null;
    assert_eq!(window["parts"], watcher["parts"]);
}

#[test]
fn the_facade_reports_the_server_it_is_served_by() {
    let fixture = Fixture::start();
    let address = fixture.facade.web_address().unwrap();
    assert_eq!(&address, fixture.server.address());
    assert!(fixture.facade.watching());
    let context = fixture
        .facade
        .context_document(crate::workbench::context::WebContext::Control);
    assert_eq!(context["instance"], "instance_test");
    assert_eq!(fixture.control(get("/api/context")).json(), context);
    let closed = Fixture::closed();
    assert!(!closed.facade.watching());
}

#[test]
fn a_facade_with_no_server_reports_watching_on_and_no_address() {
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None, None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    assert!(facade.web_address().is_none());
    assert!(facade.watching());
}

#[test]
fn the_window_quits_through_the_attached_port_and_is_told_so() {
    let fixture = Fixture::start();
    let unattached = fixture.control(post("/api/quit", "{}"));
    assert_eq!(unattached.status, 422);
    assert_eq!(unattached.code(), "refused");
    assert_eq!(
        unattached.json()["error"]["message"],
        words::QUIT_UNAVAILABLE
    );

    let quit = Arc::new(CountingQuit::default());
    fixture.facade.attach_quit(quit.clone());
    let reply = fixture.control(post("/api/quit", "{}"));
    assert_eq!(reply.status, 202);
    assert_eq!(reply.json(), json!({ "message": words::QUITTING }));
    assert_eq!(quit.asked(), 1);
}

#[test]
fn quit_is_the_windows_alone_and_a_closed_view_cannot_reach_it() {
    let watched = Fixture::start();
    let quit = Arc::new(CountingQuit::default());
    watched.facade.attach_quit(quit.clone());
    let watcher = watched.watch(post("/api/quit", "{}"));
    assert_eq!(watcher.status, 401);
    assert_eq!(watcher.code(), "needs_window");
    assert_eq!(watcher.json()["error"]["message"], words::CHANGE_IN_WINDOW);

    let closed = Fixture::closed();
    let closed_quit = Arc::new(CountingQuit::default());
    closed.facade.attach_quit(closed_quit.clone());
    let refused = closed.watch(post("/api/quit", "{}"));
    assert_eq!(refused.status, 403);
    assert_eq!(refused.code(), "watching_off");
    assert_eq!(quit.asked() + closed_quit.asked(), 0);

    // The window can still end Ghostlight while watching is off.
    assert_eq!(closed.control(post("/api/quit", "{}")).status, 202);
    assert_eq!(closed_quit.asked(), 1);
}

#[test]
fn a_quit_from_another_site_or_with_arguments_is_refused() {
    let fixture = Fixture::start();
    let quit = Arc::new(CountingQuit::default());
    fixture.facade.attach_quit(quit.clone());
    let foreign = fixture.control(
        post("/api/quit", "{}")
            .header("Origin", "http://evil.example")
            .header("Sec-Fetch-Site", "cross-site"),
    );
    assert_eq!(foreign.status, 403);
    assert_eq!(foreign.code(), "cross_site");
    let with_arguments = fixture.control(post("/api/quit", r#"{"now":true}"#));
    assert_eq!(with_arguments.status, 400);
    let wrong_method = fixture.control(get("/api/quit"));
    assert_eq!(wrong_method.status, 405);
    assert_eq!(quit.asked(), 0);
}

#[test]
fn the_context_lists_quit_as_an_act_of_the_window_only() {
    let fixture = Fixture::start();
    let window = fixture.control(get("/api/context")).json();
    assert_eq!(window["acts"]["quit"]["available"], true);
    let tab = fixture.watch(get("/api/context")).json();
    assert_eq!(tab["acts"]["quit"]["available"], false);
    assert_eq!(tab["acts"]["quit"]["why_not"], words::CHANGE_IN_WINDOW);
    let closed = Fixture::closed().watch(get("/api/context")).json();
    assert!(closed.get("acts").is_none());
}

#[test]
fn the_notification_test_reaches_the_window_only() {
    let fixture = Fixture::start();
    let unavailable = fixture.control(post("/api/notifications/test", "{}"));
    assert_eq!(unavailable.status, 422);
    fixture
        .facade
        .attach_presentation(Arc::new(CountingWindow::default()));
    assert_eq!(
        fixture
            .control(post("/api/notifications/test", "{}"))
            .status,
        200
    );
}

#[test]
fn a_request_head_over_sixteen_kibibytes_is_refused() {
    let fixture = Fixture::start();
    let long = format!(
        "GET /api/context HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Padding: {}\r\n\r\n",
        fixture.port(),
        "a".repeat(17 * 1024)
    );
    assert_eq!(raw(fixture.port(), &long).status, 400);
}

#[test]
fn a_thirty_third_connection_is_closed_without_an_answer() {
    let fixture = Fixture::start();
    let port = fixture.port();
    let idle: Vec<TcpStream> = (0..super::MAX_CONNECTIONS)
        .map(|_| TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap())
        .collect();
    // Let the listener give each idle connection its place.
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut extra = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    extra
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let _ = extra.write_all(
        format!("GET /api/context HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes(),
    );
    let mut answer = Vec::new();
    let _ = extra.read_to_end(&mut answer);
    assert!(answer.is_empty(), "{}", String::from_utf8_lossy(&answer));
    drop(idle);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(get("/api/context").send(port).status, 200);
}

/// One block of a stream: the parts of an event, or a comment.
#[derive(Debug, Default)]
struct Block {
    event: Option<String>,
    id: Option<String>,
    data: Option<String>,
    comment: Option<String>,
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
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
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
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            std::io::BufRead::read_line(&mut reader, &mut line).unwrap();
            if line.trim_end().is_empty() {
                break;
            }
            headers.push(line.trim_end().to_ascii_lowercase());
        }
        assert!(headers.contains(&"content-type: text/event-stream".to_owned()));
        assert!(headers.contains(&"cache-control: no-store".to_owned()));
        assert!(
            !headers
                .iter()
                .any(|line| line.starts_with("content-length")),
            "a stream has no length: {headers:?}"
        );
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
            if let Some(comment) = line.strip_prefix(": ") {
                block.comment = Some(comment.to_owned());
            } else if let Some(value) = line.strip_prefix("event: ") {
                block.event = Some(value.to_owned());
            } else if let Some(value) = line.strip_prefix("id: ") {
                block.id = Some(value.to_owned());
            } else if let Some(value) = line.strip_prefix("data: ") {
                block.data = Some(value.to_owned());
            } else {
                panic!("not a stream line: {line}");
            }
        }
    }

    /// The next block when one comes within `wait`.
    fn next_within(&mut self, wait: std::time::Duration) -> Option<Block> {
        self.reader.get_ref().set_read_timeout(Some(wait)).unwrap();
        let block = self.next();
        self.reader
            .get_ref()
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .unwrap();
        block
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

#[test]
fn a_window_and_a_watcher_hear_the_same_change_each_as_it_may_see_it() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    let mut tab = Stream::open(fixture.port(), None);
    let window_hello = window.hello();
    let tab_hello = tab.hello();
    assert_eq!(window_hello["context"], "control");
    assert_eq!(tab_hello["context"], "watch");
    for hello in [&window_hello, &tab_hello] {
        assert_eq!(hello["instance"], "instance_test");
        assert_eq!(hello["seq"], fixture.facade.current_seq());
    }

    fixture.facade.projection.document_coverage(HumanCoverage {
        summary: "One frame was excluded.".into(),
        explanation: "Excluded by policy.".into(),
        invocation: "invocation_1".into(),
        coverage: Coverage::default(),
        excluded_hosts: vec![EXCLUDED_HOST.into()],
        proactive: false,
    });
    let at_window = window.until(|block| block.is("document_coverage_changed"));
    let at_tab = tab.until(|block| block.is("document_coverage_changed"));
    let seq = window_hello["seq"].as_u64().unwrap() + 1;
    assert_eq!(at_window.id, Some(format!("instance_test-{seq}")));
    assert_eq!(at_tab.id, at_window.id);
    assert_eq!(at_window.json()["seq"], seq);

    let details = &at_window.json()["change"]["details"];
    assert_eq!(details["excluded_hosts"], json!([EXCLUDED_HOST]));
    assert_eq!(details["excluded_host_count"], 1);
    let watched = at_tab.json();
    assert_eq!(watched["change"]["details"]["excluded_host_count"], 1);
    assert!(watched["change"]["details"].get("excluded_hosts").is_none());
    assert!(!at_tab.data.unwrap().contains(EXCLUDED_HOST));
}

#[test]
fn an_act_answers_with_the_sequence_of_the_last_change_it_caused() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    window.hello();

    let reply = fixture.control(post("/api/runtime", r#"{"intent":"hold"}"#));
    assert_eq!(reply.status, 200);
    let seq: u64 = reply.header("Ghostlight-Seq").unwrap().parse().unwrap();
    assert_eq!(seq, fixture.facade.current_seq());

    let mut kinds = Vec::new();
    let mut note = None;
    loop {
        let block = window.next().expect("the changes the act caused arrive");
        let kind = block.kind().unwrap();
        if kind == "status_changed" {
            note = block.json()["change"]["status"]["runtime_note"]
                .as_str()
                .map(str::to_owned);
        }
        kinds.push(kind);
        if block.id == Some(format!("instance_test-{seq}")) {
            break;
        }
    }
    assert!(kinds.contains(&"runtime_changed".to_owned()), "{kinds:?}");
    assert_eq!(note.as_deref(), Some(words::PAUSED_IN_WINDOW));
    assert!(window
        .next_within(std::time::Duration::from_millis(300))
        .is_none());

    // A read answers without a sequence: it caused nothing.
    assert!(fixture
        .control(get("/api/snapshot"))
        .header("Ghostlight-Seq")
        .is_none());
    // A refused act caused nothing either.
    let refused = fixture.control(post("/api/runtime", r#"{"intent":"nonsense"}"#));
    assert_eq!(refused.status, 400);
    assert!(refused.header("Ghostlight-Seq").is_none());
}

#[test]
fn a_pause_from_a_watching_screen_says_so_on_every_screen() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    window.hello();
    let reply = fixture.watch(post("/api/runtime", r#"{"intent":"hold"}"#));
    assert_eq!(reply.status, 200);
    let status = window.until(|block| block.is("status_changed"));
    assert_eq!(
        status.json()["change"]["status"]["runtime_note"],
        words::PAUSED_FROM_WATCHER
    );
    assert_eq!(
        status.json()["change"]["status"]["service"]["runtime_state"],
        serde_json::to_value(RuntimeControlState::Held).unwrap()
    );
}

#[test]
fn the_acts_that_change_a_part_hint_at_it_with_its_tag() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    let mut tab = Stream::open(fixture.port(), None);
    window.hello();
    tab.hello();
    let hint = |stream: &mut Stream, part: &str| {
        stream.until(|block| block.is("part_changed") && block.json()["change"]["part"] == part)
    };

    let tag = fixture.control(get("/api/policy")).json()["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    let put = fixture.control(
        Call::new("PUT", "/api/policy/user")
            .header("If-Match", &tag)
            .body(&json!({ "document": READ_POLICY }).to_string()),
    );
    assert_eq!(put.status, 200);
    let hinted = hint(&mut window, "policy");
    let new_tag = fixture.control(get("/api/policy")).json()["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(new_tag, tag);
    assert_eq!(hinted.json()["change"]["etag"], new_tag);
    assert_eq!(
        hinted.id,
        Some(format!(
            "instance_test-{}",
            put.header("Ghostlight-Seq").unwrap()
        ))
    );
    // A watcher is told the part changed and is given no tag to test guesses against.
    let watched = hint(&mut tab, "policy");
    assert_eq!(watched.id, hinted.id);
    assert!(watched.json()["change"].get("etag").is_none());

    let diagnostics = fixture.control(
        Call::new("PUT", "/api/controls/diagnostics").body(&json!({ "on": true }).to_string()),
    );
    assert_eq!(diagnostics.status, 200);
    let hinted = hint(&mut window, "diagnostics");
    assert_eq!(hinted.json()["change"]["etag"].as_str().unwrap().len(), 64);
    // The last change the act caused is the sheet's new state, which follows the hint.
    let sheet = window.until(|block| block.is("controls_changed"));
    assert_eq!(
        sheet.json()["change"]["controls"]["diagnostics"]["on"],
        true
    );
    assert_eq!(
        sheet.id,
        Some(format!(
            "instance_test-{}",
            diagnostics.header("Ghostlight-Seq").unwrap()
        ))
    );

    let rescan = fixture.control(post("/api/integrations/rescan", "{}"));
    assert_eq!(rescan.status, 200);
    let hinted = hint(&mut window, "integrations");
    assert_eq!(hinted.json()["change"]["etag"].as_str().unwrap().len(), 64);
    assert_eq!(
        hinted.id,
        Some(format!(
            "instance_test-{}",
            rescan.header("Ghostlight-Seq").unwrap()
        ))
    );
    assert_eq!(
        hint(&mut tab, "integrations").json()["change"]["etag"],
        hinted.json()["change"]["etag"]
    );
}

#[test]
fn only_so_many_screens_can_watch_and_the_next_is_told_why() {
    let fixture = Fixture::start();
    let bearer = fixture.bearer();
    let windows: Vec<Stream> = (0..4)
        .map(|_| Stream::open(fixture.port(), Some(&bearer)))
        .collect();
    let refused = fixture.control(get("/api/events"));
    assert_eq!(refused.status, 503);
    assert_eq!(refused.code(), "too_many_screens");
    assert_eq!(refused.json()["error"]["message"], words::TOO_MANY_SCREENS);
    // The window's places are its own: a watcher still gets in.
    let tabs: Vec<Stream> = (0..8).map(|_| Stream::open(fixture.port(), None)).collect();
    let refused = fixture.watch(get("/api/events"));
    assert_eq!(refused.status, 503);
    assert_eq!(refused.code(), "too_many_screens");
    drop((windows, tabs));
}

#[test]
fn a_closed_view_has_no_stream_and_the_window_still_does() {
    let fixture = Fixture::closed();
    let refused = fixture.watch(get("/api/events"));
    assert_eq!(refused.status, 403);
    assert_eq!(refused.code(), "watching_off");
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    assert_eq!(window.hello()["context"], "control");
    let wrong = get("/api/events")
        .bearer("not-the-key")
        .send(fixture.port());
    assert_eq!(wrong.status, 401);
}

#[test]
fn a_stopped_server_ends_its_streams() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    window.hello();
    fixture.server.stop();
    assert!(window.next().is_none());
}

#[test]
fn a_screen_that_stops_reading_is_reset_and_another_keeps_receiving() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

    let fixture = Fixture::start();
    let bearer = fixture.bearer();
    let mut slow = Stream::open(fixture.port(), Some(&bearer));
    let quick = Stream::open(fixture.port(), Some(&bearer));
    let received = Arc::new(AtomicUsize::new(0));
    let counting = {
        let received = Arc::clone(&received);
        std::thread::spawn(move || {
            let mut quick = quick;
            while let Some(block) = quick.next() {
                if block.is("document_coverage_changed") {
                    received.fetch_add(1, Ordering::AcqRel);
                }
            }
        })
    };

    // Each change names enough sites that the sockets between the two cannot hold the backlog.
    let hosts: Vec<String> = (0..500)
        .map(|index| format!("{index:03}-{}.invalid", "x".repeat(120)))
        .collect();
    let mut published = 0_usize;
    let started = std::time::Instant::now();
    let events = &fixture.server.shared.events;
    while events.open_streams(WebContext::Control) == 2 {
        assert!(
            started.elapsed() < PATIENCE,
            "the screen that never reads was never ended"
        );
        fixture.facade.projection.document_coverage(HumanCoverage {
            summary: "Frames were excluded.".into(),
            explanation: "Excluded by policy.".into(),
            invocation: format!("invocation_{published}"),
            coverage: Coverage::default(),
            excluded_hosts: hosts.clone(),
            proactive: false,
        });
        published += 1;
        // The screen that reads is never allowed to fall far behind the one that does not.
        while published > received.load(Ordering::Acquire) + 100 {
            assert!(started.elapsed() < PATIENCE, "the quick screen stopped");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    let reset = slow.until(|block| block.event.as_deref() == Some("reset"));
    assert_eq!(reset.json(), json!({ "reason": "behind" }));
    assert!(slow.next().is_none(), "a reset ends the stream");
    assert_eq!(events.open_streams(WebContext::Control), 1);

    // The other screen lost nothing, and hears the next change too.
    fixture.facade.projection.document_coverage(HumanCoverage {
        summary: "Frames were excluded.".into(),
        explanation: "Excluded by policy.".into(),
        invocation: "invocation_last".into(),
        coverage: Coverage::default(),
        excluded_hosts: vec![EXCLUDED_HOST.into()],
        proactive: false,
    });
    published += 1;
    while received.load(Ordering::Acquire) < published {
        assert!(
            started.elapsed() < PATIENCE,
            "the quick screen lost a change"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    fixture.server.stop();
    counting.join().unwrap();
}

#[test]
fn the_window_opens_a_destination_or_a_clients_download_and_nothing_else() {
    let fixture = Fixture::start();
    let acts = Arc::new(RecordingActs::default());
    fixture.facade.attach_native_acts(acts.clone());

    let home = fixture.control(post("/api/open", r#"{"destination":"home"}"#));
    assert_eq!(home.status, 200);
    assert_eq!(
        *acts.urls.lock().unwrap(),
        [WorkbenchDestination::Home.url()]
    );

    let vscode = fixture
        .facade
        .cached_harnesses()
        .into_iter()
        .find(|summary| summary.id == "cline-vscode")
        .unwrap();
    let download = fixture.control(post(
        "/api/open",
        &json!({ "harness": vscode.id }).to_string(),
    ));
    assert_eq!(download.status, 200);
    assert_eq!(
        acts.urls.lock().unwrap()[1..],
        [vscode.download_url.unwrap()]
    );

    for unknown in [
        r#"{"harness":"not-a-harness"}"#,
        r#"{"harness":"claude-desktop"}"#,
    ] {
        let unknown = fixture.control(post("/api/open", unknown));
        assert_eq!(unknown.status, 422);
        assert_eq!(unknown.code(), "refused");
    }

    for body in [
        r#"{"destination":"https://example.com"}"#,
        r#"{"destination":"home","harness":"x"}"#,
        r#"{}"#,
        r#"{"url":"https://example.com"}"#,
        r#"{"product":"cline"}"#,
        r#"{"destination":null,"harness":null}"#,
    ] {
        let reply = fixture.control(post("/api/open", body));
        assert_eq!(reply.status, 400, "{body}");
    }
    assert_eq!(acts.urls.lock().unwrap().len(), 2);
    assert_eq!(acts.asked(), 2);
}

#[test]
fn the_window_starts_a_browser_for_a_setup_step_and_is_given_the_fresh_view() {
    use crate::language::setup::STORE_LISTING_URL;

    let fixture = Fixture::setting_up();
    let computer = fixture
        .computer
        .clone()
        .expect("a fixture that tracks setup");
    computer.install("chrome");
    computer.find_profiles(vec![crate::install::browser_profiles::BrowserProfiles {
        browser: crate::install::native_host::BrowserIdentity {
            id: "chrome",
            name: "Google Chrome",
            short: "Chrome",
        },
        scan: crate::install::browser_profiles::ProfileScan {
            profiles: 2,
            with_extension: vec![crate::install::browser_profiles::ExtensionProfile {
                directory: "Profile 1".into(),
                name: Some("Work".into()),
            }],
        },
    }]);

    let added = fixture.control(post(
        "/api/setup/act",
        r#"{"kind":"add_in_browser","browser":"chrome"}"#,
    ));
    assert_eq!(added.status, 200);
    assert_eq!(
        added.json()["stage"],
        "browser",
        "the answer is the setup view"
    );
    let opened = fixture.control(post(
        "/api/setup/act",
        r#"{"kind":"open_browser","browser":"chrome","profile":"Profile 1"}"#,
    ));
    assert_eq!(opened.status, 200);
    let started = computer.started();
    assert_eq!(started.len(), 2);
    assert_eq!(started[0].arguments, [STORE_LISTING_URL]);
    assert_eq!(started[1].arguments, ["--profile-directory=Profile 1"]);

    // A step the computer does not offer is refused in words, and starts nothing.
    for body in [
        r#"{"kind":"open_browser","browser":"chrome","profile":"Profile 9"}"#,
        r#"{"kind":"add_in_browser","browser":"edge"}"#,
        r#"{"kind":"get_chrome"}"#,
    ] {
        let reply = fixture.control(post("/api/setup/act", body));
        assert_eq!(reply.status, 422, "{body}");
        assert_eq!(reply.code(), "refused", "{body}");
        assert_eq!(
            reply.json()["error"]["message"],
            words::SETUP_STEP_NOT_OFFERED
        );
    }
    for body in [
        "",
        "[]",
        "{}",
        r#"{"kind":"nothing"}"#,
        r#"{"kind":"add_in_browser"}"#,
    ] {
        let reply = fixture.control(post("/api/setup/act", body));
        assert_eq!(reply.status, 400, "{body}");
        assert_eq!(reply.code(), "bad_request", "{body}");
    }
    assert_eq!(computer.started().len(), 2);

    computer.end_the_session(true);
    let ending = fixture.control(post(
        "/api/setup/act",
        r#"{"kind":"add_in_browser","browser":"chrome"}"#,
    ));
    assert_eq!(ending.status, 422);
    assert_eq!(
        ending.json()["error"]["message"],
        words::SETUP_SESSION_ENDING
    );
    assert_eq!(computer.started().len(), 2, "nothing starts at sign-out");
}

#[test]
fn a_closed_view_cannot_start_a_browser() {
    let fixture = Fixture::closed();
    let reply = fixture.watch(post(
        "/api/setup/act",
        r#"{"kind":"add_in_browser","browser":"chrome"}"#,
    ));
    assert_eq!(reply.status, 403);
    assert_eq!(reply.code(), "watching_off");
}

#[test]
fn get_chrome_is_in_the_table_of_links_the_page_opens_through_the_service() {
    let fixture = Fixture::start();
    let links = fixture.control(get("/api/context")).json()["links"].clone();
    assert_eq!(links["get_chrome"], "https://www.google.com/chrome/");
    let acts = Arc::new(RecordingActs::default());
    fixture.facade.attach_native_acts(acts.clone());
    let reply = fixture.control(post("/api/open", r#"{"destination":"get_chrome"}"#));
    assert_eq!(reply.status, 200);
    assert_eq!(
        *acts.urls.lock().unwrap(),
        ["https://www.google.com/chrome/"]
    );
}

#[test]
fn the_window_shows_the_diagnostics_folder_and_a_missing_adapter_is_told_so() {
    let fixture = Fixture::start();
    let unavailable = fixture.control(post("/api/diagnostics/reveal", "{}"));
    assert_eq!(unavailable.status, 422);
    assert_eq!(unavailable.code(), "refused");

    let acts = Arc::new(RecordingActs::default());
    fixture.facade.attach_native_acts(acts.clone());
    assert_eq!(
        fixture
            .control(post("/api/diagnostics/reveal", "{}"))
            .status,
        200
    );
    assert_eq!(acts.folders.lock().unwrap().len(), 1);
    assert_eq!(
        fixture
            .control(post("/api/diagnostics/reveal", r#"{"extra":1}"#))
            .status,
        400
    );
}

#[test]
fn locating_a_client_answers_its_result_or_null_when_the_dialog_is_closed() {
    let fixture = Fixture::start();
    let acts = Arc::new(RecordingActs::default());
    fixture.facade.attach_native_acts(acts.clone());

    let closed = fixture.control(post("/api/integrations/locate", r#"{"id":"goose"}"#));
    assert_eq!(closed.status, 200);
    assert_eq!(closed.json(), Value::Null);
    assert!(closed.header("Ghostlight-Seq").is_some());

    let located = fixture.root.join("portable/config.yaml");
    std::fs::create_dir_all(located.parent().unwrap()).unwrap();
    std::fs::write(
        &located,
        "provider: local
",
    )
    .unwrap();
    *acts.chosen.lock().unwrap() = Some(located.clone());
    let result = fixture.control(post("/api/integrations/locate", r#"{"id":"goose"}"#));
    assert_eq!(result.status, 200);
    assert!(result.json()["summary"]["config_path"]
        .as_str()
        .unwrap()
        .ends_with("config.yaml"));

    let unknown = fixture.control(post("/api/integrations/locate", r#"{"id":"not-a-client"}"#));
    assert_eq!(unknown.status, 422);
}

#[test]
fn every_context_is_given_the_table_of_links_and_never_an_address_of_its_own() {
    let fixture = Fixture::start();
    let wanted: serde_json::Map<String, Value> = WorkbenchDestination::all()
        .into_iter()
        .map(|destination| (destination.key().to_owned(), json!(destination.url())))
        .collect();
    let closed = Fixture::closed();
    for reply in [
        fixture.control(get("/api/context")),
        fixture.watch(get("/api/context")),
        closed.watch(get("/api/context")),
    ] {
        let links = reply.json()["links"].clone();
        assert_eq!(links, Value::Object(wanted.clone()));
        for url in links.as_object().unwrap().values() {
            assert!(url.as_str().unwrap().starts_with("https://"));
        }
    }
}

#[test]
fn a_watcher_is_given_the_download_address_of_each_host_that_has_one() {
    let fixture = Fixture::start();
    let pages: HashMap<String, Option<String>> = fixture
        .facade
        .cached_harnesses()
        .into_iter()
        .map(|summary| (summary.id, summary.download_url))
        .collect();
    for reply in [
        fixture.watch(get("/api/snapshot?parts=integrations")),
        fixture.control(get("/api/snapshot?parts=integrations")),
    ] {
        let body = reply.json();
        let harnesses = body["parts"]["integrations"]["harnesses"]
            .as_array()
            .unwrap();
        let shown: HashMap<&str, Option<&str>> = harnesses
            .iter()
            .map(|harness| {
                assert_eq!(
                    harness["can_download"] == true,
                    harness.get("download_url").is_some()
                );
                (
                    harness["id"].as_str().unwrap(),
                    harness.get("download_url").and_then(Value::as_str),
                )
            })
            .collect();
        assert_eq!(shown.len(), pages.len());
        for (id, page) in &pages {
            assert_eq!(shown[id.as_str()], page.as_deref(), "{id}");
        }
        assert_ne!(shown["cline-vscode"], shown["cline-cli"]);
    }
    let rescan = fixture.watch(post("/api/integrations/rescan", "{}")).json();
    assert!(rescan
        .as_array()
        .unwrap()
        .iter()
        .any(|harness| harness.get("download_url").is_some()));
}

/// The marker a home holds only while watching is off.
fn watch_off_marker(fixture: &Fixture) -> PathBuf {
    fixture.root.join("state").join("watch.off")
}

fn set_viewing(fixture: &Fixture, watch: bool) -> Reply {
    fixture.control(
        Call::new("PUT", "/api/controls/viewing").body(&json!({ "watch": watch }).to_string()),
    )
}

#[test]
fn a_fresh_home_is_watched_and_the_controls_part_says_so_to_each_context() {
    let fixture = Fixture::start();
    assert!(fixture.facade.watching());
    assert!(!watch_off_marker(&fixture).exists());

    let window = fixture.control(get("/api/snapshot?parts=controls")).json();
    let controls = &window["parts"]["controls"];
    assert_eq!(controls["viewing"]["watch"], true);
    assert_eq!(controls["viewing"]["url"], fixture.server.address().url());
    assert_eq!(controls["viewing"]["word"], words::CONTROLS_VIEWING_ON);
    assert_eq!(
        controls["viewing"]["acts"]["set_watching"]["available"],
        true
    );
    assert_eq!(controls["you"]["context"], "control");

    let everything = fixture.control(get("/api/snapshot")).json();
    assert!(everything["parts"].get("controls").is_some());

    let tab = fixture.watch(get("/api/snapshot?parts=controls")).json();
    let seen = &tab["parts"]["controls"];
    assert_eq!(seen["you"]["context"], "watch");
    assert_eq!(seen["you"]["sentence"], words::SENTENCE_WATCH);
    assert_eq!(seen["viewing"]["acts"]["set_watching"]["available"], false);
    assert_eq!(seen["runtime"]["acts"]["hold"]["available"], true);
}

#[test]
fn turning_watching_off_closes_a_tab_at_once_and_the_window_keeps_its_stream() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    let mut tab = Stream::open(fixture.port(), None);
    window.hello();
    tab.hello();

    let off = set_viewing(&fixture, false);
    assert_eq!(off.status, 200);
    assert!(off.header("Ghostlight-Seq").is_some());
    let answer = off.json();
    assert_eq!(answer["viewing"]["watch"], false);
    assert_eq!(answer["viewing"]["word"], words::CONTROLS_VIEWING_OFF);
    assert_eq!(answer["you"]["sentence"], words::SENTENCE_CONTROL_PRIVATE);
    assert!(watch_off_marker(&fixture).exists());
    assert!(!fixture.facade.watching());

    // A request with no key is closed from this moment.
    assert_eq!(
        fixture.watch(get("/api/context")).json()["context"],
        "closed"
    );
    assert_eq!(fixture.watch(get("/api/snapshot")).status, 403);

    // The watching stream is told why it ended, and is over.
    let closed = tab.until(|block| block.event.as_deref() == Some("closed"));
    assert_eq!(closed.json()["sentence"], words::WATCHING_OFF);
    assert!(tab.next().is_none());

    // The window hears the new state, and keeps receiving.
    let sheet = window.until(|block| {
        block.is("controls_changed")
            && block.json()["change"]["controls"]["viewing"]["watch"] == false
    });
    assert_eq!(
        sheet.json()["change"]["controls"]["you"]["sentence"],
        words::SENTENCE_CONTROL_PRIVATE
    );
    assert_eq!(
        fixture
            .control(post("/api/runtime", r#"{"intent":"hold"}"#))
            .status,
        200
    );
    window.until(|block| {
        block.is("status_changed")
            && block.json()["change"]["status"]["service"]["runtime_state"] == "held"
    });
}

#[test]
fn turning_watching_on_removes_the_marker_and_a_tab_watches_again() {
    let fixture = Fixture::closed();
    assert!(watch_off_marker(&fixture).exists());
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    window.hello();
    assert_eq!(
        fixture.watch(get("/api/context")).json()["context"],
        "closed"
    );

    let on = set_viewing(&fixture, true);
    assert_eq!(on.status, 200);
    assert_eq!(on.json()["viewing"]["watch"], true);
    assert!(!watch_off_marker(&fixture).exists());
    assert_eq!(
        fixture.watch(get("/api/context")).json()["context"],
        "watch"
    );
    window.until(|block| {
        block.is("controls_changed")
            && block.json()["change"]["controls"]["viewing"]["watch"] == true
    });
}

#[test]
fn setting_watching_to_what_it_already_is_changes_nothing_and_publishes_nothing() {
    let fixture = Fixture::start();
    assert_eq!(set_viewing(&fixture, true).status, 200);
    let seq = fixture.facade.current_seq();
    assert_eq!(set_viewing(&fixture, true).status, 200);
    assert_eq!(fixture.facade.current_seq(), seq);
}

#[test]
fn the_watching_route_takes_exactly_one_boolean() {
    let fixture = Fixture::start();
    for body in [
        r#"{"watch":"yes"}"#,
        r#"{}"#,
        r#"{"watch":false,"extra":1}"#,
    ] {
        let reply = fixture.control(Call::new("PUT", "/api/controls/viewing").body(body));
        assert_eq!(reply.status, 400, "{body}");
        assert_eq!(reply.code(), "bad_request", "{body}");
    }
    assert!(fixture.facade.watching());
}

#[test]
fn a_facade_that_is_not_served_cannot_change_watching() {
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None, None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let error = facade.set_watching(false).unwrap_err();
    assert_eq!(error.to_string(), words::WATCHING_NOT_SERVED);
    assert!(facade.watching());
}

#[test]
fn a_facade_that_is_not_served_does_not_claim_anyone_can_watch() {
    let facade = WorkbenchFacade::new(
        WorkbenchProjection::default(),
        crate::workspace::WorkspaceStore::default(),
        GovernanceFacade::new(None, None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "epoch".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    let controls = facade.snapshot().controls;
    assert!(!controls.viewing.watch);
    assert_eq!(controls.viewing.url, None);
    assert_eq!(controls.viewing.word, words::WATCHING_NOT_SERVED);
    assert_eq!(controls.you.sentence, words::SENTENCE_CONTROL_PRIVATE);
    let act = &controls.viewing.acts.set_watching;
    assert!(!act.available);
    assert_eq!(act.why_not.as_deref(), Some(words::WATCHING_NOT_SERVED));
    let tab = controls.seen_by(WebContext::Watch);
    assert!(!tab.viewing.acts.set_watching.available);
}

#[test]
fn diagnostics_turned_on_from_a_terminal_reach_settings_within_one_refresher_tick() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    window.hello();
    fixture.facade.authority_refreshed();
    window.until(|block| block.is("controls_changed"));

    // The command line flips the same marker the window does, behind the facade's back.
    fixture.facade.diagnostics.toggle();
    fixture.facade.authority_refreshed();
    let sheet = window.until(|block| {
        block.is("controls_changed")
            && block.json()["change"]["controls"]["diagnostics"]["on"] == true
    });
    assert!(sheet.json()["change"]["controls"]["diagnostics"]["word"]
        .as_str()
        .unwrap()
        .starts_with("On ("));
}

#[test]
fn a_pause_changes_the_runtime_row_of_every_screens_controls() {
    let fixture = Fixture::start();
    let mut window = Stream::open(fixture.port(), Some(&fixture.bearer()));
    let mut tab = Stream::open(fixture.port(), None);
    window.hello();
    tab.hello();
    assert_eq!(
        fixture
            .watch(post("/api/runtime", r#"{"intent":"hold"}"#))
            .status,
        200
    );
    let held = |block: &Block| {
        block.is("controls_changed")
            && block.json()["change"]["controls"]["runtime"]["state"] == "held"
    };
    let seen_by_window = window.until(held);
    let seen_by_tab = tab.until(held);
    let runtime = |block: &Block| block.json()["change"]["controls"]["runtime"].clone();
    assert_eq!(runtime(&seen_by_window)["note"], words::PAUSED_FROM_WATCHER);
    assert_eq!(
        runtime(&seen_by_window)["acts"]["resume"]["available"],
        true
    );
    assert_eq!(runtime(&seen_by_tab)["acts"]["resume"]["available"], false);
    assert_eq!(
        seen_by_tab.json()["change"]["controls"]["you"]["context"],
        "watch"
    );
}

/// A cache under `fixture` that has found Zed's icon on a Linux computer, and the icon's bytes.
fn zed_icon_cache(fixture: &Fixture) -> (crate::install::icons::IconCache, Vec<u8>) {
    use crate::install::icons::{IconCache, IconPlaces};

    let share = fixture.root.join("share");
    let entry = share.join("applications").join("dev.zed.Zed.desktop");
    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(&entry, "[Desktop Entry]\nName=Zed\nIcon=zed\n").unwrap();
    let png = share.join("icons/hicolor/64x64/apps/zed.png");
    std::fs::create_dir_all(png.parent().unwrap()).unwrap();
    let bytes = b"\x89PNG\r\n\x1a\nzed".to_vec();
    std::fs::write(&png, &bytes).unwrap();
    let icons = fixture.root.join("state").join("icons");
    let cache = IconCache::open(icons, IconPlaces::linux(vec![share]), None);
    assert!(cache.refresh());
    (cache, bytes)
}

/// The headers an agent's image is served with, which a HEAD answers too.
const IMAGE_HEADERS: [&str; 6] = [
    "Content-Type",
    "Content-Length",
    "ETag",
    "X-Content-Type-Options",
    "Content-Security-Policy",
    "Cross-Origin-Resource-Policy",
];

#[test]
fn every_catalog_agent_has_one_image_address_and_it_serves_the_best_image_there_is() {
    let fixture = Fixture::start();
    let monogram = include_bytes!("../../agent/monograms/zed.svg");

    let first = fixture.watch(get("/agent-icons/zed"));
    assert_eq!(first.status, 200, "a watcher draws the agents it is shown");
    assert_eq!(first.header("Content-Type"), Some("image/svg+xml"));
    assert_eq!(first.body, monogram, "no icon is cached, so the monogram");
    assert_eq!(first.header("X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(
        first.header("Content-Security-Policy"),
        Some("default-src 'none'; sandbox")
    );
    assert_eq!(
        first.header("Cross-Origin-Resource-Policy"),
        Some("same-origin")
    );
    let monogram_tag = first.header("ETag").unwrap().to_owned();
    let version = crate::agent::image::served("zed", None).unwrap().version;
    assert_eq!(monogram_tag, format!("\"{version}\""));

    for id in ["codex", "claude-code", "cline", "goose"] {
        let reply = fixture.control(get(&format!("/agent-icons/{id}?v=0123456789ab")));
        assert_eq!(reply.status, 200, "{id}");
        assert_eq!(reply.header("Content-Type"), Some("image/svg+xml"), "{id}");
    }

    let (cache, bytes) = zed_icon_cache(&fixture);
    fixture.facade.attach_icons(cache.clone());
    let icon = fixture.control(get("/agent-icons/zed"));
    assert_eq!(icon.status, 200);
    assert_eq!(icon.header("Content-Type"), Some("image/png"));
    assert_eq!(icon.body, bytes, "the cached icon comes first");
    assert_eq!(
        icon.header("Content-Security-Policy"),
        Some("default-src 'none'; sandbox")
    );
    let icon_tag = icon.header("ETag").unwrap().to_owned();
    assert_ne!(
        icon_tag, monogram_tag,
        "the version follows the served image"
    );
    assert_eq!(
        icon_tag,
        format!("\"{}\"", cache.found("zed").unwrap().version)
    );
    assert_eq!(
        fixture
            .watch(get("/agent-icons/codex"))
            .header("Content-Type"),
        Some("image/svg+xml"),
        "an agent with no icon keeps its monogram"
    );

    let unchanged = fixture.watch(get("/agent-icons/zed").header("If-None-Match", &icon_tag));
    assert_eq!(unchanged.status, 304);
    let stale = fixture.watch(get("/agent-icons/zed").header("If-None-Match", &monogram_tag));
    assert_eq!(
        stale.status, 200,
        "an old version is answered with the new image"
    );

    for path in ["/agent-icons/zed", "/agent-icons/codex"] {
        let full = fixture.watch(get(path));
        let head = fixture.watch(Call::new("HEAD", path));
        assert_eq!(head.status, full.status, "{path}");
        assert!(head.body.is_empty(), "{path}");
        for name in IMAGE_HEADERS {
            assert_eq!(head.header(name), full.header(name), "{path} {name}");
        }
    }

    for path in [
        "/agent-icons/not-an-agent",
        "/agent-icons/command_line",
        "/agent-icons/unknown",
        "/agent-icons/",
        "/agent-icons/zed/more",
        "/agent-icons/..%2fzed",
        "/agent-icons/zed.json",
        "/integrations/zed.svg",
    ] {
        let reply = fixture.watch(get(path));
        assert_eq!(reply.status, 404, "{path}");
        assert_eq!(reply.code(), "not_found", "{path}");
    }
    assert_eq!(
        fixture.watch(post("/agent-icons/zed", "{}")).status,
        405,
        "an image is only read"
    );
}

#[test]
fn an_agents_image_answers_that_watching_is_off_and_never_shows_what_is_installed() {
    let fixture = Fixture::closed();
    let (cache, bytes) = zed_icon_cache(&fixture);
    fixture.facade.attach_icons(cache);

    let refusals: Vec<(u16, String, Vec<u8>)> = [
        "/agent-icons/zed",
        "/agent-icons/codex",
        "/agent-icons/not-an-agent",
    ]
    .into_iter()
    .flat_map(|path| [get(path), Call::new("HEAD", path)])
    .map(|call| {
        let reply = fixture.watch(call);
        assert!(!reply.body.starts_with(&bytes[..4]));
        (reply.status, reply.code(), reply.body)
    })
    .collect();
    for (status, code, _) in &refusals {
        assert_eq!(*status, 403);
        assert_eq!(code, "watching_off");
    }
    let gets: Vec<&Vec<u8>> = refusals
        .iter()
        .step_by(2)
        .map(|(_, _, body)| body)
        .collect();
    assert!(
        gets.windows(2).all(|pair| pair[0] == pair[1]),
        "every id is refused the same way, so a refusal says nothing about the agent"
    );

    let window = fixture.control(get("/agent-icons/zed"));
    assert_eq!(window.status, 200, "the window draws its agents either way");
    assert_eq!(window.body, bytes);
}
