//! Closed composition receipt identities and readable retained-history explanations.

use crate::governance::evidence::PermissionCheck;
use crate::governance::ReasonCode;
use serde::{Deserialize, Serialize};

/// Retired passive browser-event receipt. It was never an agent invocation.
pub const RETIRED_BROWSER_LANDING: &str = "browser_landing";

/// Maximum child count supported by a composition, including restored history.
pub const COMPOSITION_STEP_LIMIT: usize = 20;

/// One language-owned human rendering of terminal facts; machine status remains unchanged.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutcomePresentation {
    /// Human-readable outcome label, independent of protocol status.
    pub label: String,
    /// Closed visual tone chosen by the orchestrator.
    pub tone: OutcomeTone,
    /// The authored sentence that leads the human surface.
    pub summary: String,
    /// Repeat guidance; absent safety evidence never becomes a safe-repeat claim.
    pub repeat_detail: String,
    /// Local guidance beside the existing exact-tab reveal control.
    pub reveal_detail: String,
}

/// Presentation distinctions shared by the hero, historical rows, and child receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeTone {
    Complete,
    Controlled,
    Refused,
    Failed,
    Caution,
}

impl OutcomePresentation {
    /// Keep a live composition distinct from a restored group without a terminal parent receipt.
    #[must_use]
    pub fn incomplete_composition(completed: usize, total: usize, live: bool) -> Self {
        use super::outcome::Outcome;
        Self {
            label: if live {
                "In progress"
            } else {
                "Completion unrecorded"
            }
            .into(),
            tone: OutcomeTone::Caution,
            summary: if live {
                Outcome::CompositionProgress { completed, total }.summary()
            } else {
                Outcome::CompositionUnrecorded.summary()
            },
            repeat_detail: if live {
                String::new()
            } else {
                "Completion was not recorded. Inspect the page before preparing unfinished work."
                    .into()
            },
            reveal_detail: super::control::SHOW_TAB_GUIDANCE.into(),
        }
    }

    /// Describe recorded facts without inferring intent, rollback, or permission to replay.
    #[must_use]
    pub fn from_record(record: &crate::governance::AuditRecord) -> Self {
        // A configured restriction can refuse work after its capability check permitted it.
        let policy_refused = !record.allowed
            || matches!(
                &record.refusal_facts,
                Some(
                    super::audit::AuditRefusal::BrowserAttentionProtected { .. }
                        | super::audit::AuditRefusal::RequestRestricted { .. }
                )
            );
        let (label, tone) = if record.effect == "unknown" {
            ("Effects uncertain", OutcomeTone::Caution)
        } else if record.effect == "partial" {
            ("Partly completed", OutcomeTone::Caution)
        } else if record.reason == ReasonCode::RuntimeHold {
            ("Paused by you", OutcomeTone::Controlled)
        } else if record.reason == ReasonCode::SessionEnded {
            ("Stopped by you", OutcomeTone::Controlled)
        } else if record.status == "succeeded" {
            ("Completed", OutcomeTone::Complete)
        } else if policy_refused {
            ("Request refused", OutcomeTone::Refused)
        } else {
            ("Could not complete", OutcomeTone::Failed)
        };
        let summary = if record.effect == "none" && record.reason == ReasonCode::RuntimeHold {
            "You paused Ghostlight. This request did not run.".into()
        } else if record.effect == "none" && record.reason == ReasonCode::SessionEnded {
            "You stopped Ghostlight. This request did not run.".into()
        } else {
            record.summary.clone()
        };
        let repeat_detail = if matches!(record.effect.as_str(), "unknown" | "partial") {
            "Do not repeat this action. Observe the page before preparing unfinished work."
        } else if record.effect == "applied" && record.status != "succeeded" {
            "The change was applied. Inspect the page before preparing unfinished work."
        } else {
            ""
        };
        Self {
            label: label.into(),
            tone,
            summary,
            repeat_detail: repeat_detail.into(),
            reveal_detail: super::control::SHOW_TAB_GUIDANCE.into(),
        }
    }
}

