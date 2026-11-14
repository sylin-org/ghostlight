//! Reading: text, targets and structure trees, from the documents a request may reach.
//!
//! A read asks every admitted document at once where it can, and folds the answers in tree
//! order with the top document first, so the merged answer never depends on which document
//! answered first. A document that fails contributes nothing to a whole-page read; the scope
//! records it as unavailable. A whole-page read that no document answered is not an empty page:
//! it fails with the first document's own failure, because only this side knows why nothing
//! answered. Reads that name their documents, by locator or by focus, fail with the document's
//! own failure.

use super::super::link::{CallRequest, Link};
use super::super::locator::{rescope, Locator};
use super::super::merge::{self, DocumentTargets};
use super::super::scope::{PlannedCall, Scope};
use super::super::wire::{CallAnswer, FrameRef};
use super::super::world::{self, Article, FocusProbe, Message, Targets, Text, TreeValue};
use super::value;
use crate::browser::adapter::{BrowserOutcome, Listing, ObservedTarget, PhysicalSemantic};
use crate::browser::BrowserError;
use crate::language::document_refusal::DocumentCause;
use crate::language::{FindScope, InspectScope, ReadMode};
use serde::de::DeserializeOwned;

/// The most nodes one structure tree holds across every document. Every document is asked
/// for its share as `max_nodes`, and the merged tree is trimmed to it once.
pub const TREE_NODE_LIMIT: usize = 400;

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
    kind: InspectScope,
    max_items: usize,
) -> Result<BrowserOutcome, BrowserError> {
    targets(
        l,
        scope,
        tab_id,
        |ask| world::inspect(kind.as_str(), ask),
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
    kind: FindScope,
    max_results: usize,
) -> Result<BrowserOutcome, BrowserError> {
    targets(
        l,
        scope,
        tab_id,
        |ask| world::find(text, kind.as_str(), ask),
        max_results,
    )
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
    selector: &PhysicalSemantic,
) -> Result<BrowserOutcome, BrowserError> {
    targets(
        l,
        scope,
        selector.tab_id,
        |ask| {
            world::query_semantic(
                &selector.name,
                selector.role.as_deref(),
                selector.exact,
                selector.form_scope,
                ask,
            )
        },
        selector.max_results,
    )
}

/// Ask every readable document for targets with one message, and fold what they found.
///
/// Each document is asked for `maximum` targets, or for fewer when its worst-case answer would
/// not fit one relay frame, and the documents are asked in as many calls as keep every receipt
/// within one frame. A document asked for fewer than it holds says it was cut.
fn targets(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    message: impl Fn(usize) -> Message,
    maximum: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let docs = scope.content_docs();
    let ask = maximum.min(merge::TARGETS_PER_DOCUMENT);
    let plans = plan_each(scope, &docs, |_| message(ask))?;
    let answers = within_frames(
        l,
        scope,
        tab_id,
        &plans,
        true,
        ask * merge::TARGET_WORST_CHARS,
    )?;
    some_answered::<Targets>(&answers)?;
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
    read_mode: ReadMode,
    max_chars: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let docs = scope.content_docs();
    let top = scope.top();
    let top_readable = docs.first() == Some(&top);
    let mut top_answer: Option<Text> = None;
    if read_mode == ReadMode::Article && top_readable {
        let plan = scope.call(
            &top,
            world::read_text(None, Some(ReadMode::Article.as_str()), max_chars),
        )?;
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
    // The fact this site knows: the scope holds no readable document to ask.
    let Some((first, rest)) = docs.split_first() else {
        return Err(BrowserError::DocumentUnavailable(
            DocumentCause::NoWebDocument,
        ));
    };
    let plan = scope.call(
        first,
        world::read_text(None, Some(ReadMode::Visible.as_str()), max_chars),
    )?;
    let answers = one(l, scope, tab_id, plan)?;
    let (first_text, first_failure) = match value::<Text>(&answers[0]) {
        Ok(text) => (Some(text), None),
        Err(error) => (None, Some(error)),
    };
    if top_readable && first_text.is_some() {
        top_answer.clone_from(&first_text);
    }
    let mut sections = vec![first_text];
    // The embedded documents are asked for what the bound has left, as many together as keep
    // each receipt within one relay frame, and no more once the bound is full.
    let mut unread = rest;
    let mut remaining = merge::remaining_after(sections.iter().map(Option::as_ref), max_chars);
    while remaining > 0 && !unread.is_empty() {
        let (batch, later) =
            unread.split_at(merge::documents_per_call(remaining).min(unread.len()));
        let message = world::read_text(None, Some(ReadMode::Visible.as_str()), remaining);
        let plans = plan_each(scope, batch, |_| message.clone())?;
        let answers = all(l, scope, tab_id, &plans, false)?;
        sections.extend(answers.iter().map(|answer| value::<Text>(answer).ok()));
        unread = later;
        remaining = merge::remaining_after(sections.iter().map(Option::as_ref), max_chars);
    }
    if let (Some(failure), None, true) = (
        first_failure,
        &top_answer,
        sections.iter().all(Option::is_none),
    ) {
        return Err(failure);
    }
    let skipped = !unread.is_empty();
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
    let mut failure = None;
    let (root, children) = if docs.first() == Some(&top) {
        let plan = scope.call(&top, world::inspect_tree(None, max_depth, TREE_NODE_LIMIT))?;
        let answers = one(l, scope, tab_id, plan)?;
        let root = value::<TreeValue>(&answers[0]);
        failure = root.as_ref().err().cloned();
        (root.ok(), &docs[1..])
    } else {
        (None, docs.as_slice())
    };
    let plan = merge::subtree_plan(root.as_ref(), TREE_NODE_LIMIT, max_depth);
    let mut subtrees = Vec::new();
    if let (Some(plan), false) = (plan, children.is_empty()) {
        let message = world::inspect_tree(None, plan.max_depth, plan.max_nodes);
        let plans = plan_each(scope, children, |_| message.clone())?;
        let first = root.is_none();
        let answers = within_frames(
            l,
            scope,
            tab_id,
            &plans,
            first,
            plan.max_nodes * merge::TREE_NODE_WORST_CHARS,
        )?;
        if failure.is_none() {
            failure = some_answered::<TreeValue>(&answers).err();
        }
        subtrees.extend(answers.iter().filter_map(|answer| value(answer).ok()));
    }
    if let (Some(failure), None, true) = (failure, &root, subtrees.is_empty()) {
        return Err(failure);
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

/// Describe the focused control: ask the document that holds the focus, or every readable one
/// when none reports it, and take the first that answers in tree order.
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
    let docs = focus_documents(l, scope, tab_id)?;
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
    // The fact this site knows: the scope holds no readable document to ask.
    Err(failure.unwrap_or(BrowserError::DocumentUnavailable(
        DocumentCause::NoWebDocument,
    )))
}

/// The document that holds the page's focus, found when the work that needs it runs.
///
/// Every addressable document of the tree is asked at once, with the scope's guard, whether it
/// holds the focus. Asking reads no content, so a document the scope leaves out is asked too:
/// that is how a focus in such a document is refused instead of typed into. The first document
/// in tree order that says so is the holder, and it is admitted as a subject of the scope's
/// input. When no document reports the focus, a policy that admits every document has nothing
/// to refuse and the answer is `None`.
///
/// # Errors
///
/// Returns the failure of the `call` primitive, [`BrowserError::OutsideScope`] when the holder
/// is a document the scope does not allow, and [`BrowserError::DocumentUnavailable`] when no
/// document reports the focus and the policy leaves some document out. Nothing has been sent as
/// input in any of these cases.
pub(super) fn focus_holder(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
) -> Result<Option<FrameRef>, BrowserError> {
    let docs: Vec<FrameRef> = scope
        .tree()
        .real_documents()
        .filter(|document| document.supported)
        .map(|document| document.doc.clone())
        .collect();
    let plans = plan_each(scope, &docs, |_| world::focus_probe())?;
    let answers = all(l, scope, tab_id, &plans, true)?;
    let holder = docs
        .into_iter()
        .zip(&answers)
        .find(|(_, answer)| value::<FocusProbe>(answer).is_ok_and(|probe| probe.focused))
        .map(|(doc, _)| doc);
    match holder {
        Some(holder) => {
            scope.admit_input_subject(&holder)?;
            Ok(Some(holder))
        }
        None if scope.unrestricted() => Ok(None),
        // The fact this site knows: no addressable document reports the page's focus, and the
        // policy leaves documents out, so the focus may be in one it would refuse.
        None => Err(BrowserError::DocumentUnavailable(
            DocumentCause::SubjectUnreachable,
        )),
    }
}

/// The documents to ask about the focused control: the one that holds the focus, or every
/// readable document when none reports it.
///
/// # Errors
///
/// Returns what [`focus_holder`] returns.
pub(super) fn focus_documents(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
) -> Result<Vec<FrameRef>, BrowserError> {
    Ok(match focus_holder(l, scope, tab_id)? {
        Some(holder) => vec![holder],
        None => scope.content_docs(),
    })
}

/// Check that some document answered a whole-page read with a value of its kind.
///
/// # Errors
///
/// Returns the first document's own failure, in plan order, when none did.
pub(super) fn some_answered<T: DeserializeOwned>(
    answers: &[CallAnswer],
) -> Result<(), BrowserError> {
    let mut first = None;
    for answer in answers {
        match value::<T>(answer) {
            Ok(_) => return Ok(()),
            Err(error) => {
                first.get_or_insert(error);
            }
        }
    }
    first.map_or(Ok(()), Err)
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
    let plans = [plan];
    l.call(
        CallRequest::sequence(tab_id, &plans).guard(scope.guard()),
        Some(&mut *scope),
    )
}

/// Send planned calls in as few `call` primitives as keep each receipt within one relay frame,
/// when each answer carries up to `characters` characters, and return every answer in plan
/// order. Only the first call carries the scope's guard, when `guarded`.
fn within_frames(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    plans: &[PlannedCall],
    guarded: bool,
    characters: usize,
) -> Result<super::super::link::Answers, BrowserError> {
    // The fact this site knows: the scope holds no readable document to ask.
    if plans.is_empty() {
        return Err(BrowserError::DocumentUnavailable(
            DocumentCause::NoWebDocument,
        ));
    }
    let mut answers = Vec::with_capacity(plans.len());
    for (index, batch) in plans
        .chunks(merge::documents_per_call(characters))
        .enumerate()
    {
        answers.extend(all(l, scope, tab_id, batch, guarded && index == 0)?);
    }
    Ok(answers)
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
    // The fact this site knows: the scope holds no readable document to ask.
    if plans.is_empty() {
        return Err(BrowserError::DocumentUnavailable(
            DocumentCause::NoWebDocument,
        ));
    }
    let request = CallRequest::parallel(tab_id, plans);
    let request = if guarded {
        request.guard(scope.guard())
    } else {
        request
    };
    l.call(request, Some(&mut *scope))
}
