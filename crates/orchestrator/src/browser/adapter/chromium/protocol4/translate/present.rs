//! One-way presentation notices. Nothing answers a notice, so presenting never waits.

use super::super::link::Link;
use super::super::locator::Locator;
use super::super::wire;
use crate::browser::adapter::{BrowserOutcome, PresentationKind, PresentationSignal};
use crate::browser::BrowserError;

/// Write one presentation notice and return at once. The extension draws it on the signal's
/// tab, in the document its locator names, and holds a denial until the tab is visible. A
/// signal with no tab has nowhere to be drawn and is dropped.
///
/// # Errors
///
/// Returns the plain failure of a locator that is not a protocol-4 locator, and the writer's
/// failure.
pub fn present(l: &Link<'_>, signal: PresentationSignal) -> Result<BrowserOutcome, BrowserError> {
    let Some(tab_id) = signal.tab_id else {
        return Ok(BrowserOutcome::Presented { rendered: false });
    };
    let target = signal.locator.as_deref().map(Locator::decode).transpose()?;
    let (frame, locator) = target.map_or((None, None), |target| {
        (Some(target.doc), Some(target.local))
    });
    l.notice(wire::Notice {
        tab_id,
        frame,
        defer_until_visible: signal.signal == PresentationKind::Denial,
        signal: wire::NoticeSignal {
            invocation: signal.invocation,
            signal: signal.signal.into(),
            activity: signal.activity.into(),
            phase: signal.phase,
            label: Some(signal.label),
            detail: signal.detail,
            locator,
            click: signal.click.map(Into::into),
        },
    })?;
    Ok(BrowserOutcome::Presented { rendered: false })
}
