// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
// See docs/licenses/LicenseRef-Ghostlight-Commercial.txt.

//! The compiled policy a person reads.
//!
//! Ghostlight decides authority per invocation from an immutable snapshot. That is the right shape
//! for enforcement and the wrong shape for a person, who arrives with one question -- what can the
//! agent do right now, and who decided that -- and cannot get it from a stack of layers.
//!
//! This module answers first and shows the derivation second, which is the posture every mature
//! layered-policy surface settled on (ADR-0122 Decision 2). Every line it produces names the layer
//! that decided it. The words are authored here, in the orchestrator, so the window renders
//! sentences rather than inventing them from booleans.

use serde::Serialize;

use super::{manifest, Capability, CapabilitySet, ManagedPolicyPassport};

/// Which layers are in force right now.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Situation {
    /// No authored policy applies.
    AllOpen,
    /// An organization layer applies and this person has authored nothing.
    OrganizationOnly,
    /// This person's own layer applies and no organization layer does.
    UserOnly,
    /// Both layers apply.
    Layered,
    /// A configured source is invalid, so governed work is refused.
    FailingClosed,
}

/// Who decided one line. A closed vocabulary; there is no fourth kind of authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    /// The organization managing this machine.
    Organization,
    /// The person using this machine.
    User,
    /// Ghostlight itself, through a boundary no policy can lift.
    Ghostlight,
}

/// Whether one capability is available, how widely, and which way the rules point.
///
/// Polarity is the difference between a rule that carves holes out of an open baseline and one
/// that opens holes in a closed one. Both leave a capability "on some sites", and telling a person
/// only that much hides which way their policy actually points.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    /// No layer admits it anywhere.
    Unavailable,
    /// Refused everywhere except the sites some rule allows.
    SomeAllowed,
    /// Allowed everywhere except the sites some rule blocks.
    SomeBlocked,
    /// Nothing restricts it.
    Available,
}

/// One capability, answered in plain words, with the layers that narrowed it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityLine {
    /// Stable policy vocabulary for the capability.
    pub capability: Capability,
    /// Plain label naming what it does, never the policy word for it.
    pub label: &'static str,
    /// What the capability covers, for someone who has never read the policy language.
    pub covers: &'static str,
    /// Whether it is available, site-scoped, or refused.
    pub state: CapabilityState,
    /// One authored sentence stating the answer and who decided it.
    pub detail: String,
    /// The layers that narrowed this capability, in authority order. Empty when nothing did.
    pub decided_by: Vec<LayerKind>,
}

/// Why a rule is worth pointing at when it cannot do anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleNote {
    /// An earlier rule in the same policy already answers everything this one would.
    Unreachable,
    /// The organization already refuses everything this rule would allow.
    NoEffect,
}

/// One authored rule, rendered for reading rather than for parsing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuleView {
    /// Authored attribution id.
    pub id: String,
    /// Authored purpose, when the author supplied one.
    pub description: Option<String>,
    /// Host patterns this rule covers.
    pub allow: Vec<String>,
    /// Holes carved out of this rule.
    pub deny: Vec<String>,
    /// Capabilities the rule permits.
    pub allowed: Vec<Capability>,
    /// Effective mode for this rule.
    pub mode: &'static str,
    /// Set only when Ghostlight can prove the rule does nothing.
    pub note: Option<RuleNote>,
}

/// One authored setting, with the level its author chose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SettingView {
    /// Registered key.
    pub key: String,
    /// Rendered value.
    pub value: String,
    /// Authored level.
    pub level: &'static str,
}

/// One layer of the compiled policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LayerView {
    /// Which authority this layer is.
    pub kind: LayerKind,
    /// Display name for the layer: the organization's name, or a plain word for this person's own.
    pub title: String,
    /// Authored policy name.
    pub policy_name: String,
    /// Authored policy version label.
    pub version: String,
    /// Effective default mode.
    pub mode: &'static str,
    /// Ordered rules.
    pub rules: Vec<RuleView>,
    /// Authored settings.
    pub settings: Vec<SettingView>,
    /// Where the document lives, when it is a file on this machine.
    pub path: Option<String>,
    /// The exact document, so the surface never becomes the only way to read the policy.
    pub document: Option<String>,
}

