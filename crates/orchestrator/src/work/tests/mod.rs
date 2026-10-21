//! The executor's test fixtures, and its tests by concern.

mod audit_health;
mod availability;
mod catalog_authority;
mod channels;
mod configured_authority;
mod control;
mod controls;
mod credentials;
mod documents;
mod execution;
mod findings;
mod golden;
mod presentation;
mod protocol4;
mod provenance;
mod rebound;
mod replacement;
mod restarts;
mod tabs;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::browser::adapter::{
    BrowserCommand, BrowserOutcome, BrowserReadiness, CaptureScope, EncodedRecording,
    ObservedTarget, PhysicalActionSubject, PhysicalRecordingSummary, PhysicalTab,
    RecordingDelivery, RecordingDestination, RecordingState, RecordingStopReason,
    RuntimeControlIntent, RuntimeControlState, ViewportGeometry, RECORDING_LOCAL_MAX_BYTES,
    RECORDING_TRANSFER_MAX_BYTES,
};
use crate::intake::session::ServiceContent;
use serde_json::json;

use crate::browser::recovery::RecoveryCandidate;
use crate::browser::testing::{summary, FakeBrowser, FAKE_BROWSER};
use crate::intake::session::IntakeChannel;
use crate::scratch::Scratch;

use crate::audit::AuditRecord;
use crate::audit::AuditSink;
use crate::governance::GovernanceFacade;
use crate::install::browser_package::BrowserPackage;
use crate::install::native_host::NativeHostState;
use crate::language::outcome::{ExtensionAdvice, Observed};
use crate::presentation::{PresentationError, PresentationPort, PresentationReactor};
use crate::workbench::WorkbenchProjection;
use crate::workspace::{StaleReason, WorkspaceStore};

use super::{
    adapter_budget_ms, observed_from, readiness_name, routing_refusal, ApplicationExecutor,
    BrowserError, BrowserPort, CancellationToken, Effect, Readiness, Status,
};

#[derive(Default)]
struct MemoryAudit(Mutex<Vec<AuditRecord>>);
impl AuditSink for MemoryAudit {
    fn record(&self, record: &AuditRecord) -> io::Result<()> {
        self.0.lock().unwrap().push(record.clone());
        Ok(())
    }
}

struct NoPresentation;
impl PresentationPort for NoPresentation {
    fn present(
        &self,
        _workspace: &str,
        _signal: crate::browser::adapter::PresentationSignal,
    ) -> Result<(), PresentationError> {
        Ok(())
    }
}

fn tab(id: u64, url: &str) -> PhysicalTab {
    PhysicalTab {
        tab_id: id,
        title: "Example".into(),
        url: url.into(),
        active: true,
        readiness: BrowserReadiness::Complete,
    }
}

fn recording_summary(state: RecordingState, source_url: &str) -> PhysicalRecordingSummary {
    PhysicalRecordingSummary {
        tab_id: 7,
        state,
        frame_count: usize::from(state != RecordingState::Recording),
        bytes_held: usize::from(state != RecordingState::Recording),
        duration_ms: 500,
        hard_expires_unix_ms: (state == RecordingState::Recording).then_some(121_000),
        retention_expires_unix_ms: (state != RecordingState::Recording).then_some(301_000),
        stop_reason: (state != RecordingState::Recording).then_some(RecordingStopReason::Explicit),
        source_urls: vec![source_url.into()],
        source_urls_complete: true,
    }
}

fn fixture_with_governance(
    governance: GovernanceFacade,
) -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
    Arc<MemoryAudit>,
) {
    let browser = Arc::new(FakeBrowser::default());
    let workspaces = WorkspaceStore::default();
    let (executor, workspace, audit) = executor_on(governance, &workspaces, browser.clone());
    (executor, browser, workspaces, workspace, audit)
}

