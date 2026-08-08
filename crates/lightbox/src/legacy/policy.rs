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
use ghostlight_core::governance::ports::ClientInfo;
use ghostlight_core::hub::outbound::browser::Browser;
use ghostlight_core::hub::peer::PeerUser;
use ghostlight_core::hub::ServiceContext;
use ghostlight_core::tool::outcome::CallOutcome;
use ghostlight_core::work::{CancellationToken, WorkContext};
use ghostlight_transport::bridge::WorkspaceUse;
use ghostlight_transport::observability::DebugSink;
use ghostlight_transport::workspace_id::WorkspaceId;

use crate::scenarios::Scenario;
use crate::support::TempRoot;

pub(super) fn registry() -> Vec<Scenario> {
    vec![
        ("legacy-org-policy-boot", org_policy_boot),
        ("legacy-org-policy-hot-reload", org_policy_hot_reload),
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

fn read_only_tools() -> Vec<String> {
    [
        "tabs_context_mcp",
        "tabs_create_mcp",
        "navigate",
        "computer",
        "find",
        "get_page_text",
        "read_console_messages",
        "read_network_requests",
        "read_page",
        "resize_window",
        "update_plan",
        "narrate",
        "wait_for",
        "script",
        "act_on",
        "dialog",
        "tab_control",
        "browser_batch",
        "gif_creator",
        "explain",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn expanded_tools() -> Vec<String> {
    [
        "tabs_context_mcp",
        "tabs_create_mcp",
        "navigate",
        "computer",
        "find",
        "form_input",
        "get_page_text",
        "read_console_messages",
        "read_network_requests",
        "read_page",
        "resize_window",
        "update_plan",
        "narrate",
        "wait_for",
        "script",
        "form_fill",
        "act_on",
        "dialog",
        "tab_control",
        "file_upload",
        "browser_batch",
        "upload_image",
        "gif_creator",
        "explain",
    ]
    .into_iter()
    .map(str::to_string)
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

    fn tool_names(&self) -> Vec<String> {
        let authority = self.context.authority.current();
        ghostlight_core::browser::advertise::advertised_tools(
            &ghostlight_core::tool::tools::advertised_tools_json(),
            authority.governance.grants(),
        )["tools"]
            .as_array()
            .expect("legacy test catalog has tools")
            .iter()
            .map(|tool| {
                tool["name"]
                    .as_str()
                    .expect("canonical tool declaration has a name")
                    .to_string()
            })
            .collect()
    }

    async fn call(&self, operation: &str, arguments: &Value) -> CallOutcome {
        let canonical =
            ghostlight_core::operation::registry::decode_legacy_call(operation, arguments)
                .expect("Lightbox legacy call decodes");
        let descriptor = ghostlight_core::operation::registry::descriptor(canonical.key())
            .expect("Lightbox call maps to an implemented operation");
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
        let work = WorkContext::new(workspace, canonical, None, self.client.clone(), None);
        let cancellation = CancellationToken::new();
        let outcome = ghostlight_core::tool::pipeline::run_work(
            &self.context.browser,
            &self.context.store,
            &self.context.authority,
            &self.context.workspaces,
            &work,
            &cancellation,
            work.arguments(),
        )
        .await;
        drop(lease);
        outcome
    }

    async fn poll_tools_until(
        &self,
        expected: &[String],
        after_generation: Option<u64>,
    ) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
        loop {
            let actual = self.tool_names();
            let generation = self.generation();
            if actual == expected && after_generation.is_none_or(|previous| generation > previous) {
                return Ok(());
            }
            ensure!(
                tokio::time::Instant::now() < deadline,
                "advertised tools never matched {expected:?} after {after_generation:?}; last generation: {generation}; last projection: {actual:?}"
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
        let tools = driver.tool_names();
        ensure!(tools == read_only_tools(), "{tools:?}");
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
        ensure!(driver.tool_names() == read_only_tools());
        let initial_generation = driver.generation();

        std::fs::write(
            &paths.org_policy,
            serde_json::to_vec(&manifest(&["read", "action", "write"]))?,
        )?;
        driver
            .poll_tools_until(&expanded_tools(), Some(initial_generation))
            .await?;
        let expanded_generation = driver.generation();
        let _ = driver.call("tabs_create_mcp", &json!({})).await;

        std::fs::remove_file(&paths.org_policy)?;
        let all_open: Vec<String> = ghostlight_core::browser::directory::advertised_tool_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        driver
            .poll_tools_until(&all_open, Some(expanded_generation))
            .await?;
        let all_open_generation = driver.generation();
        ensure!(
            expanded_generation == initial_generation + 1,
            "first catalog generation did not advance exactly once: {initial_generation} -> {expanded_generation}"
        );
        ensure!(
            all_open_generation == expanded_generation + 1,
            "second catalog generation did not advance exactly once: {expanded_generation} -> {all_open_generation}"
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
