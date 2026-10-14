//! Keyboard input and the other content a request puts into a control: typed text, key presses
//! and files.
//!
//! Text and files go to a located control in one call to its own document. Text typed into
//! whatever holds the focus first finds the document that holds it, checks the control still
//! takes text, and sends the text as input with the same check as its precondition, so the page
//! cannot move the focus between the check and the first keystroke. A key press is input behind
//! a precondition too: the located control takes the focus first, or the document the request
//! bound as the focus must still hold it. A refused precondition sends no key.

use serde_json::Value;

use super::super::keys;
use super::super::link::Link;
use super::super::locator::Locator;
use super::super::scope::Scope;
use super::super::wire::{CallMode, InputStep};
use super::super::world::{self, Focused, Typed, Uploaded};
use super::read::{all, focus_candidates, one, plan_each};
use super::{input_for, reach, send_input, settled, value, OnDialog};
use crate::browser::adapter::{BrowserOutcome, PhysicalFile};
use crate::browser::BrowserError;

/// What typing into a located control asks for.
#[derive(Clone, Copy, Debug)]
pub struct TypeText<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The control's encoded locator.
    pub locator: &'a str,
    /// The text.
    pub text: &'a str,
    /// Whether to clear the control first.
    pub clear_first: bool,
    /// Whether the request may type into a credential-class control.
    pub allow_credentials: bool,
}

/// Type text into a located control.
///
/// # Errors
///
/// Returns the plain failure of a control that refuses the text before it is touched, and an
/// unknown effect for any failure after.
pub fn type_text(
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
    request: TypeText<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let target = Locator::decode(request.locator)?;
    let mut reach = reach(l, scope, request.tab_id)?;
    let plan = reach.get().call(
        &target.doc,
        world::type_text(
            &target.local,
            request.text,
            request.clear_first,
            request.allow_credentials,
        ),
    )?;
    let answers = one(l, &mut reach, request.tab_id, plan)?;
    let typed: Typed = settled(l, &answers[0])?;
    Ok(BrowserOutcome::Typed {
        tab: l.tab(request.tab_id)?.into(),
        character_count: request.text.chars().count(),
        subject: typed.subject,
        committed_urls: l.committed(),
    })
}

/// What typing into the focused control asks for.
#[derive(Clone, Copy, Debug)]
pub struct TypeFocused<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The text.
    pub text: &'a str,
    /// Whether to clear the control first.
    pub clear_first: bool,
    /// Whether the request may type into a credential-class control.
    pub allow_credentials: bool,
}

/// Type text into the control that holds the focus.
///
/// Every candidate document is asked whether its focused control takes text, and the first that
/// says so in tree order is the one. Clearing happens there alone, and is checked again before
/// the text is sent. A dialog the text opens leaves its effect unknown.
///
/// # Errors
///
/// Returns the first candidate's failure when none holds a control that takes text, and an
/// unknown effect for any failure after the control was cleared or typed into.
pub fn type_focused(
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
    request: TypeFocused<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let mut reach = reach(l, scope, request.tab_id)?;
    let candidates = focus_candidates(&reach);
    let plans = plan_each(&reach, &candidates, |_| {
        world::verify_focused_text(request.allow_credentials)
    })?;
    let answers = all(l, &mut reach, request.tab_id, &plans, true)?;
    let mut refusal = None;
    let mut holder = None;
    for (doc, answer) in candidates.iter().zip(&answers) {
        match value::<Value>(answer) {
            Ok(_) => {
                holder = Some(doc.clone());
                break;
            }
            Err(error) => {
                refusal.get_or_insert(error);
            }
        }
    }
    let Some(holder) = holder else {
        return Err(refusal.unwrap_or(BrowserError::DocumentUnavailable));
    };
    reach.admit_input(&holder)?;
    if request.clear_first {
        let plans = [
            reach
                .get()
                .call(&holder, world::clear_focused(request.allow_credentials))?,
            reach.get().call(
                &holder,
                world::verify_focused_text(request.allow_credentials),
            )?,
        ];
        let budget = l.budget_ms();
        let cleared = l.call(
            request.tab_id,
            &plans,
            CallMode::Sequence,
            None,
            0,
            budget,
            Some(reach.scope()),
        )?;
        for answer in &cleared {
            settled::<Value>(l, answer)?;
        }
    }
    let precondition = reach
        .get()
        .call(
            &holder,
            world::verify_focused_text(request.allow_credentials),
        )?
        .call;
    let input = input_for(
        l,
        request.tab_id,
        Some(precondition),
        vec![InputStep::InsertText {
            text: request.text.to_owned(),
        }],
        reach.guard(),
        0,
    );
    send_input(l, input, OnDialog::EffectUnknown)?;
    Ok(BrowserOutcome::Typed {
        tab: l.tab(request.tab_id)?.into(),
        character_count: request.text.chars().count(),
        subject: None,
        committed_urls: l.committed(),
    })
}

