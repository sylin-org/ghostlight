// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H3 identity tests (ADR-0030 Decision 4; `docs/tasks/hub/H3-session-identity-guid.md`).
//!
//! 1. `guid_is_v4_csprng_and_bound_to_minting_peer` -- `SessionGuid::mint()` produces distinct,
//!    parseable v4 UUIDs whose `Display`/`Debug` never leak the raw string, and
//!    `SessionRegistry::admit` binds first presentation, allowing the SAME peer to re-present.
//! 2. `foreign_peer_presenting_a_guid_is_refused` -- a DIFFERENT OS user presenting an
//!    already-bound guid is refused; the original binding is unchanged (the sanctioned same-user
//!    reuse path, a different pid, still admits).
//! 3. `relay_adapter_sends_a_real_guid_not_a_placeholder` -- drives the real `ipc::relay_adapter`
//!    against a fake test service and reads the framed hello it sends, proving the old `""`
//!    placeholder guid (PINS.md SS9, item 7) was actually replaced.

use ghostlight::hub::session::{Admission, PeerCred, PeerUser, SessionGuid, SessionRegistry};
use ghostlight::native::host;
use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};

static SEQ: AtomicU32 = AtomicU32::new(0);

fn unique_endpoint(tag: &str) -> String {
    format!(
        "ghostlight-hub-identity-test-{}-{}-{tag}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

#[test]
fn guid_is_v4_csprng_and_bound_to_minting_peer() {
    let g1 = SessionGuid::mint();
    let g2 = SessionGuid::mint();

    for g in [&g1, &g2] {
        let parsed =
            uuid::Uuid::parse_str(g.as_str()).expect("mint() produces a parseable UUID string");
        assert_eq!(
            parsed.get_version(),
            Some(uuid::Version::Random),
            "mint() produces a version-4 (random) UUID"
        );
    }
    assert_ne!(
        g1.as_str(),
        g2.as_str(),
        "two mints never collide (CSPRNG, not a counter)"
    );

    // Non-leak invariant (ADR-0030 Decision 4: "Treat the GUID as secret in logs/audit").
    assert!(
        !format!("{g1}").contains(g1.as_str()),
        "Display must not leak the raw canonical guid"
    );
    assert!(
        !format!("{g1:?}").contains(g1.as_str()),
        "Debug must not leak the raw canonical guid"
    );

    let a = PeerCred {
        user: PeerUser("user-A".into()),
        pid: 100,
    };
    let mut registry = SessionRegistry::new();
    assert_eq!(
        registry.admit(&g1, &a),
        Admission::Admitted,
        "first presentation binds the guid to its peer"
    );
    assert_eq!(
        registry.admit(&g1, &a),
        Admission::Admitted,
        "the SAME peer re-presenting the same guid reuses the binding"
    );
}

#[test]
fn foreign_peer_presenting_a_guid_is_refused() {
    let g = SessionGuid::mint();
    let mut registry = SessionRegistry::new();
    let a = PeerCred {
        user: PeerUser("user-A".into()),
        pid: 100,
    };
    assert_eq!(registry.admit(&g, &a), Admission::Admitted);

    let b = PeerCred {
        user: PeerUser("user-B".into()),
        pid: 200,
    };
    assert_eq!(
        registry.admit(&g, &b),
        Admission::Refused,
        "a DIFFERENT OS user presenting an already-bound guid is refused"
    );

    let a2 = PeerCred {
        user: PeerUser("user-A".into()),
        pid: 999,
    };
    assert_eq!(
        registry.admit(&g, &a2),
        Admission::Admitted,
        "the original binding is unchanged: the SAME user, a different pid, still admits \
         (the sanctioned same-user reuse path)"
    );
}

/// PINS.md SS9 item 7: `relay_adapter` used to send a placeholder empty `"guid": ""`. This test
/// drives the REAL `ipc::relay_adapter` against a fake test service (a bare claimed
/// adapter/control endpoint) and reads back the ONE framed hello frame it sends, asserting its
/// `guid` field is non-empty and parses as a valid canonical v4 `SessionGuid` -- proving the
/// placeholder was actually replaced, not merely described as fixed.
#[tokio::test]
async fn relay_adapter_sends_a_real_guid_not_a_placeholder() {
    let endpoint = unique_endpoint("relay-guid");
    let listener = ghostlight::native::ipc::claim_adapter_endpoint(&endpoint)
        .await
        .expect("claim the adapter/control endpoint as a fake test service");

    let debug = ghostlight::debug::DebugSink::disabled();
    let relay_endpoint = endpoint.clone();
    // Fire-and-forget: past the hello, `relay_adapter` becomes a raw bidirectional relay of this
    // test process's own stdio, which never naturally completes here. We only need the ONE hello
    // frame it sends before that; the task is simply dropped (never awaited) once we have it.
    tokio::spawn(async move {
        let _ = ghostlight::native::ipc::relay_adapter(&relay_endpoint, &debug).await;
    });

    let hello_bytes = accept_one_hello(listener).await;
    let hello: Value = serde_json::from_slice(&hello_bytes).expect("hello is well-formed JSON");
    assert_eq!(hello["hub"], 1);
    assert_eq!(hello["role"], "adapter");
    let guid = hello
        .get("guid")
        .and_then(Value::as_str)
        .expect("the hello carries a string guid field");
    assert!(
        !guid.is_empty(),
        "relay_adapter must not send the old placeholder empty guid"
    );
    assert!(
        SessionGuid::parse(guid).is_some(),
        "relay_adapter's guid parses as a valid canonical v4 SessionGuid: {guid:?}"
    );
}

#[cfg(windows)]
async fn accept_one_hello(mut listener: ghostlight::native::ipc::AdapterListener) -> Vec<u8> {
    listener
        .connect()
        .await
        .expect("accept the relay's connection as a fake service");
    host::read_message(&mut listener)
        .await
        .expect("read the framed hello")
        .expect("the hello frame is present")
}

#[cfg(unix)]
async fn accept_one_hello(listener: ghostlight::native::ipc::AdapterListener) -> Vec<u8> {
    let (mut stream, _) = listener
        .accept()
        .await
        .expect("accept the relay's connection as a fake service");
    host::read_message(&mut stream)
        .await
        .expect("read the framed hello")
        .expect("the hello frame is present")
}
