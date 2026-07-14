// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Per-session denial-burst attention state (ADR-0079).
//!
//! This module is pure over caller-supplied instants and normalized decision facts. It owns no
//! transport, presentation, or live-resource mechanism. Infrastructure supplies a session-local
//! instance, presents returned transitions, and applies explicit human dispositions.

use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::governance::ports::Capability;

/// Matching denials required to open the circuit.
pub const MATCHING_DENIAL_THRESHOLD: u32 = 3;
/// Rolling window for matching denials.
pub const MATCHING_DENIAL_WINDOW: Duration = Duration::from_secs(60);
/// All enforced denials required to open the circuit.
pub const SESSION_DENIAL_THRESHOLD: u32 = 5;
/// Rolling window for all enforced denials in one session.
pub const SESSION_DENIAL_WINDOW: Duration = Duration::from_secs(120);

/// The service-authored class of an enforced denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DenialCategory {
    /// The always-on never-touch boundary denied the request.
    Sacred,
    /// An active grant or session overlay denied the request.
    Policy,
}

impl DenialCategory {
    /// Stable audit and presentation vocabulary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sacred => "sacred",
            Self::Policy => "policy",
        }
    }
}

/// Which rolling threshold opened a circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdKind {
    /// The same decision signature repeated in the short window.
    Matching,
    /// The session produced enough enforced denials of any signature.
    Session,
}

impl ThresholdKind {
    /// Stable audit and presentation vocabulary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Matching => "matching",
            Self::Session => "session",
        }
    }

    /// The configured window for this threshold.
    pub fn window(self) -> Duration {
        match self {
            Self::Matching => MATCHING_DENIAL_WINDOW,
            Self::Session => SESSION_DENIAL_WINDOW,
        }
    }
}

/// One enforced denial signal, already normalized by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenialSignal {
    /// Normalized governing origin when known. Never page text or a full URL.
    pub origin: Option<String>,
    /// The complete capability requirement set for the denied operation.
    pub capabilities: Vec<Capability>,
    /// The service-authored denial category.
    pub category: DenialCategory,
}

/// State retained while human attention is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseState {
    /// The threshold that opened the circuit.
    pub threshold: ThresholdKind,
    /// The count observed inside that threshold's rolling window.
    pub count: u32,
    /// The denial signature that caused the transition.
    pub signal: DenialSignal,
}

/// Result of observing one enforced denial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationResult {
    /// Whether to show the ordinary transient denial sticker.
    pub present_isolated: bool,
    /// The newly opened state, present only on the closed-to-open transition.
    pub opened: Option<PauseState>,
}

/// A human disposition relayed from a trusted local presentation surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionDisposition {
    /// Leave the circuit open.
    KeepPaused,
    /// Close the circuit and clear rolling history.
    Resume,
    /// Close and quiet identical isolated-denial presentation for this session.
    ResumeQuiet,
    /// End the enclosing session through its existing panic path.
    EndSession,
}

impl AttentionDisposition {
    /// Parse the exact local wire vocabulary.
    pub fn parse_wire(value: &str) -> Option<Self> {
        match value {
            "keep_paused" => Some(Self::KeepPaused),
            "resume" => Some(Self::Resume),
            "resume_quiet" => Some(Self::ResumeQuiet),
            "end_session" => Some(Self::EndSession),
            _ => None,
        }
    }

    /// Stable audit vocabulary.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeepPaused => "keep_paused",
            Self::Resume => "resume",
            Self::ResumeQuiet => "resume_quiet",
            Self::EndSession => "end_session",
        }
    }
}

/// One audit-worthy circuit transition returned to infrastructure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionEvent {
    /// Stable event vocabulary.
    pub event: &'static str,
    /// The state that caused the transition.
    pub state: PauseState,
    /// Opening threshold, only for `attention_opened`.
    pub threshold: Option<ThresholdKind>,
    /// Opening count, only for `attention_opened`.
    pub count: Option<u32>,
    /// Human disposition, only for a closing transition.
    pub disposition: Option<AttentionDisposition>,
}

