//! Keyboard input and the other content a request puts into a control: typed text, key presses
//! and files.
//!
//! Text and files go to a located control in one call to its own document. Text typed into
//! whatever holds the focus first finds the document that holds it, checks the control still
//! takes text, and sends the text as input with the same check as its precondition, so the page
//! cannot move the focus between the check and the first keystroke. A key press is input behind
//! a precondition too: the located control takes the focus first, or the document found holding
//! the focus must still hold it. A refused precondition sends no key. The focus is bound here,
//! when the input is sent, through the scope: a focus in a document the scope leaves out is
//! refused before any key.

use serde_json::Value;

use super::super::keys;
use super::super::link::{CallRequest, Link};
use super::super::locator::Locator;
use super::super::scope::Scope;
use super::super::wire::InputStep;
use super::super::world::{self, Focused, Typed, Uploaded};
use super::read::{all, focus_documents, focus_holder, one, plan_each};
use super::{input_for, send_input, settled, value, OnDialog};
use crate::browser::adapter::{
    BrowserOutcome, PhysicalFile, PhysicalPressKey, PhysicalTypeFocused, PhysicalTypeText,
};
use crate::browser::BrowserError;
use crate::language::document_refusal::DocumentCause;

/// Type text into a located control.
///
/// # Errors
///
/// Returns the plain failure of a control that refuses the text before it is touched, and an
/// unknown effect for any failure after.
pub fn type_text(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: &PhysicalTypeText,
) -> Result<BrowserOutcome, BrowserError> {
    let target = Locator::decode(&request.locator)?;

    let plan = scope.call(
        &target.doc,
        world::type_text(
            &target.local,
            &request.text,
            request.clear_first,
            request.allow_credentials,
        ),
    )?;
    let answers = one(l, scope, request.tab_id, plan)?;
    let typed: Typed = settled(l, &answers[0])?;
    Ok(BrowserOutcome::Typed {
        tab: l.tab(request.tab_id)?.into(),
        character_count: request.text.chars().count(),
        subject: typed.subject,
        committed_urls: l.committed(),
    })
}

/// Type text into the control that holds the focus.
///
/// The document that holds the focus is asked whether its focused control takes text; when no
/// document reports the focus, every readable document is asked and the first that says so in
/// tree order is the one. Clearing happens there alone, and is checked again before
/// the text is sent. A dialog the text opens leaves its effect unknown.
///
/// # Errors
///
/// Returns the first candidate's failure when none holds a control that takes text, and an
/// unknown effect for any failure after the control was cleared or typed into.
pub fn type_focused(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: &PhysicalTypeFocused,
) -> Result<BrowserOutcome, BrowserError> {
    let candidates = focus_documents(l, scope, request.tab_id)?;
    let plans = plan_each(scope, &candidates, |_| {
        world::verify_focused_text(request.allow_credentials)
    })?;
    let answers = all(l, scope, request.tab_id, &plans, true)?;
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
        // The fact the fallback knows: the scope has no readable document to ask.
        return Err(refusal.unwrap_or(BrowserError::DocumentUnavailable(
            DocumentCause::NoWebDocument,
        )));
    };
    scope.admit_input_subject(&holder)?;
    if request.clear_first {
        let plans = [
            scope.call(&holder, world::clear_focused(request.allow_credentials))?,
            scope.call(
                &holder,
                world::verify_focused_text(request.allow_credentials),
            )?,
        ];
        let cleared = l.call(
            CallRequest::sequence(request.tab_id, &plans),
            Some(&mut *scope),
        )?;
        for answer in &cleared {
            settled::<Value>(l, answer)?;
        }
    }
    let precondition = scope.call(
        &holder,
        world::verify_focused_text(request.allow_credentials),
    )?;
    let input = input_for(
        l,
        request.tab_id,
        Some(precondition),
        vec![InputStep::InsertText {
            text: request.text.to_owned(),
        }],
        Some(scope.guard()),
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

/// Press one key.
///
/// With a locator the control takes the focus first. Without one, the document that holds the
/// focus is found now and must still hold it when the key is sent. When no document reports the
/// focus and the policy admits every document, the key goes to whatever holds the focus.
///
/// # Errors
///
/// Returns the plain failure of a refused precondition, with no key sent, and an unknown effect
/// for any failure after the first key event.
pub fn press_key(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: &PhysicalPressKey,
) -> Result<BrowserOutcome, BrowserError> {
    let (document, precondition) = match request.locator.as_deref() {
        Some(locator) => {
            let target = Locator::decode(locator)?;
            let plan = scope.call(&target.doc, world::focus(&target.local))?;
            (Some(target.doc), Some(plan))
        }
        None => match focus_holder(l, scope, request.tab_id)? {
            Some(holder) => {
                let plan = scope.call(&holder, world::require_focused())?;
                (Some(holder), Some(plan))
            }
            None => (None, None),
        },
    };
    if let Some(document) = &document {
        scope.admit_input_subject(document)?;
    }
    let input = input_for(
        l,
        request.tab_id,
        precondition,
        keys::press(&request.key, keys::modifier_mask(&request.modifiers)),
        Some(scope.guard()),
        0,
    );
    let answered = send_input(l, input, OnDialog::EffectUnknown)?;
    let subject = match (request.locator.as_deref(), answered) {
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
    scope: &mut Scope,
    tab_id: u64,
    locator: &str,
    files: &[PhysicalFile],
) -> Result<BrowserOutcome, BrowserError> {
    let target = Locator::decode(locator)?;

    let plan = scope.call(&target.doc, world::upload_files(&target.local, files))?;
    let answers = one(l, scope, tab_id, plan)?;
    let uploaded: Uploaded = settled(l, &answers[0])?;
    Ok(BrowserOutcome::FilesUploaded {
        tab_id,
        uploaded_count: uploaded.uploaded_count,
        uploaded_bytes: uploaded.uploaded_bytes,
        subject: uploaded.subject,
    })
}
