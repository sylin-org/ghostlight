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

/// The note on a saved parent receipt whose children have an explicit storage gap.
pub const CHILDREN_UNSAVED: &str = "Some step history could not be saved.";

/// The note on a receipt whose own storage was not confirmed.
pub const HISTORY_UNSAVED: &str = "History could not be saved.";

/// Qualify a saved parent receipt whose children have an explicit storage gap.
#[must_use]
pub fn qualify_children(summary: &str, count: u32) -> String {
    if count == 0 {
        summary.into()
    } else {
        super::outcome::with_note(summary, CHILDREN_UNSAVED)
    }
}

impl Storage {
    /// Qualify an action without changing its outcome or suggesting replay.
    #[must_use]
    pub fn qualify(self, summary: &str) -> String {
        if self == Self::Saved {
            summary.into()
        } else {
            super::outcome::with_note(summary, HISTORY_UNSAVED)
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
            if self.unreadable_entries == 1 {
                detail.push_str(" 1 unreadable history entry was skipped.");
            } else {
                detail.push_str(&format!(
                    " {} unreadable history entries were skipped.",
                    self.unreadable_entries
                ));
            }
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

#[cfg(test)]
mod tests {
    use super::{qualify_children, AuditHealth, Storage, CHILDREN_UNSAVED, HISTORY_UNSAVED};
    use crate::language::outcome::SUMMARY_MAX_CHARS;

    #[test]
    fn skipped_history_entries_are_counted_in_number() {
        let detail = |unreadable_entries| {
            AuditHealth {
                unreadable_entries,
                ..AuditHealth::default()
            }
            .detail(false)
        };
        assert_eq!(
            detail(1),
            "History is saving. 1 unreadable history entry was skipped."
        );
        assert_eq!(
            detail(2),
            "History is saving. 2 unreadable history entries were skipped."
        );
        assert_eq!(detail(0), "History is saving.");
    }

    #[test]
    fn a_storage_note_fills_the_summary_bound_to_the_character() {
        for length in [SUMMARY_MAX_CHARS, 600] {
            let summary = "s".repeat(length);
            for (qualified, note) in [
                (Storage::Unconfirmed.qualify(&summary), HISTORY_UNSAVED),
                (qualify_children(&summary, 2), CHILDREN_UNSAVED),
            ] {
                assert_eq!(qualified.chars().count(), SUMMARY_MAX_CHARS, "{note}");
                assert!(qualified.ends_with(&format!(" {note}")), "{note}");
            }
        }
        assert_eq!(Storage::Saved.qualify("Done."), "Done.");
        assert_eq!(qualify_children("Done.", 0), "Done.");
        assert_eq!(
            Storage::Unconfirmed.qualify("Done."),
            "Done. History could not be saved."
        );
    }
}
