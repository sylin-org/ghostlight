// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The compiled policy a person reads.
//!
//! Ghostlight decides authority per invocation from an immutable snapshot. That is the right shape
//! for enforcement and the wrong shape for a person, who arrives with one question -- what can the
//! agent do right now, and who decided that -- and cannot get it from a stack of layers.
//!
//! This module answers first and shows the derivation second, which is the posture every mature
//! layered-policy surface settled on (ADR-0122 Decision 2). Every line it produces names the layer
//! that decided it. The words come from [`crate::language::policy`], so the window renders
//! sentences rather than inventing them from booleans.

use std::sync::Arc;

use serde::Serialize;

use super::{
    host_pattern, manifest, AuthoritySnapshot, AuthorityTier, Capability, CapabilitySet,
    ManagedPolicyFreshness, ManagedPolicyPassport,
};
use crate::language::channel_switch::ChannelsOff;
use crate::language::outcome::PolicyAuthor;
use crate::language::policy;

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

/// The effective browser-startup preference and the organization ceiling above it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserStartupView {
    /// The effective closed value.
    pub value: manifest::BrowserStartup,
    /// The authored layer that selected the value, or none for the platform default.
    pub decided_by: Option<LayerKind>,
    /// The organization's authored ceiling, when one exists.
    pub organization_ceiling: Option<manifest::BrowserStartup>,
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
///
/// A publication resolves it once, and every surface that names the organization reads that one
/// answer: the snapshot, the chip, the channel switches, the compiled view and the passport.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct OrganizationIdentity {
    /// Display name, when either carrier names the organization.
    pub name: Option<String>,
    /// The organization's own explanation.
    pub statement: Option<String>,
    /// A page the organization publishes. Rendered as text, never as a reachable link.
    pub url: Option<String>,
    /// Channels a governed person may use.
    pub contacts: Vec<ContactView>,
}

impl OrganizationIdentity {
    /// The one organization a publication names, resolved with ADR-0122 Decision 3's precedence:
    /// what the signed presentation says wins, and the manifest's `organization` block fills what
    /// it lacks. A contact list is taken whole from whichever carrier wins. `None` when neither
    /// carrier says anything.
    pub(super) fn resolve(
        signed: Option<Self>,
        block: Option<&manifest::OrganizationBlock>,
    ) -> Option<Self> {
        let signed = signed.unwrap_or_default();
        let authored = block.map_or_else(Self::default, |block| Self {
            name: Some(block.name.clone()),
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
        });
        let resolved = Self {
            name: signed.name.or(authored.name),
            statement: signed.statement.or(authored.statement),
            url: signed.url.or(authored.url),
            contacts: if signed.contacts.is_empty() {
                authored.contacts
            } else {
                signed.contacts
            },
        };
        (resolved != Self::default()).then_some(resolved)
    }
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
    /// The id of the authority this answer was compiled from, the one the audit line records.
    ///
    /// It is a digest of the layers' documents, so a watcher could test guesses about a withheld
    /// list against it. It is therefore not part of the serialized policy part, which a watcher
    /// is sent. The window receives it only as the tag on the policy part's change hint, and the
    /// watcher projection removes that tag.
    #[serde(skip)]
    pub id: String,
    /// Effective requirement for saving audit receipts, as the snapshot enforces it.
    pub audit: super::audit::AuditPolicy,
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
    /// Whether missing-browser recovery may start a browser on this platform.
    pub browser_startup: BrowserStartupView,
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

impl PolicyChip {
    /// The policy tab's state, read from the compiled answer: its situation and headline, and a
    /// warning tone while the organization's policy is the last known good one.
    #[must_use]
    pub fn of(authority: &EffectiveAuthority) -> Self {
        let tone = match authority.situation {
            Situation::FailingClosed => ChipTone::Failing,
            Situation::AllOpen => ChipTone::Open,
            Situation::OrganizationOnly | Situation::UserOnly | Situation::Layered => {
                if authority.passport.freshness == ManagedPolicyFreshness::LastKnownGood {
                    ChipTone::Warning
                } else {
                    ChipTone::Applied
                }
            }
        };
        Self {
            situation: authority.situation,
            detail: authority.headline.clone(),
            tone,
        }
    }
}

/// Everything the projection needs, gathered by the facade under one lock.
pub(super) struct Inputs<'a> {
    /// The snapshot enforcement reads, which answers where each capability may be used.
    pub(super) snapshot: Arc<AuthoritySnapshot>,
    pub(super) organization: Option<&'a manifest::Manifest>,
    pub(super) user: Option<&'a manifest::Manifest>,
    pub(super) valid: bool,
    /// The channel switches the same publication's snapshot enforces.
    pub(super) channels_off: ChannelsOff<'a>,
    pub(super) sacred_hosts: Vec<String>,
    pub(super) organization_source: Option<String>,
    pub(super) organization_document: Option<String>,
    pub(super) user_source: Option<String>,
    pub(super) user_document: Option<String>,
    pub(super) user_layer_source: UserLayerSource,
    /// The path this window may write the user policy to, or why it may not: the facade's own
    /// answer, so the window and a refused write say the same thing.
    pub(super) authoring: Result<String, super::AuthoringError>,
    pub(super) windows: bool,
    pub(super) passport: ManagedPolicyPassport,
}

