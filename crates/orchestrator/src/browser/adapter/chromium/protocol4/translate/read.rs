//! Reading: text, targets and structure trees, from the documents a request may reach.
//!
//! A read asks every admitted document at once where it can, and folds the answers in tree
//! order with the top document first, so the merged answer never depends on which document
//! answered first. A document that fails contributes nothing to a whole-page read; the scope
//! records it as unavailable. Reads that name their documents, by locator or by focus, fail
//! with the document's own failure.

use super::super::link::Link;
use super::super::locator::{rescope, Locator};
use super::super::merge::{self, DocumentTargets};
use super::super::scope::{PlannedCall, Scope};
use super::super::wire::{CallMode, FrameRef};
use super::super::world::{self, Article, Message, Targets, Text, TreeValue};
use super::value;
use crate::browser::adapter::{BrowserOutcome, Listing, ObservedTarget};
use crate::browser::BrowserError;

/// The most nodes one structure tree holds across every document. Every document is asked
/// for its share as `max_nodes`, and the merged tree is trimmed to it once.
pub const TREE_NODE_LIMIT: usize = 400;

/// The extraction modes of a document read.
pub mod mode {
    /// The main article, read from the top document alone.
    pub const ARTICLE: &str = "article";
    /// The visible text of every document.
    pub const VISIBLE: &str = "visible";
}

/// Read bounded text from one target, or from the top document.
///
/// The URL and title describe the tab. A target in an embedded document reports the top
/// document's URL as the tree holds it and no title, so the landing check, the observed host
/// and the agent's `url` fact are the tab's, and an embedded host never enters the record.
///
/// # Errors
///
/// Returns the failure of the read, including the document's own.
pub fn read_text(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    locator: Option<&str>,
    max_chars: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let (doc, local) = scope.target(locator)?;
    let plan = scope.call(&doc, world::read_text(local.as_deref(), None, max_chars))?;
    let answers = one(l, scope, tab_id, plan)?;
    let text: Text = value(&answers[0])?;
    let tree = scope.tree();
    let (title, url) = if doc == *tree.top() {
        (text.title, text.url)
    } else {
        (String::new(), tree.top_url().to_owned())
    };
    Ok(BrowserOutcome::Text {
        tab_id,
        text: text.text,
        truncated: text.truncated,
        title,
        url,
    })
}

/// List controls or structure of every readable document under one ceiling.
///
/// # Errors
///
/// Returns the failure of the `call` primitive.
pub fn inspect(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    kind: &str,
    max_items: usize,
) -> Result<BrowserOutcome, BrowserError> {
    targets(
        l,
        scope,
        tab_id,
        &world::inspect(kind, max_items),
        max_items,
    )
}

/// Find targets by visible text in every readable document under one ceiling.
///
/// # Errors
///
/// Returns the failure of the `call` primitive.
pub fn find(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    text: &str,
    kind: &str,
    max_results: usize,
) -> Result<BrowserOutcome, BrowserError> {
    targets(
        l,
        scope,
        tab_id,
        &world::find(text, kind, max_results),
        max_results,
    )
}

/// One semantic selector as work names it.
#[derive(Clone, Copy, Debug)]
pub struct Semantic<'a> {
    /// The accessible-name text.
    pub name: &'a str,
    /// The role a target must have, when one is named.
    pub role: Option<&'a str>,
    /// Whether the whole accessible name must equal the text.
    pub exact: bool,
    /// Whether only controls associated with a form count.
    pub form_scope: bool,
    /// The most targets to return across every document.
    pub max_results: usize,
}

/// Find targets by accessible name and role in every readable document, up to the selector's
/// `max_results`.
///
/// # Errors
///
/// Returns the failure of the `call` primitive.
pub fn query_semantic(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    selector: &Semantic<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    targets(
        l,
        scope,
        tab_id,
        &world::query_semantic(
            selector.name,
            selector.role,
            selector.exact,
            selector.form_scope,
            selector.max_results,
        ),
        selector.max_results,
    )
}

/// Ask every readable document for targets with one message, and fold what they found.
fn targets(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    message: &Message,
    maximum: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let docs = scope.content_docs();
    let plans = plan_each(scope, &docs, |_| message.clone())?;
    let answers = all(l, scope, tab_id, &plans, true)?;
    let found = docs.iter().zip(&answers).filter_map(|(doc, answer)| {
        value::<Targets>(answer).ok().map(|found| DocumentTargets {
            targets: rescope(doc, found.targets),
            truncated: found.truncated,
            forms: found.forms,
            left_out: found.left_out,
            modal_open: found.modal_open,
        })
    });
    let merged = merge::targets(found, maximum);
    if merged.over {
        scope.limit();
    }
    Ok(BrowserOutcome::Targets {
        tab_id,
        targets: merged.targets,
        listing: Listing {
            left_out: merged.left_out,
            modal_open: merged.modal_open,
            truncated: merged.over,
        },
    })
}

