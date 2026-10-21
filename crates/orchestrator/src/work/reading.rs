//! Reading, inspection, discovery, and screenshot execution.

use crate::browser::adapter::{BrowserCommand, BrowserOutcome, TreeNode};
use crate::intake::session::ServiceContent;
use serde_json::{json, Value};

use crate::governance::Capability;
use crate::language::outcome::{CaptureKind, Outcome, Refusal, TargetNoun};
use crate::language::{FindScope, InspectPage, InspectScope, ReadMode, DEFAULT_DOCUMENT_DEPTH};
use crate::workspace::{SnapshotScope, WorkspaceLease};

use super::item::control_item;
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
        mode: Option<ReadMode>,
        max_chars: usize,
    ) -> Terminal {
        self.with_authorized_optional_target(
            context,
            lease,
            requested_tab,
            target,
            |selected, locator, _, decision| {
                let command = if target.is_none() {
                    BrowserCommand::ReadDocument {
                        tab_id: selected.physical_id,
                        mode: mode.unwrap_or(ReadMode::Visible),
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
                        let landing = self.authorize_landing(context, Capability::Read, &url);
                        if !landing.allowed {
                            let hold = context.hold_of(landing, Capability::Read, &url);
                            return self.hold(context, lease, selected, hold, Effect::None);
                        }
                        let words = word_count(&text);
                        self.succeeded(
                            context,
                            landing,
                            Some(tab_id),
                            Effect::None,
                            readiness(selected.readiness),
                            true,
                            Outcome::TextRead {
                                words,
                                host: observed_host(&url),
                            },
                            json!({"tab":selected.handle.as_str(),"url":url,"title":bounded(&title,500),"text":bounded(&text,max_chars),"truncated":truncated || text.chars().count() > max_chars,"document_generation":selected.generation}),
                        )
                    }
                    Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
                    Err(error) => {
                        self.browser_failure(context, decision, error, Some(selected.physical_id))
                    }
                }
            },
        )
    }

    pub(super) fn inspect_page(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        request: &InspectPage,
    ) -> Terminal {
        let requested_tab = request.tab.as_deref();
        let kind = request.scope;
        if kind == InspectScope::Document {
            return self.inspect_document(
                context,
                lease,
                requested_tab,
                request.root.as_deref(),
                request.max_depth.unwrap_or(DEFAULT_DOCUMENT_DEPTH),
            );
        }
        self.targets_operation(
            context,
            lease,
            requested_tab,
            BrowserCommand::Inspect {
                tab_id: 0,
                kind,
                max_items: request.max_items,
            },
            if kind == InspectScope::Controls {
                TargetNoun::Control
            } else {
                TargetNoun::Item
            },
        )
    }

    fn inspect_document(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        root: Option<&str>,
        max_depth: usize,
    ) -> Terminal {
        self.with_authorized_optional_target(
            context,
            lease,
            requested_tab,
            root,
            |selected, locator, _, decision| {
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
                        let nodes = tree.as_ref().map_or(0, TreeNode::nodes);
                        let tree = json!(tree);
                        let scope = SnapshotScope {
                            root: root.map(str::to_owned),
                            max_depth,
                        };
                        let diff = lease
                            .previous_snapshot(selected, &scope)
                            .map(|old| diff_trees(&old, &tree));
                        let handle = match lease.register_snapshot(selected, scope, tree.clone())
                        {
                            Ok(handle) => handle,
                            Err(error) => return self.workspace_failure(context, error),
                        };
                        let mut facts = json!({
                            "tab":selected.handle.as_str(),
                            "snapshot":handle.as_str(),
                            "nodes":nodes,
                            "truncated":truncated,
                            "document_generation":selected.generation,
                            "tree":tree,
                        });
                        if let Some(diff) = &diff {
                            facts["diff"] = json!({"added":diff.added,"removed":diff.removed,"changed":diff.changed,"paths":diff.paths});
                        }
                        self.succeeded(
                            context,
                            decision,
                            Some(tab_id),
                            Effect::None,
                            readiness(selected.readiness),
                            true,
                            Outcome::DocumentInspected {
                                nodes,
                                truncated,
                                differences: diff.as_ref().map(TreeDiff::total),
                            },
                            facts,
                        )
                    }
                    Ok(_) => self.protocol_failure(context, decision, Some(selected.physical_id)),
                    Err(error) => {
                        self.browser_failure(context, decision, error, Some(selected.physical_id))
                    }
                }
            },
        )
    }

    pub(super) fn find(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        text: &str,
        kind: FindScope,
        max_results: usize,
    ) -> Terminal {
        self.targets_operation(
            context,
            lease,
            requested_tab,
            BrowserCommand::Find {
                tab_id: 0,
                text: text.into(),
                kind,
                max_results,
            },
            TargetNoun::Match,
        )
    }

    /// One governed target retrieval.
    ///
    /// The closed noun chooses both the product sentence and the structured fact key.
    fn targets_operation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        command: BrowserCommand,
        noun: TargetNoun,
    ) -> Terminal {
        self.with_authorized_tab(context, lease, requested_tab, |selected, decision| {
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
                Ok(BrowserOutcome::Targets {
                    tab_id,
                    targets,
                    listing,
                }) if tab_id == selected.physical_id => {
                    let mapped = match lease.register_targets(selected, &targets) {
                        Ok(mapped) => mapped,
                        Err(error) => return self.workspace_failure(context, error),
                    };
                    let items: Vec<_> = mapped
                        .into_iter()
                        .map(|(handle, target)| control_item(handle.as_str(), &target))
                        .collect();
                    let outcome = Outcome::TargetsListed {
                        noun,
                        count: items.len(),
                        host: observed_host(&selected.url),
                        left_out: listing.left_out.clone(),
                    };
                    let fact_key = match noun {
                        TargetNoun::Match => "matches",
                        TargetNoun::Control | TargetNoun::Item => "items",
                    };
                    let mut facts = serde_json::Map::new();
                    facts.insert("tab".into(), json!(selected.handle.as_str()));
                    facts.insert("document_generation".into(), json!(selected.generation));
                    facts.insert(fact_key.into(), json!(items));
                    if let Some(left_out) = listing.left_out.to_value() {
                        facts.insert("left_out".into(), left_out);
                    }
                    if listing.modal_open {
                        facts.insert("modal_open".into(), json!(true));
                    }
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
        })
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
                // The dispatch spends the invocation's preparation budget here.
                settle_ms: 0,
            };
            (selected, command, CaptureKind::Region)
        } else {
            let (selected, locator, _) = match self.resolve_optional_target(
                context,
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
                // The dispatch spends the invocation's preparation budget here.
                settle_ms: 0,
            };
            (selected, command, scope)
        };
        let decision = self.authorize(context, context.requirements, Some(selected.url.as_str()));
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
                // One volatile reuse asset beside the view; superseded by the
                // next capture and refused above the upload ceiling.
                let decoded_bytes = data.len() / 4 * 3;
                let image_handle = lease
                    .register_image(&selected, &mime_type, &data, decoded_bytes)
                    .ok()
                    .flatten();
                let outcome = Outcome::Captured {
                    scope,
                    width,
                    height,
                };
                let mut facts = json!({
                    "tab": selected.handle.as_str(),
                    "view": view.as_str(),
                    "mime_type": mime_type,
                    "width": width,
                    "height": height,
                    "visual_settle": value.visual_settle.unwrap_or(true),
                });
                if let Some(image) = &image_handle {
                    facts["image"] = json!(image.as_str());
                }
                let mut terminal = self.succeeded(
                    context,
                    decision,
                    Some(tab_id),
                    Effect::None,
                    readiness(selected.readiness),
                    true,
                    outcome,
                    facts,
                );
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