/// The organization named by the policy in force.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OrganizationIdentity {
    /// Display name.
    pub name: String,
    /// The organization's own explanation.
    pub statement: Option<String>,
    /// A page the organization publishes. Rendered as text, never as a reachable link.
    pub url: Option<String>,
    /// Channels a governed person may use.
    pub contacts: Vec<ContactView>,
}

/// One organization contact channel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContactView {
    /// Channel kind.
    pub kind: String,
    /// Channel address.
    pub value: String,
    /// Optional display label.
    pub label: Option<String>,
}

/// Where this machine's user layer comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserLayerSource {
    /// No user policy exists.
    None,
    /// `GHOSTLIGHT_POLICY_FILE` names a file Ghostlight reads but does not own.
    Environment,
    /// The file Ghostlight owns and the workbench may write.
    Workbench,
}

/// What this person may do about their own layer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UserLayer {
    /// Where the layer comes from.
    pub source: UserLayerSource,
    /// Whether an organization permits authoring here at all.
    pub authoring_allowed: bool,
    /// Whether the workbench may write, which also requires owning the file.
    pub editable: bool,
    /// The path the workbench would write, when this environment has one.
    pub path: Option<String>,
    /// Why authoring is unavailable, when it is.
    pub blocked_reason: Option<String>,
}

/// The complete compiled answer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EffectiveAuthority {
    /// Which layers are in force.
    pub situation: Situation,
    /// One sentence stating the situation, naming the organization when there is one.
    pub headline: String,
    /// The organization named by the policy in force.
    pub organization: Option<OrganizationIdentity>,
    /// One line per independent capability, in canonical order.
    pub capabilities: Vec<CapabilityLine>,
    /// The layers behind those lines, in authority order.
    pub layers: Vec<LayerView>,
    /// Boundaries no policy can lift, stated in every situation including all-open.
    pub ceilings: Vec<String>,
    /// This person's own layer and what they may do about it.
    pub user_layer: UserLayer,
    /// Provenance for a signed organization layer.
    pub passport: ManagedPolicyPassport,
}

/// How the band chip should read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipTone {
    /// Nothing is applied.
    Open,
    /// A policy is applied and healthy.
    Applied,
    /// A policy is applied and its latest reload needs attention.
    Warning,
    /// Governed work is refused.
    Failing,
}

/// The policy tab's state: the entrance to the destination, and its shortest summary.
///
/// The tab is named `Policy` in the markup and stays named that. What the orchestrator supplies is
/// the state behind the name: a tone the tab can carry quietly, and the same sentence the
/// destination opens with, for whoever hovers or reads it aloud.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolicyChip {
    /// Which layers are in force.
    pub situation: Situation,
    /// The same sentence the destination opens with, naming the organization when there is one.
    pub detail: String,
    /// Which tone the tab carries.
    pub tone: ChipTone,
}

/// Author the policy tab's state.
#[must_use]
pub fn chip(situation: Situation, organization: Option<&str>, stale: bool) -> PolicyChip {
    let detail = headline(
        situation,
        organization
            .map(|name| OrganizationIdentity {
                name: name.to_owned(),
                statement: None,
                url: None,
                contacts: Vec::new(),
            })
            .as_ref(),
    );
    let tone = match situation {
        Situation::FailingClosed => ChipTone::Failing,
        Situation::AllOpen => ChipTone::Open,
        Situation::OrganizationOnly | Situation::UserOnly | Situation::Layered => {
            if stale {
                ChipTone::Warning
            } else {
                ChipTone::Applied
            }
        }
    };
    PolicyChip {
        situation,
        detail,
        tone,
    }
}

