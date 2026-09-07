//! Content-free history storage facts and language, independent of browser effect truth.

use serde::{Deserialize, Serialize};

/// Whether this receipt's storage was acknowledged.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Storage {
    /// The sink acknowledged writing and synchronizing the receipt.
    #[default]
    Saved,
    /// The sink did not confirm storage; some bytes may still have been written.
    Unconfirmed,
}

/// Qualify a saved parent receipt whose children have an explicit storage gap.
#[must_use]
pub fn qualify_children(summary: &str, count: u32) -> String {
    if count == 0 {
        summary.into()
    } else {
        format!(
            "{} Some step history could not be saved.",
            summary.chars().take(461).collect::<String>()
        )
    }
}

impl Storage {
    /// Qualify an action without changing its outcome or suggesting replay.
    #[must_use]
    pub fn qualify(self, summary: &str) -> String {
        if self == Self::Saved {
            summary.into()
        } else {
            format!(
                "{} History could not be saved.",
                summary.chars().take(471).collect::<String>()
            )
        }
    }

    /// Human detail for a receipt held only by the running service.
    #[must_use]
    pub const fn detail(self) -> &'static str {
        match self {
            Self::Saved => "",
            Self::Unconfirmed => "History could not be saved. This receipt is available in the current view; storage was not confirmed.",
        }
    }
}

/// Small closed failure categories; operating-system paths and errors stay private.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageFailure {
    Access,
    Unavailable,
    Write,
}

/// Current ability to save new receipts, separately from earlier history gaps.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditHealth {
    /// Known current storage failure, if any.
    pub failure: Option<StorageFailure>,
    /// Receipts not confirmed saved, including restored recovery markers.
    pub unconfirmed_receipts: u64,
    /// Invalid historical entries skipped without discarding readable neighbors.
    pub unreadable_entries: u64,
    /// Earlier history could not be fully read during startup.
    pub history_unavailable: bool,
}

impl AuditHealth {
    /// Whether the current sink is known unable to save receipts.
    #[must_use]
    pub const fn unavailable(&self) -> bool {
        self.failure.is_some()
    }

    /// A persistent, non-popup explanation for the existing history and status surfaces.
    #[must_use]
    pub fn detail(&self, required: bool) -> String {
        let mut detail = if self.unavailable() {
            if required {
                "History cannot be saved. Policy stops new browser work until saving recovers. Ghostlight checks automatically.".into()
            } else {
                "History cannot be saved. Browser work can continue. Ghostlight checks automatically.".into()
            }
        } else {
            "History is saving.".to_owned()
        };
        if let Some(failure) = self.failure {
            detail.push_str(match failure {
                StorageFailure::Access => " Check access to the history destination.",
                StorageFailure::Unavailable => " Restore the history file location.",
                StorageFailure::Write => " Check free space and access to the history destination.",
            });
        }
        if self.unconfirmed_receipts > 0 {
            if self.unconfirmed_receipts == 1 {
                detail.push_str(" 1 earlier receipt was not confirmed saved.");
            } else {
                detail.push_str(&format!(
                    " {} earlier receipts were not confirmed saved.",
                    self.unconfirmed_receipts
                ));
            }
        }
        if self.unreadable_entries > 0 {
            detail.push_str(&format!(
                " {} unreadable history entries were skipped.",
                self.unreadable_entries
            ));
        }
        if self.history_unavailable {
            detail.push_str(" Earlier history could not be fully loaded. Restart Ghostlight to reload it once storage is available.");
        }
        detail
    }

    /// Whether healthy ordinary operation needs no proactive indication.
    #[must_use]
    pub const fn quiet(&self) -> bool {
        self.failure.is_none()
            && self.unconfirmed_receipts == 0
            && self.unreadable_entries == 0
            && !self.history_unavailable
    }
}
