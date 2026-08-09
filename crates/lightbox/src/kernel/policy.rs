// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Injected local-policy boot and live-reload parity scenarios.

use std::ffi::OsString;
use std::time::Duration;

use anyhow::ensure;
use serde_json::{json, Value};

use ghostlight_core::browser::pattern;
use ghostlight_core::governance::config::reload::PolicySource;
use ghostlight_core::governance::manifest::source;
use ghostlight_core::governance::paths::GovernancePaths;
use ghostlight_core::governance::ports::{Capability, ClientInfo};
use ghostlight_core::hub::outbound::browser::Browser;
use ghostlight_core::hub::peer::PeerUser;
use ghostlight_core::hub::ServiceContext;
use ghostlight_core::tool::outcome::CallOutcome;
use ghostlight_core::work::{CancellationToken, WorkContext};
use ghostlight_transport::bridge::WorkspaceUse;
use ghostlight_transport::observability::DebugSink;
use ghostlight_transport::operation::{OpenTabArguments, Operation};
use ghostlight_transport::workspace_id::WorkspaceId;

use crate::scenarios::Scenario;
use crate::support::TempRoot;

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        ("kernel-org-policy-boot", org_policy_boot),
        ("kernel-org-policy-hot-reload", org_policy_hot_reload),
    ]
}

fn manifest(capabilities: &[&str]) -> Value {
    json!({
        "schema": 3,
        "name": "lightbox-local-policy",
        "version": "1",
        "grants": [{
            "id": "r",
            "hosts": {"allow": ["example.com"]},
            "allowed": capabilities,
        }],
    })
}

fn expected_operations(allowed: &[Capability]) -> Vec<String> {
    ghostlight_core::operation::registry::descriptors()
        .iter()
        .filter(|descriptor| {
            descriptor.requires.is_empty()
                || descriptor
                    .requires
                    .iter()
                    .all(|required| allowed.contains(required))
        })
        .map(|descriptor| descriptor.operation.as_str().to_owned())
        .collect()
}

fn build_context(paths: &GovernancePaths) -> anyhow::Result<ServiceContext> {
    let loaded = source::load_policy_at(&paths.org_policy, None, pattern::is_valid_pattern)?;
    ensure!(
        loaded.manifest.is_some(),
        "injected org policy did not load"
    );
    Ok(ServiceContext::from_startup(
        Browser::new(),
        DebugSink::disabled(),
        loaded,
        PolicySource::Local {
            paths: paths.clone(),
            user_source: None,
        },
        None,
    )?)
}

struct WorkDriver {
    context: ServiceContext,
    owner: PeerUser,
    workspace: WorkspaceId,
    client: Option<ClientInfo>,
}

impl WorkDriver {
    fn start(context: ServiceContext, client: Option<ClientInfo>) -> anyhow::Result<Self> {
        let owner = PeerUser("lightbox-local-policy".to_string());
        let workspace = context.workspaces.mint(&owner, true)?;
        Ok(Self {
            context,
            owner,
            workspace,
            client,
        })
    }

    fn generation(&self) -> u64 {
        *self.context.catalog_generation.borrow()
    }

    fn authority_epoch(&self) -> u64 {
        self.context.authority.current().epoch
    }

    fn operation_names(&self) -> Vec<String> {
        let authority = self.context.authority.current();
        ghostlight_core::operation::registry::project_availability(
            &authority.governance,
            None,
            self.generation(),
        )
        .operations
        .iter()
        .map(|operation| operation.operation.as_str().to_string())
        .collect()
    }

    async fn call(&self, canonical: Operation) -> CallOutcome {
        let descriptor = ghostlight_core::operation::registry::descriptor(canonical.kind());
        let workspace = match descriptor.workspace_use {
            WorkspaceUse::Independent => None,
            _ => Some(self.workspace.clone()),
        };
        let lease = workspace.as_ref().map(|workspace| {
            self.context
                .workspaces
                .lease(workspace, &self.owner)
                .expect("live Lightbox workspace leases")
        });
        let work = WorkContext::new(workspace, canonical, self.client.clone(), None);
        let cancellation = CancellationToken::new();
        let outcome = ghostlight_core::tool::pipeline::run_work(
            &self.context.browser,
            &self.context.store,
            &self.context.authority,
            &self.context.workspaces,
            &work,
            &cancellation,
        )
        .await;
        drop(lease);
        outcome
    }

