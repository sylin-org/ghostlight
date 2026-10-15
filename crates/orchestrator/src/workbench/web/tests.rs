//! Tests of the workbench server over real loopback sockets.
//!
//! Each test starts a server over its own facade and temporary home, and talks to it the way a
//! browser tab or the window would. A server takes the default port of its home or, when another
//! test holds it, a port the system picks; no test depends on which.

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
use crate::workbench::parts::{Part, USER_HOME_VARIABLE};
use crate::workbench::WorkbenchPresentationPort;
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

/// A presentation port that counts how often the window was asked to come forward.
#[derive(Default)]
struct CountingWindow(std::sync::atomic::AtomicUsize);

impl WorkbenchPresentationPort for CountingWindow {
    fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn notify(&self, _: WorkbenchNotification) -> Result<(), WorkbenchPresentationError> {
        Ok(())
    }
}

/// A server over its own facade and temporary home.
struct Fixture {
    server: WorkbenchServer,
    facade: WorkbenchFacade,
    root: PathBuf,
}

impl Fixture {
    fn start() -> Self {
        Self::build(true, NO_POLICY)
    }

    fn closed() -> Self {
        Self::build(false, NO_POLICY)
    }

    fn with_policy(policy: &str) -> Self {
        Self::build(true, policy)
    }

    fn build(watching: bool, policy: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("ghostlight-web-{}", uuid::Uuid::new_v4().simple()));
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
        }
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
        let _ = std::fs::remove_dir_all(&self.root);
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
        assert!(body["parts"][part.token()].is_object(), "{}", part.token());
    }
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
    let fixture = Fixture::start();
    let tag = "none";
    let calls = [
        Call::new("PUT", "/api/policy/user")
            .header("If-Match", tag)
            .body(&json!({ "document": READ_POLICY }).to_string()),
        Call::new("DELETE", "/api/policy/user").header("If-Match", tag),
        Call::new("PUT", "/api/controls/diagnostics").body(r#"{"on":true}"#),
        post("/api/integrations/setup-all", "{}"),
        post(
            "/api/integrations/act",
            r#"{"id":"cursor","action":"install","expected_state":"not_detected"}"#,
        ),
        post("/api/notifications/test", "{}"),
    ];
    for call in calls {
        let label = format!("{} {}", call.method, call.path);
        let reply = fixture.watch(call);
        assert_eq!(reply.status, 401, "{label}");
        assert_eq!(reply.code(), "needs_window", "{label}");
    }
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
    ];
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
    assert_eq!(window.0.load(std::sync::atomic::Ordering::SeqCst), 4);
    let unknown = fixture.control(post("/api/window", r#"{"view":"setup"}"#));
    assert_eq!(unknown.status, 400);
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
        let directory = body["parts"]["diagnostics"]["process_diagnostics"]["directory"]
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
        "/integrations/PROVENANCE.md",
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