/// Everything the projection needs, gathered by the facade under one lock.
pub(super) struct Inputs<'a> {
    pub(super) organization: Option<&'a manifest::Manifest>,
    pub(super) user: Option<&'a manifest::Manifest>,
    pub(super) valid: bool,
    pub(super) sacred_hosts: Vec<String>,
    pub(super) organization_source: Option<String>,
    pub(super) organization_document: Option<String>,
    pub(super) user_source: Option<String>,
    pub(super) user_document: Option<String>,
    pub(super) user_layer_source: UserLayerSource,
    pub(super) owned_user_path: Option<String>,
    pub(super) authoring_allowed: bool,
    pub(super) passport: ManagedPolicyPassport,
}

/// Plain labels for the independent capabilities.
///
/// The person reading this has no reason to know what "action" means as a policy word, so the
/// surface never asks them to learn one (ADR-0122 Decision 6).
const fn wording(capability: Capability) -> (&'static str, &'static str) {
    match capability {
        Capability::Read => (
            "Look at pages",
            "Read page text, take screenshots, scroll, and find things on a page.",
        ),
        Capability::Action => (
            "Click and type",
            "Click, type, press keys, drag, and move through history.",
        ),
        Capability::Write => (
            "Fill in forms",
            "Enter information into forms and upload files.",
        ),
        Capability::Execute => ("Run page code", "Run JavaScript inside a page."),
    }
}

const fn layer_word(kind: LayerKind) -> &'static str {
    match kind {
        LayerKind::Organization => "your organization",
        LayerKind::User => "you",
        LayerKind::Ghostlight => "Ghostlight",
    }
}

/// Build the compiled answer.
pub(super) fn compile(inputs: &Inputs<'_>) -> EffectiveAuthority {
    let organization = inputs.organization.and_then(identity);
    let situation = situation(inputs);
    let headline = headline(situation, organization.as_ref());
    let capabilities = Capability::ALL
        .into_iter()
        .map(|capability| capability_line(capability, inputs, organization.as_ref()))
        .collect();
    let mut layers = Vec::new();
    if let Some(policy) = inputs.organization {
        layers.push(layer_view(
            LayerKind::Organization,
            organization.as_ref().map_or_else(
                || "Your organization".to_owned(),
                |named| named.name.clone(),
            ),
            policy,
            None,
            inputs.organization_source.clone(),
            inputs.organization_document.clone(),
        ));
    }
    if let Some(policy) = inputs.user {
        layers.push(layer_view(
            LayerKind::User,
            "Your rules".to_owned(),
            policy,
            inputs.organization,
            inputs.user_source.clone(),
            inputs.user_document.clone(),
        ));
    }
    EffectiveAuthority {
        situation,
        headline,
        organization,
        capabilities,
        layers,
        ceilings: ceilings(&inputs.sacred_hosts),
        user_layer: user_layer(inputs),
        passport: inputs.passport.clone(),
    }
}

fn identity(policy: &manifest::Manifest) -> Option<OrganizationIdentity> {
    let block = policy.organization.as_ref()?;
    Some(OrganizationIdentity {
        name: block.name.clone(),
        statement: block.statement.clone(),
        url: block.url.clone(),
        contacts: block
            .contacts
            .iter()
            .map(|contact| ContactView {
                kind: contact.kind.clone(),
                value: contact.value.clone(),
                label: contact.label.clone(),
            })
            .collect(),
    })
}

fn situation(inputs: &Inputs<'_>) -> Situation {
    if !inputs.valid {
        return Situation::FailingClosed;
    }
    match (inputs.organization.is_some(), inputs.user.is_some()) {
        (true, true) => Situation::Layered,
        (true, false) => Situation::OrganizationOnly,
        (false, true) => Situation::UserOnly,
        (false, false) => Situation::AllOpen,
    }
}

