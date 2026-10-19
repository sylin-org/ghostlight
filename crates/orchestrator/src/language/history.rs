//! Closed composition receipt identities and readable retained-history explanations.
//!
//! History is read by a person in the window. A record keeps the summary the model was told, and
//! a few of those speak to the model about "the user": the human controls' directives and the
//! channel refusal. [`for_the_person`] gives the window its own sentence for them, and the
//! permission explanations name a refusal's reason in words rather than as a code.

use crate::governance::evidence::PermissionCheck;
use crate::governance::{ReasonCode, MANAGED_TIER};
use crate::intake::session::IntakeChannel;
use crate::language::channel_switch;
use crate::language::outcome::{PolicyAuthor, HUMAN_PAUSE_DIRECTIVE, HUMAN_STOP_DIRECTIVE};
use serde::{Deserialize, Serialize};

/// What the window says in place of the pause directive the model was told.
const YOU_PAUSED: &str = "You paused Ghostlight.";

/// What the window says in place of the stop directive the model was told.
const YOU_ENDED: &str = "You ended the session.";

/// Whose layer a record's `policy_tier` names: the organization's for the managed tier, and the
/// person's own otherwise.
#[must_use]
pub fn author_of(tier: Option<&str>) -> PolicyAuthor {
    if tier == Some(MANAGED_TIER) {
        PolicyAuthor::Organization
    } else {
        PolicyAuthor::User
    }
}

/// The sentence the window shows for a recorded action whose `summary` speaks to the model, or
/// `None` when the summary already reads right for a person.
///
/// A channel refusal says whose policy turned off which work: "Your policy turns off work from MCP
/// agents.". A record keeps no organization name, so the organization's refusal says "Your
/// organization's policy", and the refusal line beneath it names the organization. A pause or a
/// stop says what the person did, followed by whatever the model's sentence went on to say about
/// the action.
#[must_use]
pub fn for_the_person(
    summary: &str,
    reason: ReasonCode,
    channel: IntakeChannel,
    tier: Option<&str>,
) -> Option<String> {
    if reason == ReasonCode::ChannelDenied {
        return Some(channel_switch::refused(channel, author_of(tier), None));
    }
    [
        (HUMAN_PAUSE_DIRECTIVE, YOU_PAUSED),
        (HUMAN_STOP_DIRECTIVE, YOU_ENDED),
    ]
    .into_iter()
    .find_map(|(directive, said)| {
        summary
            .strip_prefix(directive)
            .map(|rest| format!("{said}{rest}"))
    })
}

/// Why a check no policy layer decided was refused, as the end of "Refused: ...".
const fn refused_because(reason: ReasonCode) -> &'static str {
    match reason {
        ReasonCode::Permitted => "nothing refused it",
        ReasonCode::CapabilityDenied => "policy does not allow this kind of work",
        ReasonCode::TabCloseDenied => "policy does not let agents close tabs",
        ReasonCode::ScreenshotDenied => "policy turns off screenshots",
        ReasonCode::ChannelDenied => "policy turns off work from this channel",
        ReasonCode::HostDenied => "policy does not allow this site",
        ReasonCode::ProtectedHost => "this site is protected and never automated",
        ReasonCode::OwnWorkbench => "Ghostlight's own control page is never automated",
        ReasonCode::InvalidAuthority => "a policy that applies is not valid",
        ReasonCode::InvalidRequest => "the call did not match the Ghostlight catalog",
        ReasonCode::RuntimeHold => "you paused Ghostlight",
        ReasonCode::SessionEnded => "you ended the session",
        ReasonCode::AuditUnavailable => "policy requires saved history, and it could not be saved",
    }
}

/// Maximum child count supported by a composition, including restored history.
pub const COMPOSITION_STEP_LIMIT: usize = 20;

/// The composition identity; caller labels never supply one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionKind {
    Flow,
}

impl CompositionKind {
    /// Return the canonical catalog name.
    #[must_use]
    pub const fn tool(self) -> &'static str {
        match self {
            Self::Flow => "browser_flow",
        }
    }
}

/// Parent invocation plus position identifies one child receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StepReceipt {
    pub parent: CompositionKind,
    pub position: usize,
    pub total: usize,
    pub preparation_failed: bool,
}