/// Read one composed document: the top document's article when asked and found, otherwise the
/// visible text of every readable document under one character bound.
///
/// The top document is read first, alone, so the documents it embeds are asked only for what
/// the bound has left.
///
/// # Errors
///
/// Returns the failure of a `call` primitive.
pub fn read_document(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    read_mode: &str,
    max_chars: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let docs = scope.content_docs();
    let top = scope.top();
    let top_readable = docs.first() == Some(&top);
    let mut top_answer: Option<Text> = None;
    if read_mode == mode::ARTICLE && top_readable {
        let plan = scope.call(&top, world::read_text(None, Some(mode::ARTICLE), max_chars))?;
        let answers = one(l, scope, tab_id, plan)?;
        if let Ok(article) = value::<Article>(&answers[0]) {
            if article.article_found {
                return Ok(article.into_text(tab_id));
            }
            top_answer = Some(Text {
                text: String::new(),
                truncated: false,
                title: article.title,
                url: article.url,
            });
        }
    }
    let Some((first, rest)) = docs.split_first() else {
        return Err(BrowserError::DocumentUnavailable);
    };
    let plan = scope.call(
        first,
        world::read_text(None, Some(mode::VISIBLE), max_chars),
    )?;
    let answers = one(l, scope, tab_id, plan)?;
    let first_text = value::<Text>(&answers[0]).ok();
    if top_readable && first_text.is_some() {
        top_answer.clone_from(&first_text);
    }
    let mut sections = vec![first_text];
    let remaining = merge::remaining_after(sections.iter().map(Option::as_ref), max_chars);
    if remaining > 0 && !rest.is_empty() {
        let message = world::read_text(None, Some(mode::VISIBLE), remaining);
        let plans = plan_each(scope, rest, |_| message.clone())?;
        let answers = all(l, scope, tab_id, &plans, false)?;
        sections.extend(answers.iter().map(|answer| value::<Text>(answer).ok()));
    }
    let skipped = remaining == 0 && !rest.is_empty();
    let merged = merge::text_sections(sections.iter().map(Option::as_ref), max_chars, skipped);
    let (title, url) = top_answer.map_or_else(Default::default, |top| (top.title, top.url));
    Ok(BrowserOutcome::Text {
        tab_id,
        text: merged.text,
        truncated: merged.truncated,
        title,
        url,
    })
}

/// Read the structure tree below one target, or of the whole page: the top document's tree
/// with each embedded document's tree as one more child, under [`TREE_NODE_LIMIT`] nodes.
///
/// # Errors
///
/// Returns the failure of a `call` primitive, and a targeted document's own failure.
pub fn inspect_tree(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    locator: Option<&str>,
    max_depth: usize,
) -> Result<BrowserOutcome, BrowserError> {
    if let Some(locator) = locator {
        let target = Locator::decode(locator)?;
        let plan = scope.call(
            &target.doc,
            world::inspect_tree(Some(&target.local), max_depth, TREE_NODE_LIMIT),
        )?;
        let answers = one(l, scope, tab_id, plan)?;
        let tree: TreeValue = value(&answers[0])?;
        return Ok(BrowserOutcome::DocumentTree {
            tab_id,
            tree: Some(tree.tree),
            truncated: tree.truncated,
        });
    }
    let docs = scope.content_docs();
    let top = scope.top();
    let (root, children) = if docs.first() == Some(&top) {
        let plan = scope.call(&top, world::inspect_tree(None, max_depth, TREE_NODE_LIMIT))?;
        let answers = one(l, scope, tab_id, plan)?;
        (value::<TreeValue>(&answers[0]).ok(), &docs[1..])
    } else {
        (None, docs.as_slice())
    };
    let plan = merge::subtree_plan(root.as_ref(), TREE_NODE_LIMIT, max_depth);
    let mut subtrees = Vec::new();
    if let (Some(plan), false) = (plan, children.is_empty()) {
        let message = world::inspect_tree(None, plan.max_depth, plan.max_nodes);
        let plans = plan_each(scope, children, |_| message.clone())?;
        let first = root.is_none();
        let answers = all(l, scope, tab_id, &plans, first)?;
        subtrees.extend(answers.iter().filter_map(|answer| value(answer).ok()));
    }
    let unread = plan.is_none() && !children.is_empty();
    let merged = merge::append_and_trim(root, subtrees, unread, TREE_NODE_LIMIT);
    Ok(BrowserOutcome::DocumentTree {
        tab_id,
        tree: merged.tree,
        truncated: merged.truncated,
    })
}

