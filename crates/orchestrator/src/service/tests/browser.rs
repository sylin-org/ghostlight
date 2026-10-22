//! What a connected browser reports, the release of a session's tabs, and the browser the next
//! service remembers.

use super::*;

/// ACCEPTANCE B15 (ADR-0164) and defect d8d27be0: a page the person loads in a controlled tab
/// never authorizes, audits or reaches the executor, and a tab Ghostlight never opened stays
/// unowned; the next agent request checks the current page and is refused there.
#[test]
fn passive_browsing_never_becomes_work_or_claims_a_child_tab() {
    use crate::audit::{AuditRecorder, JsonlAuditSink};
    use crate::browser::adapter::{BrowserEvent, BrowserReadiness, PhysicalTab};
    use crate::browser::BrowserEventSink;
    use crate::governance::GovernanceFacade;
    use crate::presentation::{BrowserPresentation, PresentationReactor};
    use crate::work::{ApplicationExecutor, CancellationToken};
    use crate::workbench::{WorkbenchFacade, WorkbenchProjection};
    use crate::workspace::WorkspaceStore;

    let directory = temp_home("passive-browsing");
    let policy = directory.join("policy.json");
    std::fs::write(&policy, r#"{"schema":4,"name":"test","version":"1","grants":[{"id":"read","hosts":{"allow":["allowed.example"]},"allowed":["read"]}]}"#).unwrap();
    let governance = GovernanceFacade::new(Some(policy));
    let workspaces = WorkspaceStore::default();
    let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
    let tab = PhysicalTab {
        tab_id: 7,
        url: "https://allowed.example/".into(),
        title: "test".into(),
        active: true,
        readiness: BrowserReadiness::Complete,
    };
    let lease = workspaces.acquire(&workspace).unwrap();
    let selected = lease.add_tab(FAKE_BROWSER, &tab).unwrap();
    let selected = lease.apply_landing(&selected.handle, &tab).unwrap();
    drop(lease);
    let browser = Arc::new(FakeBrowser::default());
    let diagnostics = crate::diagnostics::DiagnosticsHub::for_tests();
    let projection = WorkbenchProjection::default();
    let audit_path = directory.join("audit.jsonl");
    let executor = ApplicationExecutor::new(
        governance.clone(),
        workspaces.clone(),
        browser.clone(),
        PresentationReactor::new(Arc::new(BrowserPresentation::new(
            browser.clone(),
            workspaces.clone(),
        ))),
        projection.clone(),
        Arc::new(AuditRecorder::new(
            Arc::new(JsonlAuditSink::new(&audit_path)),
            projection.clone(),
        )),
        diagnostics.clone(),
    );
    let view = WorkbenchFacade::new(
        projection,
        workspaces.clone(),
        governance,
        Arc::new(crate::browser::RelayBrowserPort::new(
            "passive-test".into(),
            crate::service::page_assets(),
        )),
        diagnostics,
    );
    let sink = crate::service::ServiceBrowserEvents::new(view.clone(), workspaces.clone());

    let initial_audit = std::fs::read_to_string(&audit_path).unwrap_or_default();

    // A commit never authorizes, audits or reaches the executor, whoever caused it.
    for url in [
        "chrome://version",
        "https://denied.example/private?secret=1",
        "https://allowed.example/",
    ] {
        sink.on_event(
            FAKE_BROWSER,
            BrowserEvent::DocumentCommitted {
                tab_id: 7,
                url: url.into(),
            },
        );
        let current = workspaces
            .acquire(&workspace)
            .unwrap()
            .select_tab(Some(selected.handle.as_str()))
            .unwrap();
        assert_eq!(current.url, url);
        assert!(current.generation > selected.generation);
        assert!(view.look().activity().history.is_empty());
        assert!(view.look().activity().operations.is_empty());
        assert!(browser.calls().is_empty());
        assert!(browser.control_states().is_empty());
        assert_eq!(
            std::fs::read_to_string(&audit_path).unwrap_or_default(),
            initial_audit
        );
    }
    // A tab Ghostlight never opened stays entirely unowned when it commits.
    sink.on_event(
        FAKE_BROWSER,
        BrowserEvent::DocumentCommitted {
            tab_id: 8,
            url: "chrome://version".into(),
        },
    );
    assert!(workspaces.owner_of_physical(FAKE_BROWSER, 8).is_none());
    assert!(browser.calls().is_empty());
    assert_eq!(
        std::fs::read_to_string(&audit_path).unwrap_or_default(),
        initial_audit
    );

    // It is the next agent request, not the person's visit, that receives a refusal.
    sink.on_event(
        FAKE_BROWSER,
        BrowserEvent::DocumentCommitted {
            tab_id: 7,
            url: "chrome://version".into(),
        },
    );
    let result = executor.execute(
        &workspace,
        "browser_read",
        json!({"tab":selected.handle.as_str()}),
        &CancellationToken::default(),
    );
    assert_eq!(result.status, Status::Blocked);
    assert!(browser
        .calls()
        .iter()
        .all(|call| matches!(call, BrowserCommand::Present { .. })));
    let history = view.look().activity().history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].tool, "browser_read");
    let bytes = std::fs::read_to_string(&audit_path).unwrap();
    assert!(!bytes.contains("browser_landing"));
    assert!(!bytes.contains("secret"));
}

