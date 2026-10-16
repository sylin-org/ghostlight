//! Why a target handle no longer works.
//!
//! One closed vocabulary shared by the workspace, which decides most handles are stale, the page
//! adapter, which can report that a control left the page, and the language, which says it to the
//! model. A stale handle always says why in one of these four words, whoever noticed
//! (ADR-0198 Decision 27).

use serde::{Deserialize, Serialize};

/// The reason a target handle is stale.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleReason {
    /// The control left the page it was read on, and the page is still the same document.
    Removed,
    /// The tab went on to another document.
    Navigated,
    /// The tab the control was read on is closed.
    TabClosed,
    /// Ghostlight holds no record of this handle: never issued, or dropped to bound memory.
    Unknown,
}

impl StaleReason {
    /// Every reason, in the order the model's vocabulary lists them.
    pub const ALL: [Self; 4] = [
        Self::Removed,
        Self::Navigated,
        Self::TabClosed,
        Self::Unknown,
    ];

    /// The word that names this reason in facts and in the audit.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Removed => "removed",
            Self::Navigated => "navigated",
            Self::TabClosed => "tab_closed",
            Self::Unknown => "unknown",
        }
    }
}
