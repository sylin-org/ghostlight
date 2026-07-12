// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H4 binary-authoritative cross-session tab isolation tests (ADR-0030 Decision 6;
//! `docs/tasks/hub/H4-binary-authoritative-isolation.md`; oracles PINNED in
//! `docs/tasks/hub/PINS.md` SS3).
//!
//! Decision 6 (verbatim, ADR-0030): "The service tracks, per session (keyed on Decision 4's
//! GUID), the set of tabIds that session created (`tabs_create_mcp`) or legitimately adopted.
//! Before routing any tab-scoped call OR resolving policy for it -- i.e. BEFORE any `tab_url`
//! probe -- the service refuses a tabId the session does not own, returning a uniform "unknown
//! tab" result that leaks neither the tab's existence nor its host (closing the cross-session
//! host-enumeration channel). Owned-handle sets live in `src/hub` (opaque handles that may name a
//! tabId); the governance core stays handle-agnostic. The extension's per-group checks remain
//! defense-in-depth only. A lone all-open session owns everything it touches, so the all-open
//! path stays a byte-identical pass-through."
//!
//! Session A's ownership of a tab is seeded directly on the shared `owned_tabs` map (the task
//! file's own sanctioned test shortcut: "Session A creates/owns tab 5 (via `tabs_create_mcp`
//! returning tabId 5, OR the H3-established ownership path)") rather than driven through a live
//! session -- the pinned assertions are about session B's refusal, not about how A came to own
//! the tab. Both tests drive session B for real, through the unchanged `serve_session` /
//! `handle_line` / `pipeline::handle_tools_call` chain, over a real `Browser` with a fake
//! extension attached (mirroring `pipeline.rs`'s own `attach_fake_extension_with_tab_urls`: a
//! `tab_url_request` for an unregistered tabId, or a `tool_request` for an unregistered tool,
//! PANICS -- proof that a leaked probe/dispatch fails the test loudly rather than silently).

use ghostlight::governance::audit::Recorder;
use ghostlight::governance::config::reload::ConfigStore;
use ghostlight::governance::manifest::source::LoadedPolicy;
use ghostlight::hub::outbound::browser::Browser;
use ghostlight::hub::session::{SessionGuid, SessionRegistry};
use ghostlight::hub::ServiceContext;
use ghostlight::native::host;
use ghostlight::transport::mcp::server::serve_session;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

/// `serve_session` asserts `crate::hub::role::assert_service_role` as its first line (PINS.md
/// SS8): this test drives it directly (never through `run_service`/`run_service_loop`, which is
/// what normally sets the role marker in production), so it must set the ONE-per-process role
/// marker itself, exactly once for the whole test binary (`role::set_role` panics if called twice; multiple
/// `#[tokio::test]` functions in this file run in the SAME process).
static SET_SERVICE_ROLE: Once = Once::new();
fn ensure_service_role() {
    SET_SERVICE_ROLE.call_once(|| {
        ghostlight::hub::role::set_role(ghostlight::hub::role::Role::Service);
    });
}