    async fn poll_operations_until(
        &self,
        expected: &[String],
        after_epoch: Option<u64>,
    ) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            let actual = self.operation_names();
            let generation = self.generation();
            let epoch = self.authority_epoch();
            if actual == expected && after_epoch.is_none_or(|previous| epoch > previous) {
                return Ok(());
            }
            ensure!(
                tokio::time::Instant::now() < deadline,
                "available operations never matched {expected:?} after authority epoch {after_epoch:?}; last epoch: {epoch}; last catalog generation: {generation}; last projection: {actual:?}"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    fn finish(self) -> anyhow::Result<()> {
        self.context
            .workspaces
            .release(&self.workspace, &self.owner)?;
        Ok(())
    }
}

struct AuditDirGuard {
    previous: Option<OsString>,
}

impl AuditDirGuard {
    fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var_os("GHOSTLIGHT_AUDIT_DIR");
        std::env::set_var("GHOSTLIGHT_AUDIT_DIR", path);
        Self { previous }
    }
}

impl Drop for AuditDirGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var("GHOSTLIGHT_AUDIT_DIR", previous);
        } else {
            std::env::remove_var("GHOSTLIGHT_AUDIT_DIR");
        }
    }
}

fn org_policy_boot() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let tmp = TempRoot::new("org-policy-boot")?;
        let _audit = AuditDirGuard::set(tmp.path());
        let paths = GovernancePaths::under(tmp.path());
        std::fs::write(&paths.org_policy, serde_json::to_vec(&manifest(&["read"]))?)?;
        let context = build_context(&paths)?;
        let driver = WorkDriver::start(context, None)?;
        let operations = driver.operation_names();
        ensure!(
            operations == expected_operations(&[Capability::Read]),
            "{operations:?}"
        );
        driver.finish()?;
        Ok(())
    })
}

fn org_policy_hot_reload() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let tmp = TempRoot::new("org-policy-hot-reload")?;
        let _audit = AuditDirGuard::set(tmp.path());
        let paths = GovernancePaths::under(tmp.path());
        std::fs::write(&paths.org_policy, serde_json::to_vec(&manifest(&["read"]))?)?;
        let context = build_context(&paths)?;
        let driver = WorkDriver::start(
            context,
            Some(ClientInfo {
                name: "lightbox-hot-reload".to_string(),
                version: "1.2.3".to_string(),
            }),
        )?;
        ensure!(driver.operation_names() == expected_operations(&[Capability::Read]));
        let initial_generation = driver.generation();
        let initial_epoch = driver.authority_epoch();

        std::fs::write(
            &paths.org_policy,
            serde_json::to_vec(&manifest(&["read", "action", "write"]))?,
        )?;
        driver
            .poll_operations_until(
                &expected_operations(&[
                    Capability::Read,
                    Capability::Interact,
                    Capability::Write,
                ]),
                Some(initial_epoch),
            )
            .await?;
        let expanded_generation = driver.generation();
        let expanded_epoch = driver.authority_epoch();
        let _ = driver
            .call(Operation::BrowserOpenTab(OpenTabArguments::default()))
            .await;

        std::fs::remove_file(&paths.org_policy)?;
        let all_open = ghostlight_core::operation::registry::descriptors()
            .iter()
            .map(|descriptor| descriptor.operation.as_str().to_owned())
            .collect::<Vec<_>>();
        driver
            .poll_operations_until(&all_open, Some(expanded_epoch))
            .await?;
        let all_open_generation = driver.generation();
        ensure!(
            expanded_generation == initial_generation + 1,
            "first catalog generation did not advance exactly once: {initial_generation} -> {expanded_generation}"
        );
        ensure!(
            all_open_generation == expanded_generation,
            "catalog generation changed even though the available operation set did not: {expanded_generation} -> {all_open_generation}"
        );
        driver.finish()?;

        tokio::time::sleep(Duration::from_millis(100)).await;
        let audit_path = tmp.path().join("audit.jsonl");
        let audit: Vec<Value> = std::fs::read_to_string(&audit_path)?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?;
        let reloads: Vec<&Value> = audit
            .iter()
            .filter(|record| record["event"] == "manifest_reload")
            .collect();
        ensure!(reloads.len() == 2, "reload events: {audit:?}");
        ensure!(reloads[1]["manifest"].is_null(), "{reloads:?}");
        ensure!(audit.iter().any(|record| {
            record.get("event").is_none() && record["client"]["name"] == "lightbox-hot-reload"
        }));
        Ok(())
    })
}
