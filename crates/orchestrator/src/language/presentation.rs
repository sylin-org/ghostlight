//! The words Ghostlight's in-page presentation shows a person: the caption of each activity, and
//! the phase and detail lines of the notice when Ghostlight holds a tab or blocks an action.
//!
//! The page draws these words as the service sends them and keeps no copy of its own. A tab is
//! held when governance refuses the page it shows (ADR-0203 Decisions 2.4 and 5.5). The hold is
//! the policy's refusal of that page, not the person's Pause, so the words say what the page is to
//! this session and what Ghostlight will not do there.

use crate::browser::adapter::PresentationActivity;
use crate::events::DenialPresentation;

/// The notice's phase line when Ghostlight holds a tab on a refused page.
pub const HOLD_PHASE: &str = "Ghostlight stopped on this page";

/// The notice's detail line when Ghostlight holds a tab on a refused page.
pub const HOLD_DETAIL: &str =
    "This session may not use this page. Ghostlight will not read or act on it until the tab leaves it.";

/// The caption the page shows while Ghostlight does `activity`.
#[must_use]
pub const fn activity_label(activity: PresentationActivity) -> &'static str {
    match activity {
        PresentationActivity::Quiet => "Ghostlight",
        PresentationActivity::Navigate => "Navigating",
        PresentationActivity::Click => "Clicking",
        PresentationActivity::Hover => "Hovering",
        PresentationActivity::Drag => "Dragging",
        PresentationActivity::Type => "Typing",
        PresentationActivity::Key => "Keyboard",
        PresentationActivity::Scroll => "Scrolling",
        PresentationActivity::Read => "Reading page",
        PresentationActivity::Find => "Finding on page",
        PresentationActivity::Screenshot => "Screenshot",
        PresentationActivity::Zoom => "Zooming",
        PresentationActivity::Fill => "Filling form",
        PresentationActivity::Upload => "Uploading file",
        PresentationActivity::Script => "Running JavaScript",
        PresentationActivity::Wait => "Waiting",
        PresentationActivity::Dialog => "Browser dialog",
    }
}

/// The notice's phase and detail lines for a blocked browser job, in that order.
#[must_use]
pub const fn denial_lines(presentation: DenialPresentation) -> (&'static str, &'static str) {
    match presentation {
        DenialPresentation::Guardrail => (
            "Ghostlight blocked this action",
            "A configured guardrail prevented it.",
        ),
        DenialPresentation::TabKeptOpenByPolicy => (
            "Ghostlight kept this tab open",
            "Closing tabs is blocked by policy. You can close it yourself.",
        ),
        DenialPresentation::TabKeptOpenBySetting => (
            "Ghostlight kept this tab open",
            "Your Preserve Ghostlight tabs setting is on. You can close it yourself.",
        ),
    }
}