/// Explain actual permission evidence, without reading today's policy or guessing grants.
///
/// `channel` and `tier` are the record's own: a check its channel switch refused takes the words
/// the window's row says ([`for_the_person`]).
#[must_use]
pub fn permission(check: &PermissionCheck, channel: IntakeChannel, tier: Option<&str>) -> String {
    let verdict = if check.allowed { "Allowed" } else { "Refused" };
    if check.reason == ReasonCode::ChannelDenied {
        let said = channel_switch::refused(channel, author_of(tier), None);
        return format!("{verdict}. {said}");
    }
    if check.reason != ReasonCode::Permitted && check.layers.is_empty() {
        return format!("{verdict}: {}.", refused_because(check.reason));
    }
    if check.requirements.is_empty() {
        return format!("{verdict}; no capability grant was required.");
    }
    if check.layers.is_empty() {
        return format!("{verdict} without configured policy.");
    }
    let sources: Vec<String> = check
        .layers
        .iter()
        .map(|layer| {
            let source = if layer.tier == MANAGED_TIER {
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
    format!("{verdict}: {}.", sources.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::CapabilitySet;

    fn refused(reason: ReasonCode) -> PermissionCheck {
        PermissionCheck {
            requirements: CapabilitySet::EMPTY,
            host: None,
            allowed: false,
            reason,
            layers: vec![],
        }
    }

    #[test]
    fn a_channel_refusal_speaks_to_the_person_in_the_row_and_its_details() {
        let model = "Blocked: the user's Ghostlight policy turns off work from MCP agents.";
        assert_eq!(
            for_the_person(
                model,
                ReasonCode::ChannelDenied,
                IntakeChannel::Mcp,
                Some("user")
            )
            .as_deref(),
            Some("Your policy turns off work from MCP agents.")
        );
        assert_eq!(
            for_the_person(
                model,
                ReasonCode::ChannelDenied,
                IntakeChannel::Cli,
                Some(MANAGED_TIER)
            )
            .as_deref(),
            Some("Your organization's policy turns off work from the command line.")
        );
        assert_eq!(
            permission(
                &refused(ReasonCode::ChannelDenied),
                IntakeChannel::Mcp,
                Some("user")
            ),
            "Refused. Your policy turns off work from MCP agents."
        );
    }

    #[test]
    fn the_human_controls_say_what_the_person_did() {
        assert_eq!(
            for_the_person(
                HUMAN_PAUSE_DIRECTIVE,
                ReasonCode::RuntimeHold,
                IntakeChannel::Mcp,
                None
            )
            .as_deref(),
            Some("You paused Ghostlight.")
        );
        let unconfirmed = format!(
            "{HUMAN_STOP_DIRECTIVE} The expected condition was not checked. Clicked the \"Save\" button on example.com."
        );
        assert_eq!(
            for_the_person(
                &unconfirmed,
                ReasonCode::SessionEnded,
                IntakeChannel::Mcp,
                None
            )
            .as_deref(),
            Some("You ended the session. The expected condition was not checked. Clicked the \"Save\" button on example.com.")
        );
        // A sentence already written for a person is kept.
        assert_eq!(
            for_the_person(
                "Opened example.com.",
                ReasonCode::Permitted,
                IntakeChannel::Mcp,
                None
            ),
            None
        );
    }

    #[test]
    fn a_refusal_no_layer_decided_is_said_in_words_not_as_a_code() {
        assert_eq!(
            permission(&refused(ReasonCode::RuntimeHold), IntakeChannel::Mcp, None),
            "Refused: you paused Ghostlight."
        );
        for reason in [
            ReasonCode::CapabilityDenied,
            ReasonCode::TabCloseDenied,
            ReasonCode::ScreenshotDenied,
            ReasonCode::HostDenied,
            ReasonCode::ProtectedHost,
            ReasonCode::OwnWorkbench,
            ReasonCode::InvalidAuthority,
            ReasonCode::InvalidRequest,
            ReasonCode::RuntimeHold,
            ReasonCode::SessionEnded,
            ReasonCode::AuditUnavailable,
        ] {
            let said = permission(&refused(reason), IntakeChannel::Mcp, None);
            assert!(!said.contains('_'), "{said}");
            assert!(said.is_ascii() && said.ends_with('.'), "{said}");
        }
    }
}