/// The name the client of the session whose tabs the release cleanup tests leave behind claims.
const RELEASED_CLIENT: &str = "released";

/// Admit a workspace in the fake browser, land one example.com tab per id, and release it.
fn release_example_tabs(
    workspaces: &crate::workspace::WorkspaceStore,
    ids: &[u64],
) -> ReleasedTabs {
    use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

    let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp);
    let lease = workspaces.acquire(&workspace).unwrap();
    for &tab_id in ids {
        let tab = PhysicalTab {
            tab_id,
            url: "https://example.com/".into(),
            title: "Example".into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        };
        let added = lease.add_tab(FAKE_BROWSER, &tab).unwrap();
        lease.apply_landing(&added.handle, &tab).unwrap();
    }
    drop(lease);
    workspaces.release(&workspace)
}

/// The tab a later session of the same client opening example.com would reuse, if any.
fn reusable_example_tab(workspaces: &crate::workspace::WorkspaceStore) -> Option<u64> {
    let key = ConnectionEvidence::claimed(RELEASED_CLIENT, IntakeChannel::Mcp)
        .reuse_key()
        .expect("the claim names an agent");
    workspaces
        .claim_released(FAKE_BROWSER, &key, "https://example.com/")
        .map(|released| released.physical_id)
}

/// ACCEPTANCE B25, ADR-0196 Decisions 8 and 9, and defect 740d5e77: release cleanup clears the
/// tabs' diagnostics in batches of the size the extension accepts before any close, and a tab
/// becomes reusable only once the preserve-tabs interlock kept it open.
#[test]
fn release_cleanup_clears_diagnostics_in_batches_before_any_close_and_only_a_tab_kept_open_is_reusable(
) {
    use crate::browser::adapter::BrowserOutcome;
    use crate::browser::BrowserError;
    use crate::workspace::WorkspaceStore;

    // Every batch of diagnostics is cleared before the first close is attempted.
    let browser = FakeBrowser::default();
    let ids: Vec<u64> = (1..=crate::service::DIAGNOSTIC_CLEAR_BATCH_SIZE as u64 + 1).collect();
    let released = ReleasedTabs {
        browser: Some(FAKE_BROWSER.into()),
        physical_ids: ids.clone(),
    };
    cleanup_released_tabs(&released, &browser, &WorkspaceStore::default(), None);
    let (batch, rest) = ids.split_at(crate::service::DIAGNOSTIC_CLEAR_BATCH_SIZE);
    let closes: Vec<BrowserCommand> = ids
        .iter()
        .map(|&tab_id| BrowserCommand::CloseTab {
            tab_id,
            released: true,
        })
        .collect();
    let calls = browser.calls();
    assert_eq!(
        calls[..2],
        [
            BrowserCommand::ClearDiagnostics {
                tab_ids: batch.to_vec()
            },
            BrowserCommand::ClearDiagnostics {
                tab_ids: rest.to_vec()
            },
        ]
    );
    assert_eq!(calls[2..], closes[..]);

    // Only a tab the browser kept open after its close is reusable, and only once.
    let workspaces = WorkspaceStore::default();
    let mut released = release_example_tabs(&workspaces, &[1, 2, 3]);
    // The fake answers in call order, so close the tabs in a known order.
    released.physical_ids.sort_unstable();
    // Until its close answers, the browser may still bind each tab to the dead session, and
    // that close could still act on whoever took it, so nothing is claimable yet.
    assert_eq!(reusable_example_tab(&workspaces), None);

    let browser = FakeBrowser::default();
    browser.push(Ok(BrowserOutcome::DiagnosticsCleared { cleared_count: 3 }));
    // The browser closes tab 1, keeps tab 2 open by the person's preserve-tabs setting, and
    // never answers for tab 3 in time.
    browser.push(Ok(BrowserOutcome::TabClosed { tab_id: 1 }));
    browser.push(Err(BrowserError::LocalInterlock("preserved".into())));
    browser.push(Err(BrowserError::DeadlineAfterDispatch));
    cleanup_released_tabs(&released, &browser, &workspaces, None);

    assert_eq!(
        browser.calls(),
        vec![
            BrowserCommand::ClearDiagnostics {
                tab_ids: vec![1, 2, 3]
            },
            BrowserCommand::CloseTab {
                tab_id: 1,
                released: true
            },
            BrowserCommand::CloseTab {
                tab_id: 2,
                released: true
            },
            BrowserCommand::CloseTab {
                tab_id: 3,
                released: true
            },
        ]
    );
    assert_eq!(reusable_example_tab(&workspaces), Some(2));
    assert_eq!(reusable_example_tab(&workspaces), None);
}

