//! Folding per-document results into the one answer the model sees.
//!
//! A request reads every admitted document of a tab, and the model sees one page. This module is
//! the fold: found targets under one ceiling, visible text under one character bound, and
//! structure trees under one node bound. Every fold takes its input in depth-first document
//! order with the top document first, which is the order the frame tree yields, so the merged
//! answer never depends on which document answered first.
//!
//! All of it is pure. Nothing here talks to the browser or decides what a document may reveal.

use std::collections::HashMap;

use super::world::{Text, TreeNode, TreeValue};
use crate::browser::adapter::ObservedTarget;
use crate::language::area::{LeftOut, LeftOutKey};

/// What separates the text of two documents.
const SECTION_SEPARATOR: &str = "\n\n";

/// The targets one document found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentTargets {
    /// The targets, already bound to their document.
    pub targets: Vec<ObservedTarget>,
    /// Whether the document found more than it returned.
    pub truncated: bool,
    /// How many forms the document holds, whatever its targets kept.
    pub forms: u32,
    /// What the document left out of its own answer.
    pub left_out: LeftOut,
    /// Whether the document has an open modal dialog.
    pub modal_open: bool,
}

/// Targets folded under one ceiling.
#[derive(Clone, Debug, PartialEq)]
pub struct MergedTargets {
    /// The targets kept, in document order.
    pub targets: Vec<ObservedTarget>,
    /// Whether more targets existed than were kept.
    pub over: bool,
    /// What every document left out, plus what this fold cut.
    pub left_out: LeftOut,
    /// Whether any document has an open modal dialog.
    pub modal_open: bool,
}

/// Fold the targets of every document, top first, under one ceiling.
///
/// When the documents together hold more than the ceiling, the fold cuts by tier as a single
/// document does (ADR-0198 Decision 20): hidden targets first, then header, navigation and footer
/// targets, then asides, and page content last. Within a tier the earlier targets stay. The kept
/// targets keep document order, and every target the fold cuts is counted in `left_out`, added to
/// the counts the documents sent.
///
/// Each document numbers its forms from 1. The fold renumbers them page-wide so one number names
/// one form whatever each answer kept: a document's ordinals are offset by the number of forms
/// the documents before it hold.
///
/// Each document also decides alone which of its controls repeat. The fold decides again over
/// the kept targets: a role and name that no longer repeats loses `context` and `nth`, and `nth`
/// is counted afresh, in the merged order, among the targets that share role, name and place.
#[must_use]
pub fn targets(
    per_document: impl IntoIterator<Item = DocumentTargets>,
    maximum: usize,
) -> MergedTargets {
    let mut found = Vec::new();
    let mut over = false;
    let mut modal_open = false;
    let mut left_out = LeftOut::default();
    let mut forms_before = 0;
    for document in per_document {
        for mut target in document.targets {
            target.form = target.form.map(|ordinal| ordinal + forms_before);
            found.push(target);
        }
        forms_before += document.forms;
        over = over || document.truncated;
        modal_open = modal_open || document.modal_open;
        left_out.merge(&document.left_out);
    }
    if found.len() > maximum {
        over = true;
        found = cut_by_tier(found, maximum, &mut left_out);
    }
    renumber_repeats(&mut found);
    MergedTargets {
        targets: found,
        over,
        left_out,
        modal_open,
    }
}

/// Make `context` and `nth` true of the targets the model sees.
///
/// Targets that share a role and name repeat; a credential-class target never takes part and
/// never carries either key. A target whose role and name are unique keeps neither key. Among
/// a repeat, targets that share a place (their `context`, else their `group`, else none) are
/// numbered from 1 in the given order, and a target alone in its place carries no `nth`.
fn renumber_repeats(found: &mut [ObservedTarget]) {
    let mut repeats: HashMap<(&str, &str), Vec<usize>> = HashMap::new();
    for (index, target) in found.iter().enumerate() {
        if !target.credential_class {
            repeats
                .entry((target.role.as_str(), target.name.as_str()))
                .or_default()
                .push(index);
        }
    }
    let mut nths: Vec<Option<u32>> = vec![None; found.len()];
    let mut unique = Vec::new();
    for members in repeats.into_values() {
        if let [only] = members[..] {
            unique.push(only);
            continue;
        }
        let mut places: HashMap<&str, Vec<usize>> = HashMap::new();
        for index in members {
            let target = &found[index];
            let place = target
                .context
                .as_deref()
                .or(target.group.as_deref())
                .unwrap_or_default();
            places.entry(place).or_default().push(index);
        }
        for same in places.into_values() {
            if same.len() >= 2 {
                for (position, index) in same.into_iter().enumerate() {
                    nths[index] = u32::try_from(position + 1).ok();
                }
            }
        }
    }
    for index in unique {
        found[index].context = None;
    }
    for (target, nth) in found.iter_mut().zip(nths) {
        target.nth = nth;
        if target.credential_class {
            target.context = None;
        }
    }
}

/// Keep `maximum` targets by tier, count the rest in `left_out`, and return the kept targets in
/// their given order.
fn cut_by_tier(
    found: Vec<ObservedTarget>,
    maximum: usize,
    left_out: &mut LeftOut,
) -> Vec<ObservedTarget> {
    let tiers: Vec<LeftOutKey> = found
        .iter()
        .map(|target| LeftOutKey::tier_of(&target.state, target.area.as_deref()))
        .collect();
    let mut keep = vec![false; found.len()];
    let mut room = maximum;
    for tier in LeftOutKey::KEPT_FIRST {
        for (index, own) in tiers.iter().enumerate() {
            if *own != tier {
                continue;
            }
            if room > 0 {
                keep[index] = true;
                room -= 1;
            } else {
                left_out.add(tier, 1);
            }
        }
    }
    found
        .into_iter()
        .zip(keep)
        .filter_map(|(target, kept)| kept.then_some(target))
        .collect()
}