impl AttentionEvent {
    /// Build an opening transition.
    pub fn opened(state: PauseState) -> Self {
        Self {
            event: "attention_opened",
            threshold: Some(state.threshold),
            count: Some(state.count),
            state,
            disposition: None,
        }
    }

    /// Build an audit-worthy closing transition. Keep-paused is intentionally not a transition.
    pub fn disposition(state: PauseState, disposition: AttentionDisposition) -> Option<Self> {
        let event = match disposition {
            AttentionDisposition::KeepPaused => return None,
            AttentionDisposition::Resume => "attention_resumed",
            AttentionDisposition::ResumeQuiet => "attention_quieted",
            AttentionDisposition::EndSession => "attention_ended",
        };
        Some(Self {
            event,
            state,
            threshold: None,
            count: None,
            disposition: Some(disposition),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Signature {
    origin: Option<String>,
    capabilities: Vec<Capability>,
    category: DenialCategory,
}

impl From<&DenialSignal> for Signature {
    fn from(signal: &DenialSignal) -> Self {
        Self {
            origin: signal.origin.clone(),
            capabilities: signal.capabilities.clone(),
            category: signal.category,
        }
    }
}

#[derive(Debug, Clone)]
struct Observation {
    at: Instant,
    signature: Signature,
}

/// One session's memory-only denial attention circuit.
#[derive(Debug, Default)]
pub struct AttentionCircuit {
    observations: VecDeque<Observation>,
    paused: Option<PauseState>,
    quieted: HashSet<(Option<String>, DenialCategory)>,
}

impl AttentionCircuit {
    /// Create an empty, closed circuit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe an enforced denial at `now` and return presentation/transition instructions.
    pub fn observe_at(&mut self, signal: DenialSignal, now: Instant) -> ObservationResult {
        if self.paused.is_some() {
            return ObservationResult {
                present_isolated: false,
                opened: None,
            };
        }

        self.observations
            .retain(|entry| now.saturating_duration_since(entry.at) <= SESSION_DENIAL_WINDOW);
        let signature = Signature::from(&signal);
        self.observations.push_back(Observation {
            at: now,
            signature: signature.clone(),
        });

        let matching = self
            .observations
            .iter()
            .filter(|entry| {
                entry.signature == signature
                    && now.saturating_duration_since(entry.at) <= MATCHING_DENIAL_WINDOW
            })
            .count() as u32;
        let total = self.observations.len() as u32;
        let threshold = if matching >= MATCHING_DENIAL_THRESHOLD {
            Some((ThresholdKind::Matching, matching))
        } else if total >= SESSION_DENIAL_THRESHOLD {
            Some((ThresholdKind::Session, total))
        } else {
            None
        };

        if let Some((threshold, count)) = threshold {
            let state = PauseState {
                threshold,
                count,
                signal,
            };
            self.paused = Some(state.clone());
            return ObservationResult {
                present_isolated: false,
                opened: Some(state),
            };
        }

        ObservationResult {
            present_isolated: !self
                .quieted
                .contains(&(signal.origin.clone(), signal.category)),
            opened: None,
        }
    }

    /// The current open state, if human attention is required.
    pub fn paused(&self) -> Option<&PauseState> {
        self.paused.as_ref()
    }

    /// Apply an explicit human disposition. Returns the state that was open before the action.
    pub fn apply(&mut self, disposition: AttentionDisposition) -> Option<PauseState> {
        let prior = self.paused.clone()?;
        match disposition {
            AttentionDisposition::KeepPaused | AttentionDisposition::EndSession => {}
            AttentionDisposition::Resume => {
                self.paused = None;
                self.observations.clear();
            }
            AttentionDisposition::ResumeQuiet => {
                self.quieted
                    .insert((prior.signal.origin.clone(), prior.signal.category));
                self.paused = None;
                self.observations.clear();
            }
        }
        Some(prior)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(origin: &str, category: DenialCategory) -> DenialSignal {
        DenialSignal {
            origin: Some(origin.to_string()),
            capabilities: vec![Capability::Action],
            category,
        }
    }

    #[test]
    fn three_matching_denials_inside_sixty_seconds_open_the_circuit() {
        let start = Instant::now();
        let mut circuit = AttentionCircuit::new();
        assert!(
            circuit
                .observe_at(signal("example.com", DenialCategory::Policy), start)
                .present_isolated
        );
        assert!(
            circuit
                .observe_at(
                    signal("example.com", DenialCategory::Policy),
                    start + Duration::from_secs(30),
                )
                .present_isolated
        );
        let result = circuit.observe_at(
            signal("example.com", DenialCategory::Policy),
            start + Duration::from_secs(59),
        );
        let opened = result.opened.expect("third matching denial opens");
        assert!(!result.present_isolated);
        assert_eq!(opened.threshold, ThresholdKind::Matching);
        assert_eq!(opened.count, MATCHING_DENIAL_THRESHOLD);
    }

    #[test]
    fn expired_matching_denial_does_not_count() {
        let start = Instant::now();
        let mut circuit = AttentionCircuit::new();
        circuit.observe_at(signal("example.com", DenialCategory::Policy), start);
        circuit.observe_at(
            signal("example.com", DenialCategory::Policy),
            start + Duration::from_secs(61),
        );
        let result = circuit.observe_at(
            signal("example.com", DenialCategory::Policy),
            start + Duration::from_secs(62),
        );
        assert!(result.opened.is_none());
    }

    #[test]
    fn five_mixed_denials_inside_two_minutes_open_the_session_threshold() {
        let start = Instant::now();
        let mut circuit = AttentionCircuit::new();
        for i in 0..4 {
            let result = circuit.observe_at(
                signal(
                    &format!("site-{i}.example"),
                    if i % 2 == 0 {
                        DenialCategory::Policy
                    } else {
                        DenialCategory::Sacred
                    },
                ),
                start + Duration::from_secs(i * 20),
            );
            assert!(result.opened.is_none());
        }
        let result = circuit.observe_at(
            signal("last.example", DenialCategory::Policy),
            start + Duration::from_secs(119),
        );
        let opened = result.opened.expect("fifth session denial opens");
        assert_eq!(opened.threshold, ThresholdKind::Session);
        assert_eq!(opened.count, SESSION_DENIAL_THRESHOLD);
    }

    #[test]
    fn resume_clears_state_and_history() {
        let start = Instant::now();
        let mut circuit = AttentionCircuit::new();
        for offset in [0, 1, 2] {
            circuit.observe_at(
                signal("example.com", DenialCategory::Policy),
                start + Duration::from_secs(offset),
            );
        }
        assert!(circuit.paused().is_some());
        circuit.apply(AttentionDisposition::Resume);
        assert!(circuit.paused().is_none());
        let result = circuit.observe_at(
            signal("example.com", DenialCategory::Policy),
            start + Duration::from_secs(3),
        );
        assert!(result.opened.is_none());
        assert!(result.present_isolated);
    }

    #[test]
    fn resume_quiet_suppresses_only_identical_isolated_presentation() {
        let start = Instant::now();
        let mut circuit = AttentionCircuit::new();
        for offset in [0, 1, 2] {
            circuit.observe_at(
                signal("example.com", DenialCategory::Policy),
                start + Duration::from_secs(offset),
            );
        }
        circuit.apply(AttentionDisposition::ResumeQuiet);
        let quiet = circuit.observe_at(
            signal("example.com", DenialCategory::Policy),
            start + Duration::from_secs(3),
        );
        assert!(!quiet.present_isolated);
        let other = circuit.observe_at(
            signal("other.example", DenialCategory::Policy),
            start + Duration::from_secs(4),
        );
        assert!(other.present_isolated);
    }
}
