//! Reading, inspection, discovery, and screenshot execution.

use crate::browser::adapter::{BrowserCommand, BrowserOutcome};
use crate::intake::session::ServiceContent;
use serde_json::{json, Value};

use crate::governance::Capability;
use crate::language::outcome::{CaptureKind, Outcome, Refusal, TargetNoun};
use crate::language::{InspectPage, ReadMode, DEFAULT_DOCUMENT_DEPTH};
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
            Capability::Read,
            |selected, locator, _, decision| {
                let command = if target.is_none() {
                    BrowserCommand::ReadDocument {
                        tab_id: selected.physical_id,
                        mode: mode.unwrap_or(ReadMode::Visible).as_str().to_string(),
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
                            self.hold(context, lease, selected);
                            return self.blocked_at(
                                context,
                                landing,
                                Some(tab_id),
                                Effect::None,
                                false,
                                json!({"tab":selected.handle.as_str(),"reason":landing.reason.as_str(),"held":true}),
                                Some(&url),
                            );
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
        let kind = request.scope.as_str();
        if kind == "document" {
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
            Capability::Read,
            BrowserCommand::Inspect {
                tab_id: 0,
                kind: kind.into(),
                max_items: request.max_items,
            },
            if kind == "controls" {
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
            Capability::Read,
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
                        let raw: Value = serde_json::from_str(&tree).unwrap_or(Value::Null);
                        let returned = bounded_tree(&raw, max_depth);
                        let truncated = truncated || returned.cut;
                        let scope = SnapshotScope {
                            root: root.map(str::to_owned),
                            max_depth,
                        };
                        let diff = lease
                            .previous_snapshot(selected, &scope)
                            .map(|old| diff_trees(&old, &returned.tree));
                        let handle = match lease.register_snapshot(
                            selected,
                            scope,
                            returned.tree.clone(),
                        ) {
                            Ok(handle) => handle,
                            Err(error) => return self.workspace_failure(context, error),
                        };
                        let mut facts = json!({
                            "tab":selected.handle.as_str(),
                            "snapshot":handle.as_str(),
                            "nodes":returned.nodes,
                            "truncated":truncated,
                            "document_generation":selected.generation,
                            "tree":returned.tree,
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
                                nodes: returned.nodes,
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
    fn targets_operation(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        requested_tab: Option<&str>,
        capability: Capability,
        command: BrowserCommand,
        noun: TargetNoun,
    ) -> Terminal {
        self.with_authorized_tab(
            context,
            lease,
            requested_tab,
            capability,
            |selected, decision| {
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
            },
        )
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
                visual_settle: value.visual_settle,
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
                visual_settle: value.visual_settle,
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

/// The most nodes one document tree returns.
///
/// The adapter keeps the same budget. The service keeps it again because the tree is
/// page-derived and goes to the model as it stands here.
const DOCUMENT_TREE_NODE_LIMIT: usize = 400;

/// The most characters of one node label in a returned document tree.
const DOCUMENT_TREE_LABEL_CHARS: usize = 120;

/// The most positions one diff lists.
const DIFF_PATH_LIMIT: usize = 50;

/// The position of the root node in diff paths. Each level below adds `/` and a child index.
const TREE_ROOT_PATH: &str = "/";

/// What one document-tree node is, in the closed words the returned tree uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TreeNodeKind {
    /// An interactive control such as a button, link, or field.
    Control,
    /// A heading element.
    Heading,
    /// Any other rendered element.
    Container,
}

impl TreeNodeKind {
    /// Narrow the adapter's kind to the closed set. Anything unrecognized is a container.
    fn from_adapter(kind: Option<&str>) -> Self {
        match kind {
            Some("control") => Self::Control,
            Some("heading") => Self::Heading,
            _ => Self::Container,
        }
    }

    /// The word the returned tree uses.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Heading => "heading",
            Self::Container => "container",
        }
    }
}

/// A document tree in the bounded shape returned to the caller and kept as a snapshot.
struct BoundedTree {
    /// Nodes of `{kind, label, children}` only, or null when the page gave no tree.
    tree: Value,
    /// How many nodes `tree` holds.
    nodes: usize,
    /// Whether the depth or node budget cut part of what the adapter sent.
    cut: bool,
}

/// Rebuild the adapter's tree in the returned shape.
///
/// Only closed node kinds, bounded labels, and children survive, within `max_depth` levels and
/// the node budget. Every node keeps a `children` array, so every position resolves.
fn bounded_tree(raw: &Value, max_depth: usize) -> BoundedTree {
    let mut remaining = DOCUMENT_TREE_NODE_LIMIT;
    let mut cut = false;
    let tree = if raw.is_object() {
        bounded_node(raw, 1, max_depth, &mut remaining, &mut cut)
    } else {
        Value::Null
    };
    BoundedTree {
        tree,
        nodes: DOCUMENT_TREE_NODE_LIMIT - remaining,
        cut,
    }
}

fn bounded_node(
    raw: &Value,
    level: usize,
    max_depth: usize,
    remaining: &mut usize,
    cut: &mut bool,
) -> Value {
    *remaining -= 1;
    let mut children = Vec::new();
    for child in tree_children(raw).iter().filter(|child| child.is_object()) {
        if level >= max_depth || *remaining == 0 {
            *cut = true;
            break;
        }
        children.push(bounded_node(child, level + 1, max_depth, remaining, cut));
    }
    let kind = TreeNodeKind::from_adapter(raw.get("kind").and_then(Value::as_str));
    let label = raw.get("label").and_then(Value::as_str).unwrap_or_default();
    json!({
        "kind":kind.as_str(),
        "label":bounded(label, DOCUMENT_TREE_LABEL_CHARS),
        "children":children,
    })
}

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

    use super::{bounded_tree, diff_trees, DOCUMENT_TREE_NODE_LIMIT};

    fn node(kind: &str, label: &str, children: Vec<serde_json::Value>) -> serde_json::Value {
        json!({"kind":kind,"label":label,"children":children})
    }

    #[test]
    fn returned_tree_keeps_only_closed_kinds_bounded_labels_and_the_requested_depth() {
        let raw = json!({
            "kind":"container",
            "label":"Page",
            "secret":"page-authored field",
            "children":[
                {"kind":"heading","label":"Title","children":[]},
                {"kind":"invented","label":"x".repeat(500),"children":[
                    {"kind":"control","label":"Too deep","children":[]}
                ]},
                "not a node"
            ]
        });
        let returned = bounded_tree(&raw, 2);
        assert_eq!(
            returned.tree,
            node(
                "container",
                "Page",
                vec![
                    node("heading", "Title", vec![]),
                    node("container", &"x".repeat(120), vec![]),
                ],
            )
        );
        assert_eq!(returned.nodes, 3);
        assert!(returned.cut, "a level past max_depth is reported as cut");
        let absent = bounded_tree(&serde_json::Value::Null, 6);
        assert!(absent.tree.is_null());
        assert_eq!(absent.nodes, 0);
    }

    #[test]
    fn returned_tree_stops_at_the_node_budget() {
        let wide = node(
            "container",
            "Page",
            (0..DOCUMENT_TREE_NODE_LIMIT + 10)
                .map(|index| node("control", &format!("Item {index}"), vec![]))
                .collect(),
        );
        let returned = bounded_tree(&wide, 6);
        assert_eq!(returned.nodes, DOCUMENT_TREE_NODE_LIMIT);
        assert!(returned.cut);
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