/// The most positions one diff lists.
const DIFF_PATH_LIMIT: usize = 50;

/// The position of the root node in diff paths. Each level below adds `/` and a child index.
const TREE_ROOT_PATH: &str = "/";

fn tree_children(node: &Value) -> &[Value] {
    node.get("children")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// How many nodes a returned subtree holds.
fn subtree_nodes(node: &Value) -> usize {
    let mut count = 0;
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        count += 1;
        stack.extend(tree_children(node));
    }
    count
}

/// The position of a parent's child in the returned tree.
fn child_path(parent: &str, index: usize) -> String {
    if parent == TREE_ROOT_PATH {
        format!("{TREE_ROOT_PATH}{index}")
    } else {
        format!("{parent}/{index}")
    }
}

/// Node-by-position differences between two snapshots of the same scope.
#[derive(Debug, Default, Eq, PartialEq)]
struct TreeDiff {
    /// Nodes present only in the returned tree.
    added: usize,
    /// Nodes present only in the previous snapshot.
    removed: usize,
    /// Nodes whose kind or label changed in place.
    changed: usize,
    /// Positions in the returned tree where something differs.
    paths: Vec<String>,
}

impl TreeDiff {
    fn total(&self) -> usize {
        self.added + self.removed + self.changed
    }

    fn mark(&mut self, path: &str) {
        if self.paths.len() < DIFF_PATH_LIMIT && !self.paths.iter().any(|known| known == path) {
            self.paths.push(path.to_owned());
        }
    }
}