/// Describe targets by locator, each in its own document, before a fill.
///
/// # Errors
///
/// Returns the plain failure of a malformed locator, and any document's own failure.
pub fn describe_targets(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    locators: &[String],
) -> Result<BrowserOutcome, BrowserError> {
    let mut groups: Vec<(FrameRef, Vec<String>)> = Vec::new();
    for locator in locators {
        let target = Locator::decode(locator)?;
        match groups.iter_mut().find(|(doc, _)| *doc == target.doc) {
            Some((_, locals)) => locals.push(target.local),
            None => groups.push((target.doc, vec![target.local])),
        }
    }
    if groups.is_empty() {
        return Ok(BrowserOutcome::TargetsDescribed {
            tab_id,
            targets: Vec::new(),
            rebound: false,
        });
    }
    let plans = groups
        .iter()
        .map(|(doc, locals)| scope.call(doc, world::describe(locals)))
        .collect::<Result<Vec<_>, _>>()?;
    let answers = all(l, scope, tab_id, &plans, true)?;
    let mut targets: Vec<ObservedTarget> = Vec::new();
    for ((doc, _), answer) in groups.iter().zip(&answers) {
        targets.extend(rescope(doc, value::<Targets>(answer)?.targets));
    }
    let rebound = answers.iter().any(world::answer_rebound);
    Ok(BrowserOutcome::TargetsDescribed {
        tab_id,
        targets,
        rebound,
    })
}

/// Describe the focused control: ask the request's subject documents, or every readable one,
/// and take the first that answers in tree order.
///
/// # Errors
///
/// Returns the failure of the `call` primitive, and the first document's failure when no
/// document holds a focused control.
pub fn describe_focused(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
) -> Result<BrowserOutcome, BrowserError> {
    let docs = focus_candidates(scope);
    let plans = plan_each(scope, &docs, |_| world::describe_focused())?;
    let answers = all(l, scope, tab_id, &plans, true)?;
    let mut failure = None;
    for (doc, answer) in docs.iter().zip(&answers) {
        match value::<Targets>(answer) {
            Ok(found) => {
                return Ok(BrowserOutcome::TargetsDescribed {
                    tab_id,
                    targets: rescope(doc, found.targets),
                    rebound: false,
                })
            }
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    Err(failure.unwrap_or(BrowserError::DocumentUnavailable))
}

/// The documents that may hold the page's focus: the request's subject documents that are
/// readable, or every readable document when the request bound none.
pub(super) fn focus_candidates(scope: &Scope) -> Vec<FrameRef> {
    let readable = scope.content_docs();
    let subjects: Vec<FrameRef> = readable
        .iter()
        .filter(|doc| scope.subjects().contains(doc))
        .cloned()
        .collect();
    if subjects.is_empty() {
        readable
    } else {
        subjects
    }
}

/// Plan one message for each document. Every call goes through the scope.
pub(super) fn plan_each(
    scope: &Scope,
    docs: &[FrameRef],
    message: impl Fn(&FrameRef) -> Message,
) -> Result<Vec<PlannedCall>, BrowserError> {
    docs.iter()
        .map(|doc| scope.call(doc, message(doc)))
        .collect()
}

/// Send one planned call with the scope's guard, as the first primitive of a command.
pub(super) fn one(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    plan: PlannedCall,
) -> Result<super::super::link::Answers, BrowserError> {
    let guard = Some(scope.guard());
    let budget = l.budget_ms();
    l.call(
        tab_id,
        &[plan],
        CallMode::Sequence,
        guard,
        0,
        budget,
        Some(&mut *scope),
    )
}

/// Send planned calls together, with the scope's guard when `guarded`. No calls is no
/// readable document.
pub(super) fn all(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    plans: &[PlannedCall],
    guarded: bool,
) -> Result<super::super::link::Answers, BrowserError> {
    if plans.is_empty() {
        return Err(BrowserError::DocumentUnavailable);
    }
    let guard = if guarded { Some(scope.guard()) } else { None };
    let budget = l.budget_ms();
    l.call(
        tab_id,
        plans,
        CallMode::Parallel,
        guard,
        0,
        budget,
        Some(&mut *scope),
    )
}