/// Current flow and historical sequence identities; caller labels never supply an identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionKind {
    Flow,
    /// Retained for receipts written before sequence was consolidated into flow.
    Sequence,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn configured_restriction_is_a_refusal_after_a_permitted_capability_check() {
        use crate::governance::{AuditRecord, CapabilitySet, Decision};
        use crate::language::outcome::Refusal;
        use ghostlight_bridge::browser::BrowserAttentionReason;
        for refusal in [
            super::super::audit::AuditRefusal::BrowserAttentionProtected {
                cause: BrowserAttentionReason::Focus,
            },
            super::super::audit::AuditRefusal::RequestRestricted {
                cause: crate::language::outcome::BlockedReason::Capability,
            },
        ] {
            let mut record = AuditRecord::now(
                "call",
                "workspace",
                "browser_tabs",
                CapabilitySet::ACTION,
                "authority",
                Decision::permitted(),
                "blocked",
                "none",
                &Refusal::BrowserAttentionProtected {
                    reason: BrowserAttentionReason::Focus,
                }
                .audit(),
                0,
            );
            record.refusal_facts = Some(refusal);
            assert!(record.allowed);
            assert_eq!(record.reason, ReasonCode::Permitted);
            let presentation = OutcomePresentation::from_record(&record);
            assert_eq!(presentation.tone, OutcomeTone::Refused);
            assert_eq!(presentation.label, "Request refused");
            assert_eq!(presentation.summary, record.summary);
        }
    }

    #[test]
    fn outcome_tones_preserve_control_policy_and_effect_distinctions() {
        use crate::governance::{AuditRecord, CapabilitySet, Decision};
        use crate::language::outcome::{BlockedReason, Refusal};
        for (reason, status, effect, tone, label) in [
            (
                ReasonCode::Permitted,
                "succeeded",
                "none",
                OutcomeTone::Complete,
                "Completed",
            ),
            (
                ReasonCode::RuntimeHold,
                "blocked",
                "none",
                OutcomeTone::Controlled,
                "Paused by you",
            ),
            (
                ReasonCode::SessionEnded,
                "blocked",
                "none",
                OutcomeTone::Controlled,
                "Stopped by you",
            ),
            (
                ReasonCode::HostDenied,
                "blocked",
                "none",
                OutcomeTone::Refused,
                "Request refused",
            ),
            (
                ReasonCode::Permitted,
                "failed",
                "none",
                OutcomeTone::Failed,
                "Could not complete",
            ),
            (
                ReasonCode::RuntimeHold,
                "blocked",
                "unknown",
                OutcomeTone::Caution,
                "Effects uncertain",
            ),
            (
                ReasonCode::SessionEnded,
                "blocked",
                "partial",
                OutcomeTone::Caution,
                "Partly completed",
            ),
        ] {
            let record = AuditRecord::now(
                "call",
                "workspace",
                "browser_execute",
                CapabilitySet::EXECUTE,
                "authority",
                if reason == ReasonCode::Permitted {
                    Decision::permitted()
                } else {
                    Decision::refused(reason)
                },
                status,
                effect,
                &Refusal::AuthorityBlocked {
                    reason: BlockedReason::Hold,
                    host: None,
                }
                .audit(),
                0,
            );
            let presentation = OutcomePresentation::from_record(&record);
            assert_eq!(presentation.tone, tone);
            assert_eq!(presentation.label, label);
            assert_eq!(record.status, status);
            assert_eq!(record.effect, effect);
            if tone == OutcomeTone::Caution {
                assert!(presentation.repeat_detail.contains("Do not repeat"));
            }
        }
    }

    #[test]
    fn retired_request_and_sequence_receipts_keep_their_original_meaning() {
        let check: PermissionCheck = serde_json::from_value(json!({
            "requirements":["execute"],"host":null,"allowed":false,"observed":false,
            "reason":"host_denied","layers":[],"request_restricted":true,"request_evaluated":true
        }))
        .unwrap();
        assert_eq!(
            permission(&check),
            "Refused: request restrictions refused this work."
        );
        let receipt: StepReceipt = serde_json::from_value(json!({
            "parent":"sequence","position":2,"total":3,"preparation_failed":false
        }))
        .unwrap();
        assert_eq!(receipt.parent.tool(), "browser_sequence");
    }
}

impl CompositionKind {
    /// Return the canonical catalog name.
    #[must_use]
    pub const fn tool(self) -> &'static str {
        match self {
            Self::Flow => "browser_flow",
            Self::Sequence => "browser_sequence",
        }
    }
}

/// Parent invocation plus position identifies one child receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepReceipt {
    pub parent: CompositionKind,
    pub position: usize,
    pub total: usize,
    pub preparation_failed: bool,
}

/// Explain actual permission evidence, without reading today's policy or guessing grants.
#[must_use]
pub fn permission(check: &PermissionCheck) -> String {
    let verdict = if check.observed {
        "Allowed in observe mode; policy would refuse this work"
    } else if check.allowed {
        "Allowed"
    } else {
        "Refused"
    };
    if check.reason != ReasonCode::Permitted && check.layers.is_empty() && !check.request_evaluated
    {
        return format!("{verdict}: {}.", check.reason.as_str().replace('_', " "));
    }
    if check.requirements.is_empty() {
        return format!("{verdict}; no capability grant was required.");
    }
    if check.layers.is_empty() && !check.request_restricted {
        return format!("{verdict} without configured policy.");
    }
    let mut sources: Vec<String> = check
        .layers
        .iter()
        .map(|layer| {
            let source = if layer.tier == "managed" {
                "organization policy"
            } else {
                "local policy"
            };
            let rules = if layer.grants.is_empty() {
                "no matching grant".into()
            } else {
                layer.grants.join(", ")
            };
            let verdict = if layer.allowed {
                "permits this work"
            } else {
                "refuses this work"
            };
            format!("{source} {verdict} ({rules})")
        })
        .collect();
    if check.request_restricted {
        sources.push(
            if !check.request_evaluated {
                "request restrictions were not evaluated"
            } else if check.allowed {
                "request restrictions permit this work"
            } else {
                "request restrictions refused this work"
            }
            .into(),
        );
    }
    format!("{verdict}: {}.", sources.join("; "))
}