/// What a key press asks for.
#[derive(Clone, Copy, Debug)]
pub struct PressKey<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The control to focus first, when the press names one.
    pub locator: Option<&'a str>,
    /// The key's name.
    pub key: &'a str,
    /// The modifier names held during the press.
    pub modifiers: &'a [String],
}

/// Press one key.
///
/// With a locator the control takes the focus first. Without one, a scope that bound the focus
/// to a document requires that document to still hold it; a request whose scope bound none
/// presses into whatever holds the focus.
///
/// # Errors
///
/// Returns the plain failure of a refused precondition, with no key sent, and an unknown effect
/// for any failure after the first key event.
pub fn press_key(
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
    request: PressKey<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let mut reach = reach(l, scope, request.tab_id)?;
    let (document, precondition) = match request.locator {
        Some(locator) => {
            let target = Locator::decode(locator)?;
            let plan = reach.get().call(&target.doc, world::focus(&target.local))?;
            (Some(target.doc), Some(plan.call))
        }
        None => match reach.get().subjects().first() {
            Some(subject) => {
                let plan = reach.get().call(subject, world::require_focused())?;
                (Some(subject.clone()), Some(plan.call))
            }
            None => (None, None),
        },
    };
    if let Some(document) = &document {
        reach.admit_input(document)?;
    }
    let input = input_for(
        l,
        request.tab_id,
        precondition,
        keys::press(request.key, keys::modifier_mask(request.modifiers)),
        reach.guard(),
        0,
    );
    let answered = send_input(l, input, OnDialog::EffectUnknown)?;
    let subject = match (request.locator, answered) {
        (Some(_), Some(answer)) => settled::<Focused>(l, &answer)?.subject,
        _ => None,
    };
    Ok(BrowserOutcome::KeyPressed {
        tab: l.tab(request.tab_id)?.into(),
        key: request.key.to_owned(),
        subject,
        committed_urls: l.committed(),
    })
}

/// Attach files to a located file input.
///
/// # Errors
///
/// Returns the plain failure of a control that refuses the files before they are attached, and
/// an unknown effect for any failure after.
pub fn upload_files(
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
    tab_id: u64,
    locator: &str,
    files: &[PhysicalFile],
) -> Result<BrowserOutcome, BrowserError> {
    let target = Locator::decode(locator)?;
    let mut reach = reach(l, scope, tab_id)?;
    let plan = reach
        .get()
        .call(&target.doc, world::upload_files(&target.local, files))?;
    let answers = one(l, &mut reach, tab_id, plan)?;
    let uploaded: Uploaded = settled(l, &answers[0])?;
    Ok(BrowserOutcome::FilesUploaded {
        tab_id,
        uploaded_count: uploaded.uploaded_count,
        uploaded_bytes: uploaded.uploaded_bytes,
        subject: uploaded.subject,
    })
}