/// Defect C4 (e4f353ab, findings T-02 and X-01) and ADR-0203 Decision 1: a hello that registers
/// a browser id again replaces its connection, seen to end or not, so the bindings in that
/// browser wait for it to confirm them instead of acting on a reused tab id.
#[test]
fn a_registration_that_replaces_a_connection_marks_its_bindings_pending() {
    use crate::browser::adapter::{BrowserReadiness, PhysicalTab};

    let home = TestHome::new("registration-pending");
    let served = serve_relayed(&home);
    let workspaces = served.context.workspaces.clone();
    served
        .relay
        .set_event_sink(Arc::new(crate::service::ServiceBrowserEvents::new(
            test_workbench(&workspaces),
            workspaces.clone(),
        )));
    let _first = attach_returning_browser(&served);
    let workspace = workspaces.admit(RELEASED_CLIENT.into(), IntakeChannel::Mcp);
    let lease = workspaces.acquire(&workspace).unwrap();
    let tab = PhysicalTab {
        tab_id: RETURNING_TAB,
        url: RETURNING_URL.into(),
        title: "Example".into(),
        active: true,
        readiness: BrowserReadiness::Complete,
    };
    let added = lease.add_tab(RETURNING_BROWSER, &tab).unwrap();
    lease.apply_landing(&added.handle, &tab).unwrap();
    drop(lease);
    assert!(!workspaces.tab_bindings(workspace.as_str())[0].pending);

    let _second = attach_returning_browser(&served);
    let bindings = workspaces.tab_bindings(workspace.as_str());
    assert_eq!(bindings.len(), 1);
    assert!(bindings[0].pending, "{bindings:?}");
}