/// Mirrors `src/transport/mcp/pipeline.rs`'s own `attach_fake_extension_with_tab_urls` test
/// double (task file: "use the same fake-extension pattern ... so a `tab_url_request` for an
/// unregistered tabId PANICS, which is how this test proves NO probe fired"). Panics on any
/// unregistered `tab_url_request` OR `tool_request`, so a leaked cross-session probe/dispatch
/// fails the test loudly instead of silently. `seen` records one entry per frame the fake
/// extension actually answers.
fn attach_fake_extension(
    browser: &Browser,
    responses: Vec<(&'static str, Value)>,
    tab_urls: Vec<(i64, Option<&'static str>)>,
) -> (tokio::task::JoinHandle<()>, Arc<Mutex<Vec<String>>>) {
    let (browser_side, mut ext_side) = tokio::io::duplex(64 * 1024);
    let attached = browser.clone();
    tokio::spawn(async move {
        let _ = attached.attach(browser_side).await;
    });

    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_task = Arc::clone(&seen);
    let responses: HashMap<&'static str, Value> = responses.into_iter().collect();
    let tab_urls: HashMap<i64, Option<&'static str>> = tab_urls.into_iter().collect();
    let handle = tokio::spawn(async move {
        // ADR-0058: identify as pid 0, matching a plain un-encoded small tabId's decode.
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        host::write_message(&mut ext_side, &hello).await.unwrap();
        loop {
            let Some(req) = host::read_message(&mut ext_side).await.unwrap() else {
                break;
            };
            let v: Value = serde_json::from_slice(&req).unwrap();
            if v["type"] == "tab_url_request" {
                let tab_id = v["tabId"]
                    .as_i64()
                    .expect("tab_url_request carries a tabId");
                seen_for_task
                    .lock()
                    .unwrap()
                    .push(format!("tab_url_request:{tab_id}"));
                let url = *tab_urls
                    .get(&tab_id)
                    .unwrap_or_else(|| panic!("unexpected tab_url_request for tabId {tab_id}"));
                let reply =
                    json!({ "id": v["id"], "type": "tab_url_response", "result": { "url": url } });
                host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                    .await
                    .unwrap();
                continue;
            }
            let tool = v["tool"].as_str().unwrap().to_string();
            seen_for_task.lock().unwrap().push(tool.clone());
            let result = responses
                .get(tool.as_str())
                .cloned()
                .unwrap_or_else(|| panic!("unexpected tool_request for '{tool}'"));
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": result });
            host::write_message(&mut ext_side, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        }
    });
    (handle, seen)
}

async fn wait_connected(browser: &Browser) {
    for _ in 0..200 {
        if browser.is_connected() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("browser never reported connected");
}

/// Build a `ServiceContext` for the test: a fresh `Browser`, an all-open `LoadedPolicy`, a real
/// `ConfigStore::load_initial` (the SAME all-open resolution the real binary performs at startup
/// with no manifest present -- `tests/all_open_golden.rs`'s own subprocess test already relies on
/// this resolving to all-open in this environment), a disabled `Recorder`, a fresh
/// `SessionRegistry`, and a fresh, empty `owned_tabs` map (H4's new shared state) the caller
/// pre-seeds per scenario -- "the H3-established ownership path" the task file sanctions as an
/// alternative to a live `tabs_create_mcp` round trip.
fn build_ctx(browser: Browser) -> ServiceContext {
    let store = ConfigStore::load_initial(ghostlight::browser::pattern::is_valid_pattern)
        .expect("load_initial resolves to all-open with no manifest present");
    ServiceContext {
        capabilities: ghostlight::hub::outbound::Registry::new(vec![std::sync::Arc::new(
            ghostlight::hub::outbound::browser::BrowserCapability::new(browser.clone()),
        )]),
        browser,
        store,
        recorder: Arc::new(Recorder::disabled()),
        initial_policy: LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        },
        session_registry: Arc::new(Mutex::new(SessionRegistry::new())),
        owned_tabs: Arc::new(Mutex::new(HashMap::new())),
        session_titles: Arc::new(Mutex::new(HashMap::new())),
        live_guids: Arc::new(Mutex::new(HashMap::new())),
        mint_quota: Arc::new(Mutex::new(HashMap::new())),
        live_sessions: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        debug_sink: ghostlight::observability::DebugSink::disabled(),
    }
}

/// Drive one MCP session over an in-process duplex pair: `serve_session` on one end (spawned,
/// dropped when the test's own `client` half is dropped), this test on the other.
fn drive_session(ctx: ServiceContext, guid: SessionGuid) -> DuplexStream {
    ensure_service_role();
    let (client, server) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let _ = serve_session(server, ctx, guid).await;
    });
    client
}

/// Write one `tools/call` request line and read back one raw JSON-RPC reply line.
async fn call(reader: &mut BufReader<DuplexStream>, id: i64, name: &str, args: Value) -> Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": args },
    });
    let mut line = serde_json::to_string(&req).expect("request serializes");
    line.push('\n');
    reader
        .get_mut()
        .write_all(line.as_bytes())
        .await
        .expect("write the raw JSON-RPC request line");
    let mut reply_line = String::new();
    reader
        .read_line(&mut reply_line)
        .await
        .expect("read one raw JSON-RPC reply line");
    serde_json::from_str(reply_line.trim_end()).expect("reply is well-formed JSON")
}

fn result_text(resp: &Value) -> &str {
    resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content block")
}