/// Compare the previous snapshot with the returned tree, node by node by position.
///
/// Every path is a position in the returned tree: a new node, a node whose kind or label
/// changed, or a node that lost children.
fn diff_trees(old: &Value, new: &Value) -> TreeDiff {
    let mut diff = TreeDiff::default();
    match (old.is_object(), new.is_object()) {
        (true, true) => compare_nodes(old, new, TREE_ROOT_PATH, &mut diff),
        (false, true) => {
            diff.added += subtree_nodes(new);
            diff.mark(TREE_ROOT_PATH);
        }
        (true, false) => diff.removed += subtree_nodes(old),
        (false, false) => {}
    }
    diff
}

fn compare_nodes(old: &Value, new: &Value, path: &str, diff: &mut TreeDiff) {
    if old.get("kind") != new.get("kind") || old.get("label") != new.get("label") {
        diff.changed += 1;
        diff.mark(path);
    }
    let old_children = tree_children(old);
    let new_children = tree_children(new);
    let shared = old_children.len().min(new_children.len());
    for (index, (before, after)) in old_children.iter().zip(new_children).enumerate() {
        compare_nodes(before, after, &child_path(path, index), diff);
    }
    if old_children.len() > shared {
        diff.removed += old_children[shared..]
            .iter()
            .map(subtree_nodes)
            .sum::<usize>();
        diff.mark(path);
    }
    for (index, added) in new_children.iter().enumerate().skip(shared) {
        diff.added += subtree_nodes(added);
        diff.mark(&child_path(path, index));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::diff_trees;

    fn node(kind: &str, label: &str, children: Vec<serde_json::Value>) -> serde_json::Value {
        json!({"kind":kind,"label":label,"children":children})
    }

    #[test]
    fn diff_paths_are_positions_in_the_returned_tree() {
        let old = node(
            "container",
            "Page",
            vec![
                node("heading", "Title", vec![]),
                node(
                    "container",
                    "Panel",
                    vec![
                        node("control", "Save", vec![]),
                        node("control", "Delete", vec![]),
                    ],
                ),
            ],
        );
        let new = node(
            "container",
            "Page",
            vec![
                node("heading", "New title", vec![]),
                node("container", "Panel", vec![node("control", "Save", vec![])]),
                node("container", "Toast", vec![node("control", "Undo", vec![])]),
            ],
        );
        let diff = diff_trees(&old, &new);
        assert_eq!(diff.changed, 1);
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.added, 2);
        assert_eq!(diff.total(), 4);
        assert_eq!(diff.paths, vec!["/0", "/1", "/2"]);
        let unchanged = diff_trees(&new, &new);
        assert_eq!(unchanged.total(), 0);
        assert!(unchanged.paths.is_empty());
    }
}