fn headline(situation: Situation, organization: Option<&OrganizationIdentity>) -> String {
    let named = organization.map(|identity| identity.name.as_str());
    match situation {
        Situation::AllOpen => {
            "No policy is applied. Agents can work on ordinary websites, within the boundaries \
             below that no policy can lift."
                .into()
        }
        Situation::OrganizationOnly => named.map_or_else(
            || "Your organization sets the rules on this machine.".into(),
            |name| format!("{name} sets the rules on this machine."),
        ),
        Situation::UserOnly => "You set the rules on this machine.".into(),
        Situation::Layered => named.map_or_else(
            || "Your organization sets the rules, and you have narrowed them further.".into(),
            |name| format!("{name} sets the rules, and you have narrowed them further."),
        ),
        Situation::FailingClosed => {
            "Ghostlight is refusing all governed work: a configured policy source is not valid. \
             Nothing is permitted until it can be read again."
                .into()
        }
    }
}

fn capability_line(
    capability: Capability,
    inputs: &Inputs<'_>,
    organization: Option<&OrganizationIdentity>,
) -> CapabilityLine {
    let (label, covers) = wording(capability);
    let required = CapabilitySet::one(capability);
    if !inputs.valid {
        return CapabilityLine {
            capability,
            label,
            covers,
            state: CapabilityState::Unavailable,
            detail: "Refused while a configured policy source cannot be read.".into(),
            decided_by: vec![LayerKind::Ghostlight],
        };
    }

    let layers = [
        (LayerKind::Organization, inputs.organization),
        (LayerKind::User, inputs.user),
    ];
    // Layers intersect, so the effective answer is the narrowest any of them gives, and the
    // deciders are every layer that gave something narrower than "nothing restricts this".
    let mut state = CapabilityState::Available;
    let mut decided_by = Vec::new();
    for (kind, policy) in layers {
        let Some(policy) = policy else { continue };
        let layer = reach(policy, required);
        state = state.min(layer);
        if layer != CapabilityState::Available {
            decided_by.push(kind);
        }
    }

    let named = organization.map(|identity| identity.name.clone());
    let word = |kind: LayerKind| match (kind, named.as_deref()) {
        (LayerKind::Organization, Some(name)) => name.to_owned(),
        (kind, _) => layer_word(kind).to_owned(),
    };
    let who = decided_by
        .iter()
        .map(|kind| word(*kind))
        .collect::<Vec<_>>()
        .join(" and ");

    let detail = match state {
        CapabilityState::Available => {
            "Available on ordinary websites. Nothing narrows it.".to_owned()
        }
        // Polarity is the point of these two sentences. A rule that carves holes out of an open
        // baseline and a rule that opens holes in a closed one both leave a capability available
        // "on some sites", and a person cannot act on that without knowing which way it points.
        CapabilityState::SomeBlocked => {
            format!("Available everywhere except the sites {who} blocked.")
        }
        CapabilityState::SomeAllowed => {
            format!("Refused everywhere except the sites {who} allowed.")
        }
        CapabilityState::Unavailable => {
            let kind = decided_by.first().copied().unwrap_or(LayerKind::Ghostlight);
            // "you does not allow it" is what a template gets you. Whoever refused is named in
            // their own grammar, because a person reading why they were stopped deserves a
            // sentence rather than a substitution.
            let refusal = match kind {
                LayerKind::User => "You do not allow it anywhere.".to_owned(),
                LayerKind::Ghostlight => "Ghostlight does not allow it anywhere.".to_owned(),
                LayerKind::Organization => format!(
                    "{} does not allow it anywhere.",
                    named.as_deref().unwrap_or("Your organization")
                ),
            };
            format!("Not available. {refusal}")
        }
    };

    CapabilityLine {
        capability,
        label,
        covers,
        state,
        detail,
        decided_by,
    }
}