/// A workbench over `workspaces` and a fresh authority, with no browser connected.
fn test_workbench(
    workspaces: &crate::workspace::WorkspaceStore,
) -> crate::workbench::WorkbenchFacade {
    crate::workbench::WorkbenchFacade::new(
        crate::workbench::WorkbenchProjection::default(),
        workspaces.clone(),
        crate::governance::GovernanceFacade::new(None),
        Arc::new(crate::browser::RelayBrowserPort::new(
            "events-test".into(),
            crate::service::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
}

/// Keeps the changes the workbench publishes, in order.
#[derive(Default)]
struct HeardChanges(std::sync::Mutex<Vec<crate::workbench::WorkbenchEvent>>);

impl crate::workbench::WorkbenchEventSink for HeardChanges {
    fn publish(&self, event: crate::workbench::WorkbenchEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl HeardChanges {
    fn all(&self) -> Vec<crate::workbench::WorkbenchEvent> {
        self.0.lock().unwrap().clone()
    }

    fn notes(&self) -> Vec<Option<String>> {
        self.all()
            .iter()
            .filter_map(|event| match &event.change {
                crate::workbench::WorkbenchChange::StatusChanged { status } => {
                    Some(status.runtime_note.clone())
                }
                _ => None,
            })
            .collect()
    }
}

/// Defect e74e3a23: a browser's request to show Ghostlight is carried out off the browser's
/// reader, one at a time, so a reveal that waits never holds the link the browser's next
/// event and its receipts arrive on, and a request with nothing to show frees the next.
#[test]
fn a_show_request_that_waits_does_not_hold_the_browser_link() {
    use crate::browser::adapter::{BrowserEvent, RuntimeControlIntent};
    use crate::browser::BrowserEventSink;
    use crate::language::workbench::PAUSED_FROM_BROWSER;

    let workspaces = crate::workspace::WorkspaceStore::default();
    let workbench = test_workbench(&workspaces);
    let heard = Arc::new(HeardChanges::default());
    workbench.attach_events(heard.clone());
    let sink = crate::service::ServiceBrowserEvents::new(workbench.clone(), workspaces);

    // Nothing is attached yet, and that is not an error: the request ends, so the next one is
    // carried out.
    sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
    eventually(|| !sink.showing.load(Ordering::SeqCst));

    let held = Arc::new(HeldReveal::default());
    workbench.attach_presentation(held.clone());
    // The reveal waits, and the sink still returns and applies the next event.
    sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
    eventually(|| held.started.load(Ordering::SeqCst) == 1);
    sink.on_event(FAKE_BROWSER, BrowserEvent::ShowRequested);
    sink.on_event(
        FAKE_BROWSER,
        BrowserEvent::RuntimeControlRequested {
            intent: RuntimeControlIntent::Hold,
        },
    );
    assert_eq!(heard.notes().pop(), Some(Some(PAUSED_FROM_BROWSER.into())));

    // The press that arrived while one was waiting was absorbed by it.
    held.release();
    eventually(|| !sink.showing.load(Ordering::SeqCst));
    assert_eq!(held.started.load(Ordering::SeqCst), 1);
}

/// A presentation port whose reveal waits until it is released, as one does while a shell
/// starts.
#[derive(Default)]
struct HeldReveal {
    started: AtomicUsize,
    released: std::sync::Mutex<bool>,
    changed: std::sync::Condvar,
}

impl HeldReveal {
    /// Let every reveal finish.
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.changed.notify_all();
    }
}

impl WorkbenchPresentationPort for HeldReveal {
    fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        let released = self.released.lock().unwrap();
        let _waited = self
            .changed
            .wait_timeout_while(released, PATIENCE, |released| !*released)
            .unwrap();
        Ok(())
    }

    fn notify(
        &self,
        _notification: WorkbenchNotification,
    ) -> Result<(), WorkbenchPresentationError> {
        Ok(())
    }
}

/// ACCEPTANCE B22 and defect a241b633: a browser that connects is remembered for the next
/// service of the home by the installed browser its name names, one that reports no name is
/// Chromium by that name, and a record that cannot be read is no memory.
#[test]
fn a_connected_browser_is_remembered_for_the_next_service_by_its_name() {
    use crate::browser::adapter::chromium::protocol4::fake::{
        FakeExtension, Reply as FakeReply, Script,
    };
    use crate::browser::BrowserPort as _;
    use crate::install::native_host::browser_id_named;

    let listing = || -> Script {
        Box::new(|correlation, _| {
            FakeReply::Now(FakeExtension::ok(correlation, json!({"tabs": []})))
        })
    };
    let home = temp_home("last-browser");
    let record = Layout::new(home.path()).last_browser_record();

    // A browser that reports its name is remembered as the installed browser it names, and
    // the next service of the home reads it.
    let hello = FakeExtension::hello("browser_last");
    let named = browser_id_named(hello["browser_name"].as_str().unwrap());
    assert!(named.is_some(), "{hello}");
    {
        let host = ServiceHost::start_for_test(home.path()).unwrap();
        assert_eq!(host.browser.last_connected_browser(), None);
        let (service, extension) = pair().unwrap();
        let fake = FakeExtension::connect(extension, &hello, listing());
        host.browser.attach(service).unwrap();
        assert_eq!(host.browser.last_connected_browser().as_deref(), named);
        fake.close();
    }
    let fresh = ServiceHost::start_for_test(home.path()).unwrap();
    assert_eq!(
        fresh.browser.last_connected_browser().as_deref(),
        named,
        "the next service of the home remembers it"
    );

    // Plain Chromium's brands are all generic, so its extension reports no name: it is
    // Chromium, the family every adapter belongs to, remembered and read by that name.
    let family = crate::browser::UNBRANDED_BROWSER_NAME.to_owned();
    let mut hello = FakeExtension::hello("browser_plain");
    hello.as_object_mut().unwrap().remove("browser_name");
    let (service, extension) = pair().unwrap();
    let fake = FakeExtension::connect(extension, &hello, listing());
    fresh.browser.attach(service).unwrap();
    assert_eq!(
        fresh.browser.last_connected_browser().as_deref(),
        browser_id_named(&family)
    );
    assert!(
        fresh
            .browser
            .browsers()
            .iter()
            .any(|browser| browser.id == "browser_plain" && browser.name == family),
        "the model reads a name, not null"
    );
    fake.close();

    // A record that cannot be read is no memory.
    std::fs::write(&record, b"{").unwrap();
    assert_eq!(fresh.browser.last_connected_browser(), None);
}
