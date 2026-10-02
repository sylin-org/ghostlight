//! Content-free live wait purpose and phase, authored by Language and never retained in audit.

use super::Wait;

/// A wait's current bounded responsibility; no page text, target name or selector is held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaitProgress {
    Condition {
        purpose: WaitPurpose,
        budget_ms: u64,
    },
    Settlement {
        budget_ms: u64,
    },
}

/// Closed observable purposes. Only an explicit delay retains its content-free measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WaitPurpose {
    Load,
    Url,
    TextPresent,
    TextAbsent,
    TargetPresent,
    TargetAbsent,
    SelectorPresent,
    Visual,
    Layout,
    Duration(u64),
}

impl WaitProgress {
    /// Narrow validated input to live mechanical purpose without carrying its content.
    pub(crate) fn condition(wait: &Wait, budget_ms: u64) -> Self {
        let purpose = match wait.condition.as_str() {
            "load_ready" => WaitPurpose::Load,
            "url_contains" => WaitPurpose::Url,
            "text_present" => WaitPurpose::TextPresent,
            "text_absent" => WaitPurpose::TextAbsent,
            "target_present" => WaitPurpose::TargetPresent,
            "target_absent" => WaitPurpose::TargetAbsent,
            "selector_present" => WaitPurpose::SelectorPresent,
            "visual_settle" => WaitPurpose::Visual,
            "layout_stable" => WaitPurpose::Layout,
            "duration" => WaitPurpose::Duration(
                wait.value
                    .as_deref()
                    .and_then(|raw| raw.parse().ok())
                    .unwrap_or(0),
            ),
            _ => unreachable!("Language validated the wait condition"),
        };
        Self::Condition { purpose, budget_ms }
    }

    /// Compose the live phase from closed purpose and the original remaining time budget.
    pub(crate) fn summary(self) -> String {
        let (purpose, budget_ms) = match self {
            Self::Condition { purpose, budget_ms } => (purpose, budget_ms),
            Self::Settlement { budget_ms } => return format!("Condition observed; waiting for the page to settle (up to {budget_ms} ms remaining)."),
        };
        let purpose = match purpose {
            WaitPurpose::Load => "the page to finish loading",
            WaitPurpose::Url => "the requested address condition",
            WaitPurpose::TextPresent => "the requested text to appear",
            WaitPurpose::TextAbsent => "the requested text to disappear",
            WaitPurpose::TargetPresent | WaitPurpose::SelectorPresent => {
                "the requested control to appear"
            }
            WaitPurpose::TargetAbsent => "the requested control to disappear",
            WaitPurpose::Visual => "the page to settle visually",
            WaitPurpose::Layout => "the layout to become stable",
            WaitPurpose::Duration(duration_ms) => {
                return format!("Waiting for {duration_ms} ms (up to {budget_ms} ms remaining).");
            }
        };
        format!("Waiting for {purpose} (up to {budget_ms} ms remaining).")
    }
}