/// How widely one layer admits a capability, and which way its rules point.
fn reach(policy: &manifest::Manifest, required: CapabilitySet) -> CapabilityState {
    let mut widest = CapabilityState::Unavailable;
    for grant in &policy.grants {
        if !required.is_subset_of(grant.allowed_set()) {
            continue;
        }
        let universal = grant.hosts.allow.iter().any(|pattern| pattern == "*");
        let state = if universal && grant.hosts.deny.is_empty() {
            CapabilityState::Available
        } else if universal {
            CapabilityState::SomeBlocked
        } else if !any_allow_survives_deny(grant) {
            // Every allow pattern in this grant is fully canceled by a deny in the same grant,
            // so it admits nothing anywhere -- this must read the same as an empty allow list,
            // not as "some sites allowed". See any_allow_survives_deny for why exact pattern
            // equality is the right (and only) test.
            continue;
        } else {
            CapabilityState::SomeAllowed
        };
        widest = widest.max(state);
    }
    widest
}

/// Whether at least one of this grant's allow patterns is not fully canceled by its own deny
/// list, i.e. whether the grant can admit any host at all.
///
/// A deny pattern only ever fully cancels an allow pattern in the *same* grant when the two are
/// the identical pattern text. This falls directly out of `pattern_specificity`'s tie-break: for
/// a deny to beat an allow at *every* host the allow pattern matches, it must be at least as
/// specific everywhere the allow pattern applies. A broader deny (`*.example.com` denying
/// `*.a.example.com`) loses the tie at every shared host, because the narrower allow pattern is
/// *more* specific there, not less; a narrower deny (one exact host under a wildcard allow)
/// only cancels that one host, leaving the rest of the allow pattern's hosts still admitted. The
/// only pattern that is simultaneously broad enough to cover every host the allow pattern
/// matches and specific enough to win the tie at all of them is that same pattern, verbatim.
fn any_allow_survives_deny(grant: &manifest::Grant) -> bool {
    grant.hosts.allow.iter().any(|allow| {
        !grant
            .hosts
            .deny
            .iter()
            .any(|deny| deny.eq_ignore_ascii_case(allow))
    })
}

fn layer_view(
    kind: LayerKind,
    title: String,
    policy: &manifest::Manifest,
    ceiling: Option<&manifest::Manifest>,
    path: Option<String>,
    document: Option<String>,
) -> LayerView {
    let default_mode = policy.mode.unwrap_or_default();
    let rules = policy
        .grants
        .iter()
        .enumerate()
        .map(|(index, grant)| RuleView {
            id: grant.id.clone(),
            description: grant.description.clone(),
            allow: grant.hosts.allow.clone(),
            deny: grant.hosts.deny.clone(),
            allowed: grant.allowed.clone(),
            mode: grant.mode.unwrap_or(default_mode).as_str(),
            note: note(index, grant, &policy.grants, ceiling),
        })
        .collect();
    LayerView {
        kind,
        title,
        policy_name: policy.name.clone(),
        version: policy.version.clone(),
        mode: default_mode.as_str(),
        rules,
        settings: policy
            .config
            .iter()
            .map(|entry| SettingView {
                key: entry.key.clone(),
                value: entry.value.to_string(),
                level: entry.level.as_str(),
            })
            .collect(),
        path,
        document,
    }
}

/// Mark a rule only when Ghostlight can prove it does nothing.
///
/// Both checks are deliberately conservative. A rule carrying a note is certainly inert; a rule
/// without one may still be, and saying so on a guess would be worse than staying quiet.
fn note(
    index: usize,
    grant: &manifest::Grant,
    siblings: &[manifest::Grant],
    ceiling: Option<&manifest::Manifest>,
) -> Option<RuleNote> {
    if siblings[..index]
        .iter()
        .any(|earlier| covers_grant(earlier, grant))
    {
        return Some(RuleNote::Unreachable);
    }
    let ceiling = ceiling?;
    let admitted = grant.hosts.allow.iter().any(|pattern| {
        ceiling
            .grants
            .iter()
            .any(|above| covers_pattern_with(above, grant.allowed_set(), pattern))
    });
    (!admitted && !grant.hosts.allow.is_empty()).then_some(RuleNote::NoEffect)
}

