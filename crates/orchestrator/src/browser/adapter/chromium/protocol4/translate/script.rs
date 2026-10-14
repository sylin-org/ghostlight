//! Script evaluation in the page's main world.
//!
//! A script can do anything the page can, so a script that threw is not a script that did
//! nothing: the browser may already have acted before the exception. The service reports the
//! exception as an unknown effect and keeps the description the page gave.

use super::super::link::Link;
use super::super::scope::Scope;
use super::super::wire::EvaluateMain;
use crate::browser::adapter::BrowserOutcome;
use crate::browser::BrowserError;

/// How long the page may settle after a script, in milliseconds.
const SETTLE_MS: u64 = 100;

/// Evaluate an explicit script in the top document's main world.
///
/// The scope's guard rides on the evaluation, so a page that changed since the request was
/// authorized is not touched.
///
/// # Errors
///
/// Returns the failure of the primitive, and an unknown effect carrying the page's description
/// when the script threw.
pub fn evaluate(
    l: &mut Link<'_>,
    scope: Option<&mut Scope>,
    tab_id: u64,
    script: &str,
    max_result_chars: usize,
) -> Result<BrowserOutcome, BrowserError> {
    let evaluated = l.send(EvaluateMain {
        tab_id,
        script: script.to_owned(),
        max_result_chars,
        guard: scope.as_deref().map(Scope::guard),
        settle_ms: SETTLE_MS,
        timeout_ms: l.budget_ms(),
    })?;
    if let Some(description) = evaluated.exception {
        return Err(BrowserError::EffectUnknown(description));
    }
    Ok(BrowserOutcome::ScriptEvaluated {
        tab: l.tab(tab_id)?.into(),
        value: evaluated.value,
        truncated: evaluated.truncated,
        committed_urls: l.committed(),
    })
}
