//! The words the in-page notice shows a person when Ghostlight holds a tab.
//!
//! A tab is held when governance refuses the page it shows (ADR-0203 Decisions 2.4 and 5.5). The
//! hold is the policy's refusal of that page, not the person's Pause, so the words say what the
//! page is to this session and what Ghostlight will not do there.

/// The notice's phase line when Ghostlight holds a tab on a refused page.
pub const HOLD_PHASE: &str = "Ghostlight stopped on this page";

/// The notice's detail line when Ghostlight holds a tab on a refused page.
pub const HOLD_DETAIL: &str =
    "This session may not use this page. Ghostlight will not read or act on it until the tab leaves it.";