fn covers_grant(earlier: &manifest::Grant, later: &manifest::Grant) -> bool {
    if !earlier.hosts.deny.is_empty() {
        return false;
    }
    if !later.allowed_set().is_subset_of(earlier.allowed_set()) {
        return false;
    }
    !later.hosts.allow.is_empty()
        && later.hosts.allow.iter().all(|pattern| {
            earlier
                .hosts
                .allow
                .iter()
                .any(|broad| pattern_covers(broad, pattern))
        })
}

fn covers_pattern_with(above: &manifest::Grant, required: CapabilitySet, pattern: &str) -> bool {
    required.is_subset_of(above.allowed_set())
        && above
            .hosts
            .allow
            .iter()
            .any(|broad| pattern_covers(broad, pattern))
}

/// Whether every host matched by `narrow` is also matched by `broad`.
///
/// Ghostlight's suffix wildcard covers subdomains only, so `*.example.com` does not cover the bare
/// `example.com`. Getting that backwards would mark a live rule inert.
fn pattern_covers(broad: &str, narrow: &str) -> bool {
    if broad == "*" {
        return true;
    }
    if broad.eq_ignore_ascii_case(narrow) {
        return true;
    }
    let Some(suffix) = broad.strip_prefix("*.") else {
        return false;
    };
    let candidate = narrow.strip_prefix("*.").unwrap_or(narrow);
    candidate.len() > suffix.len()
        && candidate
            .to_ascii_lowercase()
            .ends_with(&format!(".{}", suffix.to_ascii_lowercase()))
}

fn ceilings(sacred: &[String]) -> Vec<String> {
    let mut lines = vec![
        "Anything that is not an ordinary http or https address.".to_owned(),
        "localhost and any name ending in .localhost.".to_owned(),
        "Loopback and link-local addresses.".to_owned(),
    ];
    for host in sacred {
        lines.push(format!("{host}, marked never-touch by policy."));
    }
    lines
}

fn user_layer(inputs: &Inputs<'_>) -> UserLayer {
    let owned = inputs.user_layer_source != UserLayerSource::Environment;
    let editable = inputs.authoring_allowed && owned && inputs.owned_user_path.is_some();
    let blocked_reason = if !inputs.authoring_allowed {
        Some(organization_refusal(inputs))
    } else if !owned {
        Some(
            "An environment variable points Ghostlight at a policy file it does not own, so this \
             window shows it without changing it."
                .to_owned(),
        )
    } else if inputs.owned_user_path.is_none() {
        Some("This environment names no per-user state directory to keep a policy in.".to_owned())
    } else {
        None
    };
    UserLayer {
        source: inputs.user_layer_source,
        authoring_allowed: inputs.authoring_allowed,
        editable,
        path: inputs.owned_user_path.clone(),
        blocked_reason,
    }
}

fn organization_refusal(inputs: &Inputs<'_>) -> String {
    let identity = inputs.organization.and_then(identity);
    let name = identity.as_ref().map_or_else(
        || "Your organization".to_owned(),
        |named| named.name.clone(),
    );
    identity
        .as_ref()
        .and_then(|named| named.statement.clone())
        .map_or_else(
            || format!("{name} does not allow rules to be set on this machine."),
            |statement| {
                format!("{name} does not allow rules to be set on this machine. {statement}")
            },
        )
}

#[cfg(test)]
mod tests {
    use super::{
        compile, pattern_covers, CapabilityState, Inputs, LayerKind, RuleNote, Situation,
        UserLayerSource,
    };
    use crate::governance::{manifest, ManagedPolicyPassport};

    fn policy(text: &str) -> manifest::Manifest {
        manifest::parse(text, "test").expect("test policy parses")
    }