/// Build the compiled answer.
pub(super) fn compile(inputs: &Inputs<'_>) -> EffectiveAuthority {
    let organization = inputs.snapshot.organization().cloned();
    let situation = situation(inputs);
    let headline = headline(situation, organization.as_ref(), inputs.channels_off);
    let capabilities = Capability::ALL
        .into_iter()
        .map(|capability| capability_line(capability, inputs, organization.as_ref()))
        .collect();
    let mut layers = Vec::new();
    if let Some(policy) = inputs.organization {
        layers.push(layer_view(
            LayerKind::Organization,
            policy::organization_title(
                organization
                    .as_ref()
                    .and_then(|named| named.name.as_deref()),
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
            policy::USER_LAYER_TITLE.to_owned(),
            policy,
            Some(&inputs.snapshot),
            inputs.user_source.clone(),
            inputs.user_document.clone(),
        ));
    }
    EffectiveAuthority {
        id: inputs.snapshot.id().to_owned(),
        audit: inputs.snapshot.audit_policy().clone(),
        situation,
        headline,
        organization,
        capabilities,
        layers,
        ceilings: policy::ceilings(&inputs.sacred_hosts),
        user_layer: user_layer(inputs),
        browser_startup: browser_startup(inputs),
        passport: inputs.passport.clone(),
    }
}

/// Resolve the closed startup preference monotonically across platform, organization, and user.
pub(super) fn browser_startup(inputs: &Inputs<'_>) -> BrowserStartupView {
    let platform_default = manifest::BrowserStartup::default_for_platform(inputs.windows);
    let organization = inputs
        .organization
        .and_then(manifest::Manifest::browser_startup);
    let user = inputs.user.and_then(manifest::Manifest::browser_startup);
    let (value, decided_by) = match (organization, user) {
        (Some(manifest::BrowserStartup::Manual), _) => (
            manifest::BrowserStartup::Manual,
            Some(LayerKind::Organization),
        ),
        (Some(manifest::BrowserStartup::OnDemand), Some(user)) => (user, Some(LayerKind::User)),
        (Some(value), None) => (value, Some(LayerKind::Organization)),
        (None, Some(value)) => (value, Some(LayerKind::User)),
        (None, None) => (platform_default, None),
    };
    BrowserStartupView {
        value,
        decided_by,
        organization_ceiling: organization,
    }
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

/// The situation's sentence, followed by the switched-off channels a valid policy turns off.
fn headline(
    situation: Situation,
    organization: Option<&OrganizationIdentity>,
    channels_off: ChannelsOff<'_>,
) -> String {
    let said = policy::situation(
        situation,
        organization.and_then(|identity| identity.name.as_deref()),
    );
    match channels_off.policy_line() {
        Some(line) if situation != Situation::FailingClosed => format!("{said} {line}"),
        _ => said,
    }
}

fn capability_line(
    capability: Capability,
    inputs: &Inputs<'_>,
    organization: Option<&OrganizationIdentity>,
) -> CapabilityLine {
    let (label, covers) = policy::capability_wording(capability);
    let required = CapabilitySet::one(capability);
    if !inputs.valid {
        return CapabilityLine {
            capability,
            label,
            covers,
            state: CapabilityState::Unavailable,
            detail: policy::REFUSED_WHILE_INVALID.into(),
            decided_by: vec![LayerKind::Ghostlight],
        };
    }

    // The snapshot answers across every layer at once, so two layers that each allow the
    // capability on different sites read as allowing it nowhere, as enforcement decides.
    let reach = inputs.snapshot.reach(required);
    let state = reach.state;
    let mut decided_by: Vec<LayerKind> = reach
        .narrowed_by
        .iter()
        .map(|tier| layer_kind(*tier))
        .collect();

    let named = organization.and_then(|identity| identity.name.as_deref());
    let who = policy::who(&decided_by, named);

    let switched = inputs.channels_off;
    if switched.all() {
        // Every channel is off, so no request reaches this capability whatever the rules allow.
        return CapabilityLine {
            capability,
            label,
            covers,
            state: CapabilityState::Unavailable,
            detail: policy::not_available(&switched.said().unwrap_or_default()),
            decided_by: with_switches(decided_by, switched),
        };
    }

    let mut detail = match state {
        CapabilityState::Available if switched.any() => policy::AVAILABLE.to_owned(),
        CapabilityState::Available => policy::AVAILABLE_UNNARROWED.to_owned(),
        // Polarity is the point of these two sentences. A rule that carves holes out of an open
        // baseline and a rule that opens holes in a closed one both leave a capability available
        // "on some sites", and a person cannot act on that without knowing which way it points.
        CapabilityState::SomeBlocked => policy::some_blocked(&who),
        CapabilityState::SomeAllowed => policy::some_allowed(&who),
        CapabilityState::Unavailable if reach.disjoint() => policy::disjoint(&who),
        CapabilityState::Unavailable => match (reach.nowhere.first(), &reach.never_touched) {
            // Every layer allows it somewhere, but only on never-touch hosts, so the never-touch
            // list is what refuses it: its author is named and credited.
            (None, Some(never)) => {
                let kinds = |tiers: &[AuthorityTier]| {
                    tiers
                        .iter()
                        .map(|tier| layer_kind(*tier))
                        .collect::<Vec<_>>()
                };
                let marked_by = kinds(&never.marked_by);
                for kind in &marked_by {
                    if !decided_by.contains(kind) {
                        decided_by.push(*kind);
                    }
                }
                decided_by.sort_by_key(|kind| *kind != LayerKind::Organization);
                policy::only_never_touch(&kinds(&never.allowed_by), &marked_by, named)
            }
            (nowhere, _) => policy::allowed_nowhere(
                nowhere.map_or(LayerKind::Ghostlight, |tier| layer_kind(*tier)),
                named,
            ),
        },
    };
    // A switch narrows who may do it rather than where: said on every line something can still
    // use, and its layer named among the deciders.
    let usable = state != CapabilityState::Unavailable;
    if let Some(line) = switched.capability_line().filter(|_| usable) {
        detail = format!("{detail} {line}");
        decided_by = with_switches(decided_by, switched);
    }

    CapabilityLine {
        capability,
        label,
        covers,
        state,
        detail,
        decided_by,
    }
}

/// `decided_by` with every layer that turned a channel switch off, in authority order.
fn with_switches(mut decided_by: Vec<LayerKind>, switched: ChannelsOff<'_>) -> Vec<LayerKind> {
    for (kind, author) in [
        (LayerKind::Organization, PolicyAuthor::Organization),
        (LayerKind::User, PolicyAuthor::User),
    ] {
        if [switched.mcp, switched.cli].contains(&Some(author)) && !decided_by.contains(&kind) {
            decided_by.push(kind);
        }
    }
    decided_by.sort_by_key(|kind| *kind != LayerKind::Organization);
    decided_by
}

/// The layer kind a person reads for one authority tier.
pub(super) const fn layer_kind(tier: AuthorityTier) -> LayerKind {
    match tier {
        AuthorityTier::Managed => LayerKind::Organization,
        AuthorityTier::User => LayerKind::User,
    }
}

fn layer_view(
    kind: LayerKind,
    title: String,
    policy: &manifest::Manifest,
    ceiling: Option<&AuthoritySnapshot>,
    path: Option<String>,
    document: Option<String>,
) -> LayerView {
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
            note: note(index, grant, &policy.grants, ceiling),
        })
        .collect();
    LayerView {
        kind,
        title,
        policy_name: policy.name.clone(),
        version: policy.version.clone(),
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
/// A rule carrying a note is certainly inert; a rule without one may still be, and saying so on a
/// guess would be worse than staying quiet. The organization's refusal is read from `ceiling`, the
/// snapshot enforcement reads, over every host the rule allows.
fn note(
    index: usize,
    grant: &manifest::Grant,
    siblings: &[manifest::Grant],
    ceiling: Option<&AuthoritySnapshot>,
) -> Option<RuleNote> {
    if siblings[..index]
        .iter()
        .any(|earlier| covers_grant(earlier, grant))
    {
        return Some(RuleNote::Unreachable);
    }
    let admitted =
        ceiling?.layer_admits_within(AuthorityTier::Managed, grant.allowed_set(), &grant.hosts)?;
    (!admitted && !grant.hosts.allow.is_empty()).then_some(RuleNote::NoEffect)
}

fn covers_grant(earlier: &manifest::Grant, later: &manifest::Grant) -> bool {
    if !earlier.hosts.deny.is_empty() {
        return false;
    }
    if !later.allowed_set().is_subset_of(earlier.allowed_set()) {
        return false;
    }
    // A suffix wildcard covers the hosts below its suffix only, so `*.example.com` does not cover
    // the bare `example.com`. Getting that backwards would mark a live rule inert.
    let broader = host_pattern::parse_all(&earlier.hosts.allow);
    !later.hosts.allow.is_empty()
        && host_pattern::parse_all(&later.hosts.allow)
            .iter()
            .all(|narrow| broader.iter().any(|broad| broad.covers(narrow)))
}

fn user_layer(inputs: &Inputs<'_>) -> UserLayer {
    let (path, blocked_reason) = match &inputs.authoring {
        Ok(path) => (Some(path.clone()), None),
        Err(why) => (None, Some(why.to_string())),
    };
    UserLayer {
        source: inputs.user_layer_source,
        editable: path.is_some(),
        path,
        blocked_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compile, CapabilityState, ChipTone, Inputs, LayerKind, OrganizationIdentity, PolicyChip,
        RuleNote, Situation, UserLayerSource,
    };
    use std::sync::Arc;

    use crate::governance::{
        assemble, manifest, AuthoringError, AuthoritySnapshot, AuthorityTier, Capability,
        CapabilitySet, ManagedPolicyPassport, OwnOrigin,
    };
    use crate::language::capability_map;
    use crate::language::catalog_for;
    use crate::language::channel_switch::ChannelsOff;
    use crate::language::outcome::{PolicyAuthor, OWN_CONTROL_PAGE_CEILING};
    use crate::language::policy;

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

    /// The snapshot enforcement would read for these layers.
    fn snapshot(
        organization: Option<&manifest::Manifest>,
        user: Option<&manifest::Manifest>,
    ) -> Arc<AuthoritySnapshot> {
        Arc::new(assemble(
            [
                (organization.cloned(), AuthorityTier::Managed),
                (user.cloned(), AuthorityTier::User),
            ],
            OrganizationIdentity::resolve(
                None,
                organization.and_then(|policy| policy.organization.as_ref()),
            ),
            None,
            true,
            &OwnOrigin::default(),
        ))
    }

    fn inputs<'a>(
        organization: Option<&'a manifest::Manifest>,
        user: Option<&'a manifest::Manifest>,
    ) -> Inputs<'a> {
        let snapshot = snapshot(organization, user);
        Inputs {
            sacred_hosts: snapshot.sacred_hosts(),
            snapshot,
            organization,
            user,
            valid: true,
            channels_off: ChannelsOff::default(),
            organization_source: None,
            organization_document: None,
            user_source: None,
            user_document: None,
            user_layer_source: if user.is_some() {
                UserLayerSource::Workbench
            } else {
                UserLayerSource::None
            },
            authoring: Ok("state/user-policy.json".into()),
            windows: cfg!(windows),
            passport: passport(),
        }
    }

    #[test]
    fn all_open_answers_completely_and_still_states_the_permanent_boundaries() {
        let view = compile(&inputs(None, None));
        assert_eq!(view.situation, Situation::AllOpen);
        assert_eq!(
            view.headline,
            "No rules apply. Agents may work on ordinary websites, within the limits below."
        );
        assert_eq!(view.capabilities.len(), 4);
        assert!(view
            .capabilities
            .iter()
            .all(|line| line.state == CapabilityState::Available && line.decided_by.is_empty()));
        assert_eq!(
            view.ceilings,
            [
                "Anything that is not an ordinary http or https address.",
                OWN_CONTROL_PAGE_CEILING
            ]
        );
        assert!(view.layers.is_empty());
        assert!(view.user_layer.editable);

        let mut restricted = inputs(None, None);
        restricted.sacred_hosts.push("localhost".into());
        assert_eq!(
            compile(&restricted).ceilings,
            [
                "Anything that is not an ordinary http or https address.",
                "localhost, marked never-touch by policy.",
                OWN_CONTROL_PAGE_CEILING
            ]
        );
    }

    #[test]
    fn one_channel_off_is_named_in_the_headline_and_on_every_usable_line() {
        let mine = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write"]}]}"#,
        );
        let mut switched = inputs(None, Some(&mine));
        switched.channels_off = ChannelsOff {
            mcp: Some(PolicyAuthor::User),
            ..ChannelsOff::default()
        };
        let view = compile(&switched);
        assert_eq!(
            view.headline,
            "You set the rules on this machine. MCP agents are turned off. Command-line work may continue."
        );
        let read = &view.capabilities[0];
        assert_eq!(read.state, CapabilityState::Available);
        assert_eq!(
            read.detail,
            "Available on ordinary websites. Only command-line work may use it: MCP agents are turned off."
        );
        assert_eq!(read.decided_by, [LayerKind::User]);
        // A capability nothing may use says only why, with no promise about who may use it.
        let execute = &view.capabilities[3];
        assert_eq!(execute.state, CapabilityState::Unavailable);
        assert_eq!(
            execute.detail,
            "Not available. You do not allow it anywhere."
        );
    }

    #[test]
    fn both_channels_off_leave_nothing_available_and_say_whose_policy_did_it() {
        let organization = policy(
            r#"{"schema":4,"name":"org","version":"1","organization":{"name":"Northwind"},"grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
        );
        let mut switched = inputs(Some(&organization), None);
        switched.channels_off = ChannelsOff {
            mcp: Some(PolicyAuthor::Organization),
            cli: Some(PolicyAuthor::Organization),
            organization: Some("Northwind"),
        };
        let view = compile(&switched);
        assert_eq!(
            view.headline,
            "Northwind sets the rules on this machine. MCP agents and the command line are turned off, so no agent may work."
        );
        for line in &view.capabilities {
            assert_eq!(
                line.state,
                CapabilityState::Unavailable,
                "{:?}",
                line.capability
            );
            assert_eq!(
                line.detail,
                "Not available. Northwind's policy turns off work from MCP agents and the command line."
            );
            assert_eq!(line.decided_by, [LayerKind::Organization]);
        }
        // An invalid policy says only that it is invalid.
        switched.valid = false;
        assert!(!compile(&switched).headline.contains("turned off"));
    }

    #[test]
    fn every_narrowed_capability_names_the_layer_that_narrowed_it() {
        let organization = policy(
            r#"{"schema":4,"name":"org","version":"1","organization":{"name":"Example Organization"},"grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]}"#,
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
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"everywhere-but","hosts":{"allow":["*"],"deny":["intranet.example"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&carved)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeBlocked);
        assert_eq!(
            view.capabilities[0].detail,
            "Available everywhere except the sites you blocked."
        );

        let opened = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"just-here","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
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
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"pointless","hosts":{"allow":["*.example.com"],"deny":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&self_canceling)));
        assert_eq!(view.capabilities[0].state, CapabilityState::Unavailable);

        // A deny that only removes ONE host under a wildcard allow does not cancel the rest:
        // every other host under that suffix is still genuinely admitted.
        let partly_canceling = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"minus-one","hosts":{"allow":["*.example.com"],"deny":["admin.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&partly_canceling)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeAllowed);

        // A broader deny loses the specificity tie at every host the narrower allow matches --
        // covering the same hosts is not the same as outranking them.
        let broader_deny_never_wins = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"narrow-allow","hosts":{"allow":["*.a.example.com"],"deny":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(None, Some(&broader_deny_never_wins)));
        assert_eq!(view.capabilities[0].state, CapabilityState::SomeAllowed);
    }

    #[test]
    fn whoever_refused_is_named_in_their_own_grammar() {
        let mine = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
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
            r#"{"schema":4,"name":"org","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
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
            r#"{"schema":4,"name":"org","version":"1","grants":[{"id":"approved","hosts":{"allow":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let user = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"wide","hosts":{"allow":["*.example.com"]},"allowed":["read"]},{"id":"shadowed","hosts":{"allow":["a.example.com"]},"allowed":["read"]},{"id":"elsewhere","hosts":{"allow":["other.test"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(Some(&organization), Some(&user)));
        assert_eq!(view.situation, Situation::Layered);
        let rules = &view.layers[1].rules;
        assert_eq!(rules[0].note, None);
        assert_eq!(rules[1].note, Some(RuleNote::Unreachable));
        assert_eq!(rules[2].note, Some(RuleNote::NoEffect));

        // A wide rule under a narrow organization grant still works on the host both allow, so it
        // is not marked inert.
        let narrow = policy(
            r#"{"schema":4,"name":"org","version":"1","grants":[{"id":"one","hosts":{"allow":["x.example.com"]},"allowed":["read"]}]}"#,
        );
        let wide = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"wide","hosts":{"allow":["*.example.com"]},"allowed":["read"]}]}"#,
        );
        let view = compile(&inputs(Some(&narrow), Some(&wide)));
        assert_eq!(view.layers[1].rules[0].note, None);
    }

    #[test]
    fn a_user_file_ghostlight_does_not_own_is_shown_and_never_editable() {
        let user = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"work","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
        );
        let mut named = inputs(None, Some(&user));
        named.user_layer_source = UserLayerSource::Environment;
        named.authoring = Err(AuthoringError::NotOwned);
        let view = compile(&named);
        assert!(!view.user_layer.editable);
        assert_eq!(view.user_layer.path, None);
        assert_eq!(
            view.user_layer.blocked_reason.as_deref(),
            Some(policy::NOT_OWNED)
        );
        // A wide-open grant narrows nothing, so no capability line invents a decider.
        assert!(view
            .capabilities
            .iter()
            .all(|line| line.state == CapabilityState::Available));
        assert!(compile(&inputs(None, Some(&user))).user_layer.editable);
    }

    #[test]
    fn an_organization_ceiling_pins_browser_startup() {
        let organization = policy(
            r#"{"schema":4,"name":"org","version":"1","grants":[],"config":[{"key":"browser.startup","value":"manual","level":"mandatory"}]}"#,
        );
        let user = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[],"config":[{"key":"browser.startup","value":"on_demand","level":"mandatory"}]}"#,
        );
        let mut facts = inputs(Some(&organization), Some(&user));
        facts.windows = true;
        let view = compile(&facts);
        assert_eq!(view.browser_startup.value, manifest::BrowserStartup::Manual);
        assert_eq!(
            view.browser_startup.decided_by,
            Some(LayerKind::Organization)
        );
        assert_eq!(
            view.browser_startup.organization_ceiling,
            Some(manifest::BrowserStartup::Manual)
        );
    }

    #[test]
    fn suffix_wildcards_cover_subdomains_and_never_the_bare_host() {
        let pattern_covers = |broad: &str, narrow: &str| {
            let parse = |text| crate::governance::host_pattern::HostPattern::parse(text).unwrap();
            parse(broad).covers(&parse(narrow))
        };
        assert!(pattern_covers("*", "anything.test"));
        assert!(pattern_covers("*.example.com", "a.example.com"));
        assert!(pattern_covers("*.example.com", "*.a.example.com"));
        assert!(!pattern_covers("*.example.com", "example.com"));
        assert!(!pattern_covers("example.com", "a.example.com"));
        assert!(pattern_covers("example.com", "EXAMPLE.com"));
    }

    /// Whether a tool needs nothing, so a policy that admits nothing still lists it.
    fn needs_nothing(tool: &str) -> bool {
        capability_map::variants(tool).any(|variant| variant.requirements.is_empty())
    }

    #[test]
    fn disjoint_layer_grants_admit_nothing_and_advertise_nothing() {
        let organization = policy(
            r#"{"schema":4,"name":"org","version":"1","organization":{"name":"Northwind"},"grants":[{"id":"a","hosts":{"allow":["a.example"]},"allowed":["read"]}]}"#,
        );
        let user = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"b","hosts":{"allow":["b.example"]},"allowed":["read"]}]}"#,
        );
        let snapshot = snapshot(Some(&organization), Some(&user));
        for host in ["a.example", "b.example", "c.example"] {
            assert!(
                !snapshot
                    .authorize_landing(Capability::Read, &format!("https://{host}/"))
                    .allowed,
                "{host}"
            );
        }
        assert!(!snapshot.could_admit(CapabilitySet::READ));
        let listed = catalog_for(&snapshot);
        assert!(listed.iter().all(|tool| needs_nothing(&tool.name)));
        assert!(!listed.iter().any(|tool| tool.name == "browser_read"));

        let read = &compile(&inputs(Some(&organization), Some(&user))).capabilities[0];
        assert_eq!(read.state, CapabilityState::Unavailable);
        assert_eq!(
            read.detail,
            "Not available. The sites Northwind and you allow do not overlap."
        );
        assert_eq!(read.decided_by, [LayerKind::Organization, LayerKind::User]);
    }

    #[test]
    fn a_grant_whose_deny_cancels_its_universal_allow_reads_unavailable() {
        let canceled = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"nothing","hosts":{"allow":["*"],"deny":["*"]},"allowed":["read"]}]}"#,
        );
        let snapshot = snapshot(None, Some(&canceled));
        assert!(
            !snapshot
                .authorize_landing(Capability::Read, "https://example.com/")
                .allowed
        );
        assert!(!snapshot.could_admit(CapabilitySet::READ));
        let read = &compile(&inputs(None, Some(&canceled))).capabilities[0];
        assert_eq!(read.state, CapabilityState::Unavailable);
        assert_eq!(read.detail, "Not available. You do not allow it anywhere.");
        assert_eq!(read.decided_by, [LayerKind::User]);
    }

    #[test]
    fn a_grant_reaching_only_never_touch_hosts_advertises_nothing() {
        let cases = [
            // The user allows only hosts the user marked never-touch.
            (
                None,
                r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"bank","hosts":{"allow":["bank.example","*.vault.example"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["bank.example","*.vault.example"],"level":"mandatory"}]}"#,
                "Not available. The only sites you allow it on are marked never-touch by you.",
                vec![LayerKind::User],
            ),
            // The organization allows every host and marks the only hosts the user allows
            // never-touch: the user's rule is not what refuses it.
            (
                Some(
                    r#"{"schema":4,"name":"org","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["*.bank.example"],"level":"mandatory"}]}"#,
                ),
                r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"bank","hosts":{"allow":["*.bank.example"]},"allowed":["read"]}]}"#,
                "Not available. The only sites you allow it on are marked never-touch by your organization.",
                vec![LayerKind::Organization, LayerKind::User],
            ),
            // Each layer allows a site of its own, and the one site they share is never-touch,
            // so their sites do overlap.
            (
                Some(
                    r#"{"schema":4,"name":"org","version":"1","organization":{"name":"Northwind"},"grants":[{"id":"some","hosts":{"allow":["a.example","bank.example"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["bank.example"],"level":"mandatory"}]}"#,
                ),
                r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"some","hosts":{"allow":["b.example","bank.example"]},"allowed":["read"]}]}"#,
                "Not available. The only sites Northwind and you allow it on are marked never-touch by Northwind.",
                vec![LayerKind::Organization, LayerKind::User],
            ),
        ];
        for (organization, user, detail, decided_by) in cases {
            let organization = organization.map(policy);
            let user = policy(user);
            let snapshot = snapshot(organization.as_ref(), Some(&user));
            assert!(!snapshot.could_admit(CapabilitySet::READ), "{detail}");
            assert!(!catalog_for(&snapshot)
                .iter()
                .any(|tool| tool.name == "browser_read"));
            let read = &compile(&inputs(organization.as_ref(), Some(&user))).capabilities[0];
            assert_eq!(read.state, CapabilityState::Unavailable);
            assert_eq!(read.detail, detail);
            assert_eq!(read.decided_by, decided_by, "{detail}");
        }
    }

    #[test]
    fn the_catalog_the_policy_page_and_enforcement_agree_on_every_fixture() {
        let layer = |grants: &str, config: &str| {
            policy(&format!(
                r#"{{"schema":4,"name":"fixture","version":"1","grants":{grants},"config":{config}}}"#
            ))
        };
        let read = |hosts: &str| format!(r#"[{{"id":"g","hosts":{hosts},"allowed":["read"]}}]"#);
        let never_touch_bank = r#"[{"key":"content.security.sacred_domains","value":["bank.example"],"level":"mandatory"}]"#;
        let fixtures = [
            (None, None),
            (None, Some(layer(&read(r#"{"allow":["*"]}"#), "[]"))),
            (
                Some(layer(&read(r#"{"allow":["a.example"]}"#), "[]")),
                Some(layer(&read(r#"{"allow":["b.example"]}"#), "[]")),
            ),
            (
                None,
                Some(layer(&read(r#"{"allow":["*"],"deny":["*"]}"#), "[]")),
            ),
            (
                None,
                Some(layer(
                    &read(r#"{"allow":["bank.example"]}"#),
                    never_touch_bank,
                )),
            ),
            (
                Some(layer(&read(r#"{"allow":["*.example.com"]}"#), "[]")),
                Some(layer(
                    r#"[{"id":"g","hosts":{"allow":["a.example.com"]},"allowed":["read","action"]}]"#,
                    "[]",
                )),
            ),
            (
                Some(layer(
                    &read(r#"{"allow":["*"],"deny":["intranet.example"]}"#),
                    "[]",
                )),
                Some(layer(&read(r#"{"allow":["example.com"]}"#), "[]")),
            ),
            (Some(layer("[]", "[]")), None),
            (
                None,
                Some(layer(
                    &read(r#"{"allow":["*.a.example.com"],"deny":["*.example.com"]}"#),
                    "[]",
                )),
            ),
            (
                Some(layer(&read(r#"{"allow":["*.example.com"]}"#), "[]")),
                Some(layer(&read(r#"{"allow":["deep.a.example.com"]}"#), "[]")),
            ),
        ];
        // Chosen by hand, apart from the probes, so enforcement is asked independently.
        let hosts = [
            "example.com",
            "a.example.com",
            "deep.a.example.com",
            "a.example",
            "b.example",
            "intranet.example",
            "bank.example",
            "unrelated.org",
        ];
        for (index, (organization, user)) in fixtures.iter().enumerate() {
            let snapshot = snapshot(organization.as_ref(), user.as_ref());
            let view = compile(&inputs(organization.as_ref(), user.as_ref()));
            for line in &view.capabilities {
                let required = CapabilitySet::one(line.capability);
                let enforced = hosts.iter().any(|host| {
                    snapshot
                        .authorize_landing(required, &format!("https://{host}/"))
                        .allowed
                });
                let page = line.state != CapabilityState::Unavailable;
                let catalog = snapshot.could_admit(required);
                assert_eq!(page, enforced, "fixture {index}, {:?}", line.capability);
                assert_eq!(catalog, enforced, "fixture {index}, {:?}", line.capability);
            }
            let reads = catalog_for(&snapshot)
                .iter()
                .any(|tool| tool.name == "browser_read");
            assert_eq!(
                reads,
                view.capabilities[0].state != CapabilityState::Unavailable,
                "fixture {index}"
            );
        }
    }

    #[test]
    fn the_chip_is_the_compiled_headline() {
        let organization = policy(
            r#"{"schema":4,"name":"org","version":"1","organization":{"name":"Northwind"},"grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]}"#,
        );
        let user = policy(
            r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        );
        let mut switched = inputs(Some(&organization), None);
        switched.channels_off = ChannelsOff {
            mcp: Some(PolicyAuthor::Organization),
            cli: None,
            organization: Some("Northwind"),
        };
        let mut stale = inputs(Some(&organization), Some(&user));
        stale.passport.freshness = crate::governance::ManagedPolicyFreshness::LastKnownGood;
        let mut broken = inputs(None, Some(&user));
        broken.valid = false;
        for (fixture, tone) in [
            (inputs(None, None), ChipTone::Open),
            (inputs(Some(&organization), None), ChipTone::Applied),
            (inputs(None, Some(&user)), ChipTone::Applied),
            (inputs(Some(&organization), Some(&user)), ChipTone::Applied),
            (switched, ChipTone::Applied),
            (stale, ChipTone::Warning),
            (broken, ChipTone::Failing),
        ] {
            let view = compile(&fixture);
            let chip = PolicyChip::of(&view);
            assert_eq!(chip.detail, view.headline);
            assert_eq!(chip.situation, view.situation);
            assert_eq!(chip.tone, tone, "{:?}", view.situation);
        }
    }

    #[test]
    fn enforcement_and_the_view_name_the_same_audit_layer() {
        let mode = |value: &str| {
            policy(&format!(
                r#"{{"schema":4,"name":"p","version":"1","grants":[],"config":[{{"key":"{}","value":"{value}","level":"mandatory"}}]}}"#,
                manifest::PolicySetting::AuditAvailability.key()
            ))
        };
        let choices = [
            None,
            Some(mode("keep_working")),
            Some(mode("require_audit")),
        ];
        for organization in &choices {
            for user in &choices {
                let fixture = inputs(organization.as_ref(), user.as_ref());
                let view = compile(&fixture);
                let refused = fixture.snapshot.authorize_audit(false);
                assert_eq!(
                    !refused.allowed,
                    fixture.snapshot.requires_audit(),
                    "{organization:?} {user:?}"
                );
                if refused.allowed {
                    assert_eq!(
                        view.audit.mode,
                        crate::governance::audit::AuditMode::KeepWorking
                    );
                    continue;
                }
                let (tier, _) = fixture.snapshot.attribution(refused).expect("attributed");
                let named = match view.audit.source {
                    Some(LayerKind::Organization) => AuthorityTier::Managed,
                    Some(LayerKind::User) => AuthorityTier::User,
                    other => panic!("no layer named: {other:?}"),
                };
                assert_eq!(tier, named.as_str(), "{organization:?} {user:?}");
                assert_eq!(
                    view.audit.mode,
                    crate::governance::audit::AuditMode::RequireAudit
                );
            }
        }
    }
}