/// An executor that works in `workspaces` through `browser`, with one admitted workspace.
fn executor_on(
    governance: GovernanceFacade,
    workspaces: &WorkspaceStore,
    browser: Arc<dyn BrowserPort>,
) -> (
    ApplicationExecutor,
    crate::workspace::WorkspaceId,
    Arc<MemoryAudit>,
) {
    let workspace = workspaces.admit("test".into(), IntakeChannel::Mcp);
    let audit = Arc::new(MemoryAudit::default());
    let mut executor = ApplicationExecutor::new(
        governance,
        workspaces.clone(),
        browser,
        PresentationReactor::new(Arc::new(NoPresentation)),
        WorkbenchProjection::default(),
        Arc::new(crate::audit::AuditRecorder::new(
            audit.clone(),
            WorkbenchProjection::default(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    );
    executor
        .recovery
        .set_test_candidates(vec![RecoveryCandidate {
            id: "chromium".into(),
            name: "Chromium".into(),
            package: BrowserPackage::Native,
            package_detail: "Chromium native package".into(),
            registration: NativeHostState::Current,
            ordinary_executable: Some(PathBuf::from("chromium")),
            extension: true,
        }]);
    (executor, workspace, audit)
}

fn fixture() -> (
    ApplicationExecutor,
    Arc<FakeBrowser>,
    WorkspaceStore,
    crate::workspace::WorkspaceId,
    Arc<MemoryAudit>,
) {
    fixture_with_governance(GovernanceFacade::new(None))
}

/// Let `workspace` hold the recording the fake browser's recording replies describe, as a
/// start would have left it.
fn holding_recording(executor: &ApplicationExecutor, workspace: &crate::workspace::WorkspaceId) {
    executor.workspaces.hold_recording(
        workspace,
        crate::browser::testing::FAKE_BROWSER,
        "recording_1",
    );
}

fn temporary_policy(name: &str) -> Scratch {
    Scratch::file(&format!("work-{name}.json"))
}

/// A real, isolated policy source that tests may edit between invocations.
///
/// An edit through `set` or `publish` is published to every facade the source gave out, as
/// the refresher would after a hand edit.
struct TestPolicy(Scratch, Mutex<Vec<GovernanceFacade>>);

impl TestPolicy {
    fn new() -> Self {
        let path = temporary_policy("configured-authority");
        write_policy(&path, &json!(["read", "action", "write", "execute"]));
        Self(path, Mutex::default())
    }

    fn facade(&self) -> GovernanceFacade {
        let facade = GovernanceFacade::new(Some(self.0.to_path_buf()));
        self.1.lock().unwrap().push(facade.clone());
        facade
    }

    fn set(&self, capabilities: serde_json::Value) {
        write_policy(&self.0, &capabilities);
        self.refresh();
    }

    /// Replace the whole policy document with `document`.
    fn publish(&self, document: &serde_json::Value) {
        write_document(&self.0, document);
        self.refresh();
    }

    fn refresh(&self) {
        for facade in self.1.lock().unwrap().iter() {
            facade.refresh_tick();
        }
    }
}

/// Write the test authority granting `capabilities`.
fn write_policy(path: &Path, capabilities: &serde_json::Value) {
    write_document(
        path,
        &json!({
            "schema":4,"name":"test authority","version":"1",
            "grants":[{"id":"test","hosts":{"allow":["*"]},"allowed":capabilities}]
        }),
    );
}

/// Write a policy document, moving the modified time forward so the refresher sees the edit
/// however fast the test runs.
fn write_document(path: &Path, document: &serde_json::Value) {
    let before = fs::metadata(path).and_then(|metadata| metadata.modified());
    fs::write(path, serde_json::to_vec(document).unwrap()).unwrap();
    if let Ok(before) = before {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(before + Duration::from_secs(2))
            .unwrap();
    }
}

/// A policy that admits all work on example.com and nothing anywhere else, so a page that
/// lands on another host is refused after the fact and holds its tab.
fn example_com_only() -> TestPolicy {
    let policy = TestPolicy::new();
    fs::write(
        &policy.0,
        serde_json::to_vec(&json!({
            "schema":4,"name":"test authority","version":"1",
            "grants":[{"id":"example","hosts":{"allow":["example.com"]},
                "allowed":["read","action","write","execute"]}]
        }))
        .unwrap(),
    )
    .unwrap();
    policy
}

fn all_open_policy_with(config: &str) -> String {
    format!(
        r#"{{"schema":4,"name":"work test","version":"1","grants":[{{"id":"all","hosts":{{"allow":["*"]}},"allowed":["read","action","write","execute"]}}],"config":{config}}}"#
    )
}