/// Visible text folded under one character bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MergedText {
    /// The text of every document that fit, separated by a blank line.
    pub text: String,
    /// Whether any text was cut, by a document's own bound or by this one.
    pub truncated: bool,
}

/// Fold the text of every document under one character bound.
///
/// A document that gave no section contributes nothing. `skipped` says the caller stopped
/// reading documents because the bound was already full, so the result is truncated even though
/// the sections it holds are whole. The bound counts characters.
#[must_use]
pub fn text_sections<'a>(
    sections: impl IntoIterator<Item = Option<&'a Text>>,
    maximum: usize,
    skipped: bool,
) -> MergedText {
    let mut text = String::new();
    let mut used = 0_usize;
    let mut truncated = skipped;
    for section in sections.into_iter().flatten() {
        truncated = truncated || section.truncated;
        let value = section.text.trim();
        if value.is_empty() {
            continue;
        }
        let separator = if text.is_empty() {
            ""
        } else {
            SECTION_SEPARATOR
        };
        let remaining = maximum.saturating_sub(used);
        if separator.chars().count() >= remaining {
            truncated = true;
            break;
        }
        let piece_chars = separator.chars().count() + value.chars().count();
        if piece_chars > remaining {
            text.extend(separator.chars().chain(value.chars()).take(remaining));
            truncated = true;
            break;
        }
        text.push_str(separator);
        text.push_str(value);
        used += piece_chars;
    }
    MergedText { text, truncated }
}

/// How many characters the next document may still add, or zero when the bound is already full
/// or a section was cut.
///
/// Reading stops at zero. A document asked for a positive amount can always answer, because the
/// answer is at least one character.
#[must_use]
pub fn remaining_after<'a>(
    sections: impl IntoIterator<Item = Option<&'a Text>>,
    maximum: usize,
) -> usize {
    let merged = text_sections(sections, maximum, false);
    if merged.truncated {
        return 0;
    }
    let used = merged.text.chars().count();
    let separator = if merged.text.is_empty() {
        0
    } else {
        SECTION_SEPARATOR.chars().count()
    };
    maximum.saturating_sub(used + separator).max(1)
}

/// What to ask of the embedded documents after the top document's tree is known.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubtreePlan {
    /// The deepest tree each embedded document may build.
    pub max_depth: usize,
    /// The most nodes each embedded document may spend.
    pub max_nodes: usize,
}

/// Decide whether the embedded documents are read, and with what budget.
///
/// Returns `None` when the top tree already spent the node bound, was cut, or leaves no depth
/// for a child. Embedded documents always sit one level below the top, so they get one level
/// less than `max_depth` even when the top tree could not be read: the first embedded tree then
/// stands in as the root and the rest nest under it, which keeps the whole answer within
/// `max_depth`.
#[must_use]
pub fn subtree_plan(
    root: Option<&TreeValue>,
    limit: usize,
    max_depth: usize,
) -> Option<SubtreePlan> {
    if max_depth <= 1 {
        return None;
    }
    let used = match root {
        None => 0,
        Some(root) if root.truncated => return None,
        Some(root) => count_nodes(&root.tree),
    };
    (used < limit).then(|| SubtreePlan {
        max_depth: max_depth - 1,
        max_nodes: limit - used,
    })
}

/// A structure tree folded across documents.
#[derive(Clone, Debug, PartialEq)]
pub struct MergedTree {
    /// The top document's tree with each embedded document's tree as one more child, or `None`
    /// when no document built one.
    pub tree: Option<TreeNode>,
    /// Whether any tree was cut, skipped or trimmed.
    pub truncated: bool,
}

/// Append the embedded documents' trees to the top tree, then trim the result to `limit` nodes
/// in pre-order.
///
/// The subtrees arrive in document order. Appending stops once the bound is spent or a tree was
/// cut, because everything after it would only be a partial view. `unread` says embedded
/// documents were left unasked (see [`subtree_plan`]), which makes the result truncated. Because
/// the embedded documents were asked at the same time, each with the whole remaining budget,
/// their sum can exceed the bound; the trim cuts the overflow from the end of the tree, so what
/// remains is what a sequential read would have kept.
#[must_use]
pub fn append_and_trim(
    root: Option<TreeValue>,
    subtrees: Vec<TreeValue>,
    unread: bool,
    limit: usize,
) -> MergedTree {
    let mut tree: Option<TreeNode> = None;
    let mut nodes = 0_usize;
    let mut truncated = unread;
    let mut pending = std::iter::once(root)
        .chain(subtrees.into_iter().map(Some))
        .peekable();
    while let Some(result) = pending.next() {
        if nodes >= limit {
            truncated = true;
            break;
        }
        let Some(result) = result else { continue };
        nodes += count_nodes(&result.tree);
        truncated = truncated || result.truncated;
        match tree.as_mut() {
            None => tree = Some(result.tree),
            Some(top) => top.children.push(result.tree),
        }
        if truncated && pending.peek().is_some() {
            break;
        }
    }
    if let Some(top) = tree.as_mut() {
        if nodes > limit {
            let mut budget = limit.saturating_sub(1);
            trim(top, &mut budget);
            truncated = true;
        }
    }
    MergedTree { tree, truncated }
}

/// How many nodes a tree holds, itself included.
fn count_nodes(node: &TreeNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

/// Keep the descendants of `node` that fit in `budget`, in pre-order, and drop the rest.
fn trim(node: &mut TreeNode, budget: &mut usize) {
    let mut kept = 0;
    for child in &mut node.children {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        trim(child, budget);
        kept += 1;
    }
    node.children.truncate(kept);
}

#[cfg(test)]
mod tests;
