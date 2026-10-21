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
    host_pattern, manifest, AuthoritySnapshot, Capability, CapabilitySet, ManagedPolicyFreshness,
    ManagedPolicyPassport,
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
    pub audit: super::audit_requirement::AuditPolicy,
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
    pub(super) never_touch_hosts: Vec<String>,
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
        ceilings: policy::ceilings(&inputs.never_touch_hosts),
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
                let kinds = |tiers: &[PolicyAuthor]| {
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
pub(super) const fn layer_kind(tier: PolicyAuthor) -> LayerKind {
    match tier {
        PolicyAuthor::Organization => LayerKind::Organization,
        PolicyAuthor::User => LayerKind::User,
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
    let admitted = ceiling?.layer_admits_within(
        PolicyAuthor::Organization,
        grant.allowed_set(),
        &grant.hosts,
    )?;
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
        compile, CapabilityState, Inputs, LayerKind, OrganizationIdentity, RuleNote, Situation,
        UserLayerSource,
    };
    use std::sync::Arc;

    use crate::governance::{
        assemble, manifest, AuthoritySnapshot, Capability, CapabilitySet, ManagedPolicyPassport,
        OwnOrigin, PROBE_LABEL,
    };
    use crate::language::catalog_for;
    use crate::language::channel_switch::ChannelsOff;
    use crate::language::outcome::PolicyAuthor;
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
                (organization.cloned(), PolicyAuthor::Organization),
                (user.cloned(), PolicyAuthor::User),
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
            never_touch_hosts: snapshot.never_touch_hosts(),
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

    /// One layer of a fixture: a policy with these grants and settings.
    fn layer(grants: &str, config: &str) -> manifest::Manifest {
        policy(&format!(
            r#"{{"schema":4,"name":"fixture","version":"1","grants":{grants},"config":{config}}}"#
        ))
    }

    /// An organization layer named `name`, with these grants.
    fn named(name: &str, grants: &str, config: &str) -> manifest::Manifest {
        policy(&format!(
            r#"{{"schema":4,"name":"org","version":"1","organization":{{"name":"{name}"}},"grants":{grants},"config":{config}}}"#
        ))
    }

    /// One read grant over `hosts`.
    fn read(hosts: &str) -> String {
        format!(r#"[{{"id":"g","hosts":{hosts},"allowed":["read"]}}]"#)
    }

    /// The never-touch setting over `hosts`, a JSON list.
    fn never_touch(hosts: &str) -> String {
        format!(
            r#"[{{"key":"{}","value":{hosts},"level":"mandatory"}}]"#,
            manifest::PolicySetting::NeverTouch.key()
        )
    }

    /// ADR-0122 Decision 2 and ACCEPTANCE Governance 4, as the Policy page and policy_explain read
    /// it: the headline names the situation and any switched-off channel; each line says whether
    /// its capability is available, refused except where allowed, or allowed except where
    /// blocked, and names who narrowed it; a capability nothing admits says why -- allowed
    /// nowhere, layers that do not overlap, or only never-touch sites -- and who. An invalid
    /// policy says only that it is invalid (842ef606, e45f09ed, 3373e274).
    #[test]
    fn the_policy_page_says_what_each_capability_can_do_and_who_decided() {
        use LayerKind::{Ghostlight, Organization, User};

        /// What one capability's line says.
        type Line = (Capability, CapabilityState, String, Vec<LayerKind>);
        let every = |state: CapabilityState, detail: &str, decided_by: &[LayerKind]| -> Vec<Line> {
            Capability::ALL
                .into_iter()
                .map(|capability| (capability, state, detail.to_owned(), decided_by.to_vec()))
                .collect()
        };
        let one = |capability, state, detail: String, decided_by: &[LayerKind]| -> Line {
            (capability, state, detail, decided_by.to_vec())
        };
        let read_line = |state, detail, decided_by: &[LayerKind]| {
            vec![one(Capability::Read, state, detail, decided_by)]
        };
        let everything = r#"[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]"#;
        let user_mcp_off = ChannelsOff {
            mcp: Some(PolicyAuthor::User),
            ..ChannelsOff::default()
        };
        let northwind_off = ChannelsOff {
            mcp: Some(PolicyAuthor::Organization),
            cli: Some(PolicyAuthor::Organization),
            organization: Some("Northwind"),
        };
        let user_off_under_northwind = ChannelsOff {
            mcp: Some(PolicyAuthor::User),
            cli: None,
            organization: Some("Northwind"),
        };
        let with_line = |said: String, switched: ChannelsOff<'_>| {
            format!(
                "{said} {}",
                switched.policy_line().expect("a switch is off")
            )
        };
        let usable = |said: &str, switched: ChannelsOff<'_>| {
            format!(
                "{said} {}",
                switched.capability_line().expect("one switch is off")
            )
        };

        /// (what, organization, user, switches, valid) -> (situation, headline, lines).
        type Row<'a> = (
            &'a str,
            Option<manifest::Manifest>,
            Option<manifest::Manifest>,
            ChannelsOff<'a>,
            bool,
            Situation,
            String,
            Vec<Line>,
        );
        let rows: Vec<Row<'_>> = vec![
            (
                "all-open",
                None,
                None,
                ChannelsOff::default(),
                true,
                Situation::AllOpen,
                policy::situation(Situation::AllOpen, None),
                every(
                    CapabilityState::Available,
                    policy::AVAILABLE_UNNARROWED,
                    &[],
                ),
            ),
            (
                "an open baseline the user carved",
                None,
                Some(layer(
                    &read(r#"{"allow":["*"],"deny":["intranet.example"]}"#),
                    "[]",
                )),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                read_line(
                    CapabilityState::SomeBlocked,
                    policy::some_blocked(&policy::who(&[User], None)),
                    &[User],
                ),
            ),
            (
                "a closed baseline the user opened",
                None,
                Some(layer(&read(r#"{"allow":["example.com"]}"#), "[]")),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                vec![
                    one(
                        Capability::Read,
                        CapabilityState::SomeAllowed,
                        policy::some_allowed(&policy::who(&[User], None)),
                        &[User],
                    ),
                    one(
                        Capability::Execute,
                        CapabilityState::Unavailable,
                        policy::allowed_nowhere(User, None),
                        &[User],
                    ),
                ],
            ),
            (
                "a named organization",
                Some(named(
                    "Example Organization",
                    r#"[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]"#,
                    "[]",
                )),
                None,
                ChannelsOff::default(),
                true,
                Situation::OrganizationOnly,
                policy::situation(Situation::OrganizationOnly, Some("Example Organization")),
                vec![
                    one(
                        Capability::Read,
                        CapabilityState::SomeAllowed,
                        policy::some_allowed(&policy::who(
                            &[Organization],
                            Some("Example Organization"),
                        )),
                        &[Organization],
                    ),
                    one(
                        Capability::Execute,
                        CapabilityState::Unavailable,
                        policy::allowed_nowhere(Organization, Some("Example Organization")),
                        &[Organization],
                    ),
                ],
            ),
            (
                "an organization with no name",
                Some(layer(&read(r#"{"allow":["example.com"]}"#), "[]")),
                None,
                ChannelsOff::default(),
                true,
                Situation::OrganizationOnly,
                policy::situation(Situation::OrganizationOnly, None),
                vec![one(
                    Capability::Execute,
                    CapabilityState::Unavailable,
                    policy::allowed_nowhere(Organization, None),
                    &[Organization],
                )],
            ),
            (
                "the narrower of two layers, both named",
                Some(layer(
                    &read(r#"{"allow":["*"],"deny":["intranet.example"]}"#),
                    "[]",
                )),
                Some(layer(&read(r#"{"allow":["example.com"]}"#), "[]")),
                ChannelsOff::default(),
                true,
                Situation::Layered,
                policy::situation(Situation::Layered, None),
                read_line(
                    CapabilityState::SomeAllowed,
                    policy::some_allowed(&policy::who(&[Organization, User], None)),
                    &[Organization, User],
                ),
            ),
            (
                "a grant whose deny cancels its own allow",
                None,
                Some(layer(
                    &read(r#"{"allow":["*.example.com"],"deny":["*.example.com"]}"#),
                    "[]",
                )),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                read_line(
                    CapabilityState::Unavailable,
                    policy::allowed_nowhere(User, None),
                    &[User],
                ),
            ),
            (
                "a deny that removes one host under a wildcard",
                None,
                Some(layer(
                    &read(r#"{"allow":["*.example.com"],"deny":["admin.example.com"]}"#),
                    "[]",
                )),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                read_line(
                    CapabilityState::SomeAllowed,
                    policy::some_allowed(&policy::who(&[User], None)),
                    &[User],
                ),
            ),
            (
                "a broader deny under a narrower allow",
                None,
                Some(layer(
                    &read(r#"{"allow":["*.a.example.com"],"deny":["*.example.com"]}"#),
                    "[]",
                )),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                read_line(
                    CapabilityState::SomeAllowed,
                    policy::some_allowed(&policy::who(&[User], None)),
                    &[User],
                ),
            ),
            (
                "layers whose sites do not overlap",
                Some(named(
                    "Northwind",
                    &read(r#"{"allow":["a.example"]}"#),
                    "[]",
                )),
                Some(layer(&read(r#"{"allow":["b.example"]}"#), "[]")),
                ChannelsOff::default(),
                true,
                Situation::Layered,
                policy::situation(Situation::Layered, Some("Northwind")),
                read_line(
                    CapabilityState::Unavailable,
                    policy::disjoint(&policy::who(&[Organization, User], Some("Northwind"))),
                    &[Organization, User],
                ),
            ),
            (
                "a deny that cancels a universal allow",
                None,
                Some(layer(&read(r#"{"allow":["*"],"deny":["*"]}"#), "[]")),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                read_line(
                    CapabilityState::Unavailable,
                    policy::allowed_nowhere(User, None),
                    &[User],
                ),
            ),
            (
                "the user allows only sites the user marked never-touch",
                None,
                Some(layer(
                    &read(r#"{"allow":["bank.example","*.vault.example"]}"#),
                    &never_touch(r#"["bank.example","*.vault.example"]"#),
                )),
                ChannelsOff::default(),
                true,
                Situation::UserOnly,
                policy::situation(Situation::UserOnly, None),
                read_line(
                    CapabilityState::Unavailable,
                    policy::only_never_touch(&[User], &[User], None),
                    &[User],
                ),
            ),
            (
                "the user allows only sites the organization marked never-touch",
                Some(layer(
                    &read(r#"{"allow":["*"]}"#),
                    &never_touch(r#"["*.bank.example"]"#),
                )),
                Some(layer(&read(r#"{"allow":["*.bank.example"]}"#), "[]")),
                ChannelsOff::default(),
                true,
                Situation::Layered,
                policy::situation(Situation::Layered, None),
                read_line(
                    CapabilityState::Unavailable,
                    policy::only_never_touch(&[User], &[Organization], None),
                    &[Organization, User],
                ),
            ),
            (
                "the one site both layers share is never-touch",
                Some(named(
                    "Northwind",
                    &read(r#"{"allow":["a.example","bank.example"]}"#),
                    &never_touch(r#"["bank.example"]"#),
                )),
                Some(layer(
                    &read(r#"{"allow":["b.example","bank.example"]}"#),
                    "[]",
                )),
                ChannelsOff::default(),
                true,
                Situation::Layered,
                policy::situation(Situation::Layered, Some("Northwind")),
                read_line(
                    CapabilityState::Unavailable,
                    policy::only_never_touch(
                        &[Organization, User],
                        &[Organization],
                        Some("Northwind"),
                    ),
                    &[Organization, User],
                ),
            ),
            (
                "the user turned MCP off",
                None,
                Some(policy(
                    r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write"]}]}"#,
                )),
                user_mcp_off,
                true,
                Situation::UserOnly,
                with_line(policy::situation(Situation::UserOnly, None), user_mcp_off),
                vec![
                    one(
                        Capability::Read,
                        CapabilityState::Available,
                        usable(policy::AVAILABLE, user_mcp_off),
                        &[User],
                    ),
                    // A capability nothing may use says only why, with no promise about who may
                    // use it.
                    one(
                        Capability::Execute,
                        CapabilityState::Unavailable,
                        policy::allowed_nowhere(User, None),
                        &[User],
                    ),
                ],
            ),
            (
                "the user turned MCP off under Northwind",
                Some(named("Northwind", everything, "[]")),
                Some(layer(everything, "[]")),
                user_off_under_northwind,
                true,
                Situation::Layered,
                with_line(
                    policy::situation(Situation::Layered, Some("Northwind")),
                    user_off_under_northwind,
                ),
                every(
                    CapabilityState::Available,
                    &usable(policy::AVAILABLE, user_off_under_northwind),
                    &[User],
                ),
            ),
            (
                "Northwind turned both channels off",
                Some(named("Northwind", everything, "[]")),
                None,
                northwind_off,
                true,
                Situation::OrganizationOnly,
                with_line(
                    policy::situation(Situation::OrganizationOnly, Some("Northwind")),
                    northwind_off,
                ),
                every(
                    CapabilityState::Unavailable,
                    &policy::not_available(&northwind_off.said().expect("both are off")),
                    &[Organization],
                ),
            ),
            (
                "an invalid policy with both channels off",
                Some(named("Northwind", everything, "[]")),
                None,
                northwind_off,
                false,
                Situation::FailingClosed,
                policy::situation(Situation::FailingClosed, Some("Northwind")),
                every(
                    CapabilityState::Unavailable,
                    policy::REFUSED_WHILE_INVALID,
                    &[Ghostlight],
                ),
            ),
        ];
        for (what, organization, user, switched, valid, situation, headline, lines) in &rows {
            let mut facts = inputs(organization.as_ref(), user.as_ref());
            facts.channels_off = *switched;
            facts.valid = *valid;
            let view = compile(&facts);
            assert_eq!(view.situation, *situation, "{what}");
            assert_eq!(&view.headline, headline, "{what}");
            for (capability, state, detail, decided_by) in lines {
                let line = view
                    .capabilities
                    .iter()
                    .find(|line| line.capability == *capability)
                    .expect("every capability has a line");
                assert_eq!(line.state, *state, "{what}: {capability:?}");
                assert_eq!(&line.detail, detail, "{what}: {capability:?}");
                assert_eq!(&line.decided_by, decided_by, "{what}: {capability:?}");
            }
        }
    }

    /// ADR-0122 rule notes and 3373e274: a rule is marked only when Ghostlight can prove it does
    /// nothing -- unreachable behind an earlier rule that covers it, or of no effect where the
    /// organization refuses everything it allows -- and a wide rule that still works on a host
    /// the organization allows is not marked.
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

    /// 3373e274 (ADR-0203 Decision 7.1, finding G-04) and e45f09ed: the catalog, the Policy page
    /// and enforcement answer "can this capability be used anywhere" one way. For every fixture, a
    /// line the page calls usable is one some hand-chosen host admits, and one the catalog can
    /// list. The probes the answer is built from never collide with an authored pattern.
    #[test]
    fn the_catalog_the_policy_page_and_enforcement_agree_on_every_fixture() {
        let never_touch_bank = never_touch(r#"["bank.example"]"#);
        // A probe that took an authored exact host's name would stand for that host alone, and
        // the page would miss the hosts its class stands for.
        let probe_collision =
            format!(r#"{{"allow":["*.example.com"],"deny":["{PROBE_LABEL}.example.com"]}}"#);
        let label_collision = format!(r#"{{"allow":["*"],"deny":["{PROBE_LABEL}"]}}"#);
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
                    &never_touch_bank,
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
            (
                None,
                Some(layer(
                    &read(r#"{"allow":["*.example.com"],"deny":["*.example.com"]}"#),
                    "[]",
                )),
            ),
            (
                Some(layer(
                    &read(r#"{"allow":["*"]}"#),
                    &never_touch(r#"["*.bank.example"]"#),
                )),
                Some(layer(&read(r#"{"allow":["*.bank.example"]}"#), "[]")),
            ),
            (
                Some(layer(
                    &read(r#"{"allow":["a.example","bank.example"]}"#),
                    &never_touch_bank,
                )),
                Some(layer(
                    &read(r#"{"allow":["b.example","bank.example"]}"#),
                    "[]",
                )),
            ),
            (None, Some(layer(&read(&probe_collision), "[]"))),
            (None, Some(layer(&read(&label_collision), "[]"))),
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
            "www.bank.example",
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
}
