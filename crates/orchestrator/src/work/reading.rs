//! Reading, inspection, discovery, and screenshot execution.

use ghostlight_bridge::browser::{BrowserCommand, BrowserOutcome};
use ghostlight_bridge::service::ServiceContent;
use serde_json::{json, Value};

use crate::governance::Capability;
use crate::language::outcome::{CaptureKind, Outcome, Refusal, TargetNoun};
use crate::workspace::WorkspaceLease;

use super::{
    bounded, observed_host, readiness, word_count, ApplicationExecutor, Effect, InvocationContext,
    TakeScreenshot, Terminal,
};

impl ApplicationExecutor {
    pub(super) fn read_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        target: Option<&str>,
        mode: Option<&str>,
        max_chars: usize,
    ) -> Terminal {
        let (selected, locator, _) =
            match self.resolve_optional_target(lease, requested_tab, target) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let command = if let Some(document_mode) = mode.filter(|_| target.is_none()) {
            BrowserCommand::ReadDocument {
                tab_id: selected.physical_id,
                mode: document_mode.to_string(),
                max_chars,
            }
        } else {
            BrowserCommand::ReadText {
                tab_id: selected.physical_id,
                locator,
                max_chars,
            }
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Text {
                tab_id,
                text,
                truncated,
                title,
                url,
            }) if tab_id == selected.physical_id => {
                let landing = self.authorize(context, Capability::Read, Some(&url));
                if !landing.allowed {
                    let _ = lease.hold_tab(&selected.handle);
                    return self.blocked_at(context, landing, Some(tab_id), Effect::None, false, json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}), observed_host(&url));
                }
                let words = word_count(&text);
                self.succeeded(context, landing, Some(tab_id), Effect::None, readiness(selected.readiness), true, Outcome::TextRead { words, host: observed_host(&url) }, json!({"tab":selected.handle.as_str(),"url":url,"title":bounded(&title,500),"text":bounded(&text,max_chars),"truncated":truncated || text.chars().count() > max_chars,"document_generation":selected.generation}))
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn inspect_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        kind: &str,
        root: Option<&str>,
        max_depth: Option<usize>,
        max_items: usize,
    ) -> Terminal {
        if kind == "document" {
            return self.inspect_document(context, lease, requested_tab, root, max_depth.unwrap_or(6));
        }
        self.targets_operation(
            context,
            lease,
            requested_tab,
            Capability::Read,
            BrowserCommand::Inspect {
                tab_id: 0,
                kind: kind.into(),
                max_items,
            },
            if kind == "controls" {
                TargetNoun::Control
            } else {
                TargetNoun::Item
            },
        )
    }

    pub(super) fn inspect_document(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        root: Option<&str>,
        max_depth: usize,
    ) -> Terminal {
        let (selected, locator, _) =
            match self.resolve_optional_target(lease, requested_tab, root) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(
            context,
            BrowserCommand::InspectTree {
                tab_id: selected.physical_id,
                locator,
                max_depth,
            },
        ) {
            Ok(BrowserOutcome::DocumentTree {
                tab_id,
                tree,
                truncated,
            }) if tab_id == selected.physical_id => {
                let parsed: Value = serde_json::from_str(&tree).unwrap_or(Value::Null);
                let nodes = count_tree_nodes(&parsed);
                let prior = lease.previous_snapshot(&selected);
                let diff = prior.as_ref().map(|old| diff_trees(old, &parsed));
                let handle = match lease.register_snapshot(&selected, parsed) {
                    Ok(handle) => handle,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let mut facts = json!({
                    "tab":selected.handle.as_str(),
                    "snapshot":handle.as_str(),
                    "nodes":nodes,
                    "truncated":truncated,
                    "document_generation":selected.generation,
                });
                if let Some((added, removed, changed, paths)) = &diff {
                    facts["diff"] = json!({"added":added,"removed":removed,"changed":changed,"paths":paths});
                }
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    Outcome::DocumentInspected { nodes, truncated, compared: diff.is_some() },
                    facts,
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn find(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        text: &str,
        kind: &str,
        max_results: usize,
    ) -> Terminal {
        self.targets_operation(
            context,
            lease,
            requested_tab,
            Capability::Read,
            BrowserCommand::Find {
                tab_id: 0,
                text: text.into(),
                kind: kind.into(),
                max_results,
            },
            TargetNoun::Match,
        )
    }

    /// One governed target retrieval.
    ///
    /// The closed noun chooses both the product sentence and the structured fact key.
    #[allow(clippy::too_many_arguments)]
    fn targets_operation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        capability: Capability,
        command: BrowserCommand,
        noun: TargetNoun,
    ) -> Terminal {
        let selected = match lease.select_tab(requested_tab) {
            Ok(tab) => tab,
            Err(error) => return self.workspace_failure(context, error),
        };
        let decision = self.authorize(context, capability, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        let command = match command {
            BrowserCommand::Inspect {
                kind, max_items, ..
            } => BrowserCommand::Inspect {
                tab_id: selected.physical_id,
                kind,
                max_items,
            },
            BrowserCommand::Find {
                text,
                kind,
                max_results,
                ..
            } => BrowserCommand::Find {
                tab_id: selected.physical_id,
                text,
                kind,
                max_results,
            },
            _ => unreachable!("target operations are closed"),
        };
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Targets { tab_id, targets }) if tab_id == selected.physical_id => {
                let mapped = match lease.register_targets(&selected, &targets) {
                    Ok(mapped) => mapped,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let items: Vec<_> = mapped.into_iter().map(|(handle, target)| json!({"target":handle.as_str(),"role":bounded(&target.role,100),"name":bounded(&target.name,500),"state":target.state,"credential_class":target.credential_class})).collect();
                let outcome = Outcome::TargetsListed {
                    noun,
                    count: items.len(),
                    host: observed_host(&selected.url),
                };
                let fact_key = match noun {
                    TargetNoun::Match => "matches",
                    TargetNoun::Control | TargetNoun::Item => "items",
                };
                let mut facts = serde_json::Map::new();
                facts.insert("tab".into(), json!(selected.handle.as_str()));
                facts.insert("document_generation".into(), json!(selected.generation));
                facts.insert(fact_key.into(), json!(items));
                self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    outcome,
                    Value::Object(facts),
                )
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }

    pub(super) fn screenshot(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        value: &TakeScreenshot,
    ) -> Terminal {
        let (selected, command, scope) = if let Some(view_handle) = value.view.as_deref() {
            let resolved = if let Some(requested) = value.tab.as_deref() {
                let selected = match lease.select_tab(Some(requested)) {
                    Ok(selected) => selected,
                    Err(error) => return self.workspace_failure(context, error),
                };
                match lease.resolve_view_region(
                    view_handle,
                    Some(&selected),
                    value.x.expect("language validated x"),
                    value.y.expect("language validated y"),
                    value.width.expect("language validated width"),
                    value.height.expect("language validated height"),
                ) {
                    Ok((view, region)) => (selected, view, region),
                    Err(error) => return self.workspace_failure(context, error),
                }
            } else {
                let (view, region) = match lease.resolve_view_region(
                    view_handle,
                    None,
                    value.x.expect("language validated x"),
                    value.y.expect("language validated y"),
                    value.width.expect("language validated width"),
                    value.height.expect("language validated height"),
                ) {
                    Ok(resolved) => resolved,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let selected = match lease.select_tab(Some(view.tab.as_str())) {
                    Ok(selected) => selected,
                    Err(error) => return self.workspace_failure(context, error),
                };
                (selected, view, region)
            };
            let (selected, view, region) = resolved;
            let command = BrowserCommand::ScreenshotRegion {
                tab_id: selected.physical_id,
                region,
                expected_viewport: view.viewport,
            };
            (selected, command, CaptureKind::Region)
        } else {
            let (selected, locator, _) = match self.resolve_optional_target(
                lease,
                value.tab.as_deref(),
                value.target.as_deref(),
            ) {
                Ok(value) => value,
                Err(error) => return self.workspace_failure(context, error),
            };
            let scope = if value.full_page {
                CaptureKind::FullPage
            } else if value.target.is_some() {
                CaptureKind::Target
            } else {
                CaptureKind::Viewport
            };
            let command = BrowserCommand::Screenshot {
                tab_id: selected.physical_id,
                locator,
                full_page: value.full_page,
            };
            (selected, command, scope)
        };
        let decision = self.authorize(context, Capability::Read, Some(selected.url.as_str()));
        if !decision.allowed {
            return self.blocked(
                context,
                decision,
                Some(selected.physical_id),
                Effect::None,
                true,
                json!({"reason":decision.reason.as_str()}),
            );
        }
        match self.dispatch(context, command) {
            Ok(BrowserOutcome::Screenshot {
                tab_id,
                mime_type,
                data,
                width,
                height,
                viewport,
            }) if tab_id == selected.physical_id => {
                if data.len() > 7_000_000 {
                    return self.failed(
                        context,
                        decision,
                        Some(tab_id),
                        Refusal::CaptureTooLarge,
                        json!({"reason":"screenshot_too_large"}),
                    );
                }
                let view = match lease.register_view(&selected, viewport, width, height) {
                    Ok(view) => view,
                    Err(error) => return self.workspace_failure(context, error),
                };
                let outcome = Outcome::Captured {
                    scope,
                    width,
                    height,
                };
                let mut terminal = self.succeeded(context, decision, Some(tab_id), Effect::None, readiness(selected.readiness), true, outcome, json!({"tab":selected.handle.as_str(),"view":view.as_str(),"mime_type":mime_type,"width":width,"height":height}));
                terminal.result = terminal
                    .result
                    .with_content(ServiceContent::Image { mime_type, data });
                terminal
            }
            Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
            Err(error) => {
                self.browser_failure(context, decision, error, Some(selected.physical_id))
            }
        }
    }
}

fn count_tree_nodes(tree: &Value) -> usize {
    let mut count = 0;
    let mut stack = vec![tree];
    while let Some(node) = stack.pop() {
        count += 1;
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                stack.push(child);
            }
        }
    }
    count
}

const DIFF_PATH_LIMIT: usize = 50;
const DIFF_DEPTH_LIMIT: usize = 24;

fn diff_trees(old: &Value, new: &Value) -> (usize, usize, usize, Vec<String>) {
    let mut added = 0;
    let mut removed = 0;
    let mut changed = 0;
    let mut paths = Vec::new();
    let mut record = |paths: &mut Vec<String>, path: String| {
        if paths.len() < DIFF_PATH_LIMIT {
            paths.push(path);
        }
    };
    compare_nodes(old, new, String::new(), &mut added, &mut removed, &mut changed, &mut paths, &mut record, 0);
    (added, removed, changed, paths)
}

#[allow(clippy::too_many_arguments)]
fn compare_nodes(
    old: &Value,
    new: &Value,
    path: String,
    added: &mut usize,
    removed: &mut usize,
    changed: &mut usize,
    paths: &mut Vec<String>,
    record: &mut dyn FnMut(&mut Vec<String>, String),
    depth: usize,
) {
    if depth > DIFF_DEPTH_LIMIT {
        return;
    }
    let old_children = old.get("children").and_then(Value::as_array);
    let new_children = new.get("children").and_then(Value::as_array);
    match (old_children, new_children) {
        (Some(old_children), Some(new_children)) => {
            if old.get("kind") != new.get("kind") || old.get("label") != new.get("label") {
                *changed += 1;
                record(paths, path.clone());
            }
            let shared = old_children.len().min(new_children.len());
            for index in 0..shared {
                let child_path = format!("{path}/{index}");
                compare_nodes(&old_children[index], &new_children[index], child_path, added, removed, changed, paths, record, depth + 1);
            }
            for index in shared..old_children.len() {
                *removed += 1;
                record(paths, format!("{path}/-{index}"));
            }
            for index in shared..new_children.len() {
                *added += 1;
                record(paths, format!("{path}/+{index}"));
            }
        }
        (None, Some(new_children)) => {
            *added += new_children.len();
            record(paths, path.clone());
        }
        (Some(old_children), None) => {
            *removed += old_children.len();
            record(paths, path.clone());
        }
        (None, None) => {}
    }
}