    fn passport() -> ManagedPolicyPassport {
        ManagedPolicyPassport {
            configured: false,
            verified: false,
            freshness: crate::governance::ManagedPolicyFreshness::NotConfigured,
            sequence: None,
            organization: None,
            rationale: None,
            contacts: Vec::new(),
            source_class: crate::governance::ManagedPolicySource::None,
            last_success_ms: None,
            last_attempt_ms: None,
        }
    }

    fn inputs<'a>(
        organization: Option<&'a manifest::Manifest>,
        user: Option<&'a manifest::Manifest>,
    ) -> Inputs<'a> {
        Inputs {
            organization,
            user,
            valid: true,
            sacred_hosts: Vec::new(),
            organization_source: None,
            organization_document: None,
            user_source: None,
            user_document: None,
            user_layer_source: if user.is_some() {
                UserLayerSource::Workbench
            } else {
                UserLayerSource::None
            },
            owned_user_path: Some("state/user-policy.json".into()),
            authoring_allowed: true,
            passport: passport(),
        }
    }

    #[test]
    fn all_open_answers_completely_and_still_states_the_permanent_boundaries() {
        let view = compile(&inputs(None, None));
        assert_eq!(view.situation, Situation::AllOpen);
        assert!(view.headline.starts_with("No policy is applied."));
        assert_eq!(view.capabilities.len(), 4);
        assert!(view
            .capabilities
            .iter()
            .all(|line| line.state == CapabilityState::Available && line.decided_by.is_empty()));
        assert_eq!(view.ceilings.len(), 3);
        assert!(view.layers.is_empty());
        assert!(view.user_layer.editable);
    }

    #[test]
    fn every_narrowed_capability_names_the_layer_that_narrowed_it() {
        let organization = policy(
            r#"{"schema":3,"name":"org","version":"1","organization":{"name":"Example Organization"},"grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]}"#,
        );
        let view = compile(&inputs(Some(&organization), None));
        assert_eq!(view.situation, Situation::OrganizationOnly);
        assert_eq!(
            view.headline,
            "Example Organization sets the rules on this machine."
        );

        let read = &view.capabilities[0];
        assert_eq!(read.state, CapabilityState::SomeAllowed);
        assert_eq!(read.decided_by, vec![LayerKind::Organization]);
        assert_eq!(
            read.detail,
            "Refused everywhere except the sites Example Organization allowed."
        );

        let execute = &view.capabilities[3];
        assert_eq!(execute.state, CapabilityState::Unavailable);
        assert!(execute.detail.contains("does not allow it"));
    }

    #[test]
    fn polarity_is_stated_rather_than_flattened_into_some_sites() {
        // An open baseline with holes cut in it, and a closed one with holes opened. Both leave a
        // capability available on "some sites" and they are opposite situations.
        let carved = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"everywhere-but","hosts":{"allow":["*"],"deny":["intranet.example"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&carved)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeBlocked);
        assert_eq!(
            view.capabilities[0].detail,
            "Available everywhere except the sites you blocked."
        );

        let opened = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"just-here","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&opened)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeAllowed);
        assert_eq!(
            view.capabilities[0].detail,
            "Refused everywhere except the sites you allowed."
        );

        // The narrower of the two layers is the answer, and both are named as deciders.
        let view = compile(&inputs(Some(&carved), Some(&opened)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeAllowed);
        assert_eq!(
            view.capabilities[0].decided_by,
            vec![LayerKind::Organization, LayerKind::User]
        );
    }

    #[test]
    fn a_grant_whose_own_deny_cancels_its_own_allow_admits_nothing() {
        // The real decision engine's tie-break (evaluate_host: deny wins a specificity tie)
        // means a deny pattern identical to its own grant's allow pattern refuses every host that
        // grant would otherwise have admitted. The compiled view must say so -- "some sites
        // allowed" here would tell a person a site is reachable that the real decision path
        // refuses on every single call.
        let self_canceling = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"pointless","hosts":{"allow":["*.example.com"],"deny":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&self_canceling)));
        assert_eq!(view.capabilities[0].state, CapabilityState::Unavailable);

        // A deny that only removes ONE host under a wildcard allow does not cancel the rest:
        // every other host under that suffix is still genuinely admitted.
        let partly_canceling = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"minus-one","hosts":{"allow":["*.example.com"],"deny":["admin.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&partly_canceling)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeAllowed);

        // A broader deny loses the specificity tie at every host the narrower allow matches --
        // covering the same hosts is not the same as outranking them.
        let broader_deny_never_wins = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"narrow-allow","hosts":{"allow":["*.a.example.com"],"deny":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&broader_deny_never_wins)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeAllowed);
    }

    #[test]
    fn whoever_refused_is_named_in_their_own_grammar() {
        let mine = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&mine)));
        let execute = &view.capabilities[3];
        assert_eq!(execute.state, CapabilityState::Unavailable);
        assert_eq!(
            execute.detail,
            "Not available. You do not allow it anywhere."
        );
        assert_eq!(execute.decided_by, vec![LayerKind::User]);

        let anonymous = policy(
            r#"{"schema":3,"name":"org","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(Some(&anonymous), None));
        assert_eq!(
            view.capabilities[3].detail,
            "Not available. Your organization does not allow it anywhere."
        );
    }

    #[test]
    fn failing_closed_says_so_on_every_line() {
        let mut broken = inputs(None, None);
        broken.valid = false;
        let view = compile(&broken);
        assert_eq!(view.situation, Situation::FailingClosed);
        assert!(view
            .capabilities
            .iter()
            .all(|line| line.state == CapabilityState::Unavailable
                && line.decided_by == vec![LayerKind::Ghostlight]));
    }

    #[test]
    fn inert_rules_are_marked_only_when_they_can_be_proven_inert() {
        let organization = policy(
            r#"{"schema":3,"name":"org","version":"1","grants":[{"id":"approved","hosts":{"allow":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let user = policy(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"wide","hosts":{"allow":["*.example.com"]},"allowed":["read"]},{"id":"shadowed","hosts":{"allow":["a.example.com"]},"allowed":["read"]},{"id":"elsewhere","hosts":{"allow":["other.test"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(Some(&organization), Some(&user)));
        assert_eq!(view.situation, Situation::Layered);
        let rules = &view.layers[1].rules;
        assert_eq!(rules[0].note, None);
        assert_eq!(rules[1].note, Some(RuleNote::Unreachable));
        assert_eq!(rules[2].note, Some(RuleNote::NoEffect));
    }

    #[test]
    fn an_organization_refusal_is_explained_in_its_own_words() {
        let organization = policy(
            r#"{"schema":3,"name":"org","version":"1","organization":{"name":"Example Organization","statement":"Ask the service desk for an exception."},"grants":[{"id":"work","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}],"config":[{"key":"policy.user.enabled","value":false,"level":"mandatory"}]}"#,
        );
        let mut refused = inputs(Some(&organization), None);
        refused.authoring_allowed = false;
        let view = compile(&refused);
        assert!(!view.user_layer.editable);
        let reason = view.user_layer.blocked_reason.expect("a stated reason");
        assert!(reason.contains("Example Organization"));
        assert!(reason.contains("Ask the service desk for an exception."));
        // A wide-open organization grant narrows nothing, so no capability line invents a decider.
        assert!(view
            .capabilities
            .iter()
            .all(|line| line.state == CapabilityState::Available));
    }

    #[test]
    fn suffix_wildcards_cover_subdomains_and_never_the_bare_host() {
        assert!(pattern_covers("*", "anything.test"));
        assert!(pattern_covers("*.example.com", "a.example.com"));
        assert!(pattern_covers("*.example.com", "*.a.example.com"));
        assert!(!pattern_covers("*.example.com", "example.com"));
        assert!(!pattern_covers("example.com", "a.example.com"));
        assert!(pattern_covers("example.com", "EXAMPLE.com"));
    }
}