/// `tests/hub_isolation.rs::unowned_tab_is_refused_before_any_tab_url_probe` (task file, BY NAME).
///
/// Session A owns tab 5 (seeded directly on the shared map -- "the H3-established ownership
/// path"). Session B (a DIFFERENT guid) then issues a tab-scoped call naming tab 5.
///
/// Pinned assertions (task file, transcribed):
/// - The result text for B's call EQUALS the uniform unknown-tab string `unknown tab` (PINNED in
///   PINS.md SS3). It is a success result, never `isError: true`.
/// - The fake extension recorded ZERO frames for B's call: `seen` stays empty (no
///   `tab_url_request:5`, no `read_page` entry) -- refused before the probe at
///   `pipeline.rs:118` and before dispatch.
#[tokio::test]
async fn unowned_tab_is_refused_before_any_tab_url_probe() {
    let browser = Browser::new();
    let (_fake_ext, seen) = attach_fake_extension(
        &browser,
        vec![(
            "read_page",
            json!({ "content": [{ "type": "text", "text": "page text" }] }),
        )],
        vec![(5, Some("https://a-host.example/"))],
    );
    wait_connected(&browser).await;

    let ctx = build_ctx(browser);
    let guid_a = SessionGuid::mint();
    let guid_b = SessionGuid::mint();
    // Session A owns tab 5 -- the task file's own sanctioned shortcut ("the H3-established
    // ownership path"), bypassing a live `tabs_create_mcp` round trip for A.
    ctx.owned_tabs.lock().unwrap().insert(5, guid_a.clone());
    // ADR-0047 D5: A is a LIVE owner (the test's premise), so B's cross-session reference to A's
    // tab is REFUSED, not adopted (dead-owner adoption applies only to a guid with no live
    // connection). Seed A's liveness directly, mirroring the direct ownership seed above.
    ctx.live_guids
        .lock()
        .unwrap()
        .insert(guid_a.as_str().to_string(), 1);

    let client_b = drive_session(ctx, guid_b);
    let mut reader_b = BufReader::new(client_b);

    let resp = call(&mut reader_b, 1, "read_page", json!({ "tabId": 5 })).await;

    assert_ne!(
        resp["result"]["isError"], true,
        "a refusal is a success result, never isError: {resp:?}"
    );
    assert_eq!(
        result_text(&resp),
        "unknown tab",
        "PINS.md SS3: the uniform unknown-tab string"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "the fake extension must record ZERO frames for B's refused call: {:?}",
        seen.lock().unwrap()
    );
}

/// `tests/hub_isolation.rs::unknown_tab_result_leaks_no_host_or_existence` (task file, BY NAME).
///
/// Session B issues the SAME tab-scoped call twice: once naming a tabId owned by session A (the
/// tab EXISTS, on a distinctive host, `secret-host.example`) and once naming a tabId that no
/// session owns and no extension knows (does NOT exist) -- seeded as owned by A too, so B's
/// reference to it is refused by the SAME cross-session mechanism rather than first-touch-adopted
/// (Decision 6's gate cannot and must not distinguish "exists but owned by someone else" from
/// "does not exist at all" -- that is the leak being closed).
///
/// Pinned assertions (task file, transcribed):
/// - `assert_eq!(text_for_existing_other_session_tab, text_for_nonexistent_tab)`, both equal to
///   the PINNED uniform string `unknown tab`.
/// - Neither text contains the owning tab's host substring (`secret-host`).
#[tokio::test]
async fn unknown_tab_result_leaks_no_host_or_existence() {
    let browser = Browser::new();
    let (_fake_ext, seen) = attach_fake_extension(
        &browser,
        vec![(
            "read_page",
            json!({ "content": [{ "type": "text", "text": "page text" }] }),
        )],
        vec![(5, Some("https://secret-host.example/account"))],
    );
    wait_connected(&browser).await;

    let ctx = build_ctx(browser);
    let guid_a = SessionGuid::mint();
    let guid_b = SessionGuid::mint();
    // Tab 5 exists (on the distinctive host); tab 999 is absent from every table (the fake
    // extension has zero configuration for it -- "no extension knows [it]"). Both are owned by
    // A, so B's reference to either is refused by the SAME cross-session-ownership mechanism.
    // ADR-0047 D5: A is a LIVE owner, so B's references to A's tabs are REFUSED, not adopted
    // (dead-owner adoption applies only to a guid with no live connection). Seed A's liveness
    // directly, mirroring the direct ownership seeds below.
    ctx.live_guids
        .lock()
        .unwrap()
        .insert(guid_a.as_str().to_string(), 1);
    {
        let mut owned = ctx.owned_tabs.lock().unwrap();
        owned.insert(5, guid_a.clone());
        owned.insert(999, guid_a);
    }

    let client_b = drive_session(ctx, guid_b);
    let mut reader_b = BufReader::new(client_b);

    let existing_resp = call(&mut reader_b, 1, "read_page", json!({ "tabId": 5 })).await;
    let nonexistent_resp = call(&mut reader_b, 2, "read_page", json!({ "tabId": 999 })).await;

    let existing_text = result_text(&existing_resp).to_string();
    let nonexistent_text = result_text(&nonexistent_resp).to_string();

    assert_eq!(
        existing_text, nonexistent_text,
        "the uniform message is identical whether or not the tab exists"
    );
    assert_eq!(existing_text, "unknown tab", "PINS.md SS3 uniform string");
    assert_eq!(
        nonexistent_text, "unknown tab",
        "PINS.md SS3 uniform string"
    );
    assert!(
        !existing_text.contains("secret-host"),
        "no host leak: {existing_text}"
    );
    assert!(
        !nonexistent_text.contains("secret-host"),
        "no host leak: {nonexistent_text}"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "the fake extension must record ZERO frames for either of B's refused calls: {:?}",
        seen.lock().unwrap()
    );
}
