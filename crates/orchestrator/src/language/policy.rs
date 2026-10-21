//! What a person reads about the policy in force.
//!
//! Governance decides; this module says it. The compiled view's headline and capability lines, its
//! ceilings and layer titles, why this window may not change the person's own policy, the class of
//! a signed organization source, and what `ghostlight policy` prints are all authored here, so the
//! window renders sentences rather than inventing them from booleans and the command line and the
//! window never say one fact two ways (ADR-0203 Decision 8).
//!
//! The channel switches have their own module, [`super::channel_switch`].

use crate::governance::effective::{LayerKind, Situation};
use crate::governance::{manifest, Capability, ManagedPolicySource};
use crate::language::capability_map;
use crate::language::outcome::OWN_CONTROL_PAGE_CEILING;

/// Why this window cannot change a user policy an environment variable names. The window shows
/// it beside the policy, and a write the window is asked for anyway is refused with it.
pub const NOT_OWNED: &str = "An environment variable points Ghostlight at a policy file it does \
not own, so this window cannot change it.";

/// Why this window cannot keep a user policy on a machine with no per-user state directory. The
/// window shows it, and a write the window is asked for anyway is refused with it.
pub const NO_HOME: &str = "This machine names no per-user state directory to keep a policy in.";

/// The title of the person's own layer.
pub const USER_LAYER_TITLE: &str = "Your rules";

/// Every capability line while a configured policy source cannot be read.
pub const REFUSED_WHILE_INVALID: &str = "Refused while a configured policy source cannot be read.";

/// A capability nothing narrows, said without a promise when a channel switch narrows who may use
/// it; the switch's own line follows.
pub const AVAILABLE: &str = "Available on ordinary websites.";

/// A capability nothing narrows at all.
pub const AVAILABLE_UNNARROWED: &str = "Available on ordinary websites. Nothing narrows it.";

/// The first ceiling, in every situation including all-open.
const NOT_HTTP_CEILING: &str = "Anything that is not an ordinary http or https address.";

/// The organization's name where its policy names none.
const UNNAMED_ORGANIZATION: &str = "Your organization";

/// What `ghostlight policy` prints when its arguments name no subcommand it knows.
pub const USAGE: &str = "usage: ghostlight policy <validate|explain|migrate> <policy.json>\n       ghostlight policy simulate <policy.json> <audit.jsonl>\n       ghostlight policy <keygen|pubkey|sign|publish> ...";

/// How many hex characters of a policy hash the command line shows.
const SHORT_HASH_CHARS: usize = 12;

/// The plain label for a capability and what it covers, for someone who has never read the
/// policy language (ADR-0122 Decision 6).
#[must_use]
pub const fn capability_wording(capability: Capability) -> (&'static str, &'static str) {
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

/// The situation's sentence, naming the organization when its policy names one.
#[must_use]
pub fn situation(situation: Situation, organization: Option<&str>) -> String {
    match situation {
        Situation::AllOpen => {
            "No rules apply. Agents may work on ordinary websites, within the limits below.".into()
        }
        Situation::OrganizationOnly => format!(
            "{} sets the rules on this machine.",
            organization.unwrap_or(UNNAMED_ORGANIZATION)
        ),
        Situation::UserOnly => "You set the rules on this machine.".into(),
        Situation::Layered => format!(
            "{} sets the rules, and you have narrowed them further.",
            organization.unwrap_or(UNNAMED_ORGANIZATION)
        ),
        Situation::FailingClosed => {
            "Ghostlight is refusing all governed work: a configured policy source is not valid. \
             Nothing is permitted until it can be read again."
                .into()
        }
    }
}

/// The title of the organization's layer.
#[must_use]
pub fn organization_title(organization: Option<&str>) -> String {
    organization.unwrap_or(UNNAMED_ORGANIZATION).to_owned()
}

/// The layers named inside a sentence, in the order given: "Northwind and you".
#[must_use]
pub fn who(kinds: &[LayerKind], organization: Option<&str>) -> String {
    kinds
        .iter()
        .map(|kind| match (kind, organization) {
            (LayerKind::Organization, Some(name)) => name,
            (LayerKind::Organization, None) => "your organization",
            (LayerKind::User, _) => "you",
            (LayerKind::Ghostlight, _) => "Ghostlight",
        })
        .collect::<Vec<_>>()
        .join(" and ")
}

/// A capability allowed everywhere except the sites `who` blocked.
#[must_use]
pub fn some_blocked(who: &str) -> String {
    format!("Available everywhere except the sites {who} blocked.")
}

/// A capability refused everywhere except the sites `who` allowed.
#[must_use]
pub fn some_allowed(who: &str) -> String {
    format!("Refused everywhere except the sites {who} allowed.")
}

/// A capability no request can use, and why.
#[must_use]
pub fn not_available(why: &str) -> String {
    format!("Not available. {why}")
}

/// A capability the layers named by `who` each allow somewhere, but never on one shared site.
#[must_use]
pub fn disjoint(who: &str) -> String {
    not_available(&format!("The sites {who} allow do not overlap."))
}

/// A capability allowed only on sites a never-touch list refuses: who allowed it, and who marked
/// those sites never-touch.
#[must_use]
pub fn only_never_touch(
    allowed_by: &[LayerKind],
    marked_by: &[LayerKind],
    organization: Option<&str>,
) -> String {
    let allow = match allowed_by {
        [LayerKind::Organization] => "allows",
        _ => "allow",
    };
    not_available(&format!(
        "The only sites {} {allow} it on are marked never-touch by {}.",
        who(allowed_by, organization),
        who(marked_by, organization)
    ))
}

/// A capability `kind` allows nowhere, said in that layer's own grammar: a person reading why
/// they were stopped deserves "You do not allow it", not "you does not allow it".
#[must_use]
pub fn allowed_nowhere(kind: LayerKind, organization: Option<&str>) -> String {
    let refusal = match kind {
        LayerKind::User => "You do not allow it anywhere.".to_owned(),
        LayerKind::Ghostlight => "Ghostlight does not allow it anywhere.".to_owned(),
        LayerKind::Organization => format!(
            "{} does not allow it anywhere.",
            organization.unwrap_or(UNNAMED_ORGANIZATION)
        ),
    };
    not_available(&refusal)
}

/// The boundaries no policy can lift, one line each, naming every never-touch host.
#[must_use]
pub fn ceilings(never_touch: &[String]) -> Vec<String> {
    let mut lines = vec![NOT_HTTP_CEILING.to_owned()];
    lines.extend(
        never_touch
            .iter()
            .map(|host| format!("{host}, marked never-touch by policy.")),
    );
    lines.push(OWN_CONTROL_PAGE_CEILING.to_owned());
    lines
}

/// A signed organization source, named by class and never by address.
#[must_use]
pub const fn signed_source(source: ManagedPolicySource) -> &'static str {
    match source {
        ManagedPolicySource::Https => "Signed bundle from an HTTPS source",
        ManagedPolicySource::File => "Signed bundle from a local file",
        ManagedPolicySource::None => "Signed bundle",
    }
}

/// What `ghostlight policy validate` prints for a valid policy.
#[must_use]
pub fn valid(policy: &manifest::Manifest) -> String {
    format!(
        "Valid schema-4 policy: {} {} ({})",
        policy.name,
        policy.version,
        short_hash(&policy.hash)
    )
}

/// What `ghostlight policy explain` prints: the policy, its rules and settings, and the exact
/// capability map, one line each.
#[must_use]
pub fn explanation(policy: &manifest::Manifest) -> Vec<String> {
    let mut lines = vec![
        format!("Policy: {} {}", policy.name, policy.version),
        format!("Identity: {}", short_hash(&policy.hash)),
    ];
    if let Some(identity) = &policy.identity {
        if let Some(principal) = &identity.principal {
            lines.push(format!("Principal: {principal}"));
        }
        if let Some(groups) = identity.groups.as_ref().filter(|groups| !groups.is_empty()) {
            lines.push(format!("Groups: {}", groups.join(", ")));
        }
    }
    if policy.grants.is_empty() {
        lines.push("Grants: none. Governed browser work is denied by default.".to_owned());
    } else {
        lines.push("Grants:".to_owned());
        for grant in &policy.grants {
            let hosts = if grant.hosts.allow.is_empty() {
                "no hosts".to_owned()
            } else {
                grant.hosts.allow.join(", ")
            };
            let mut line = format!("  {}: {} on {hosts}", grant.id, grant.allowed_set().label());
            if !grant.hosts.deny.is_empty() {
                line.push_str(&format!("; except {}", grant.hosts.deny.join(", ")));
            }
            lines.push(line);
            if let Some(description) = &grant.description {
                lines.push(format!("    {description}"));
            }
        }
    }
    if !policy.config.is_empty() {
        lines.push("Settings:".to_owned());
        lines.extend(policy.config.iter().map(|setting| {
            format!(
                "  {} = {} ({})",
                setting.key,
                setting.value,
                setting.level.as_str()
            )
        }));
    }
    lines.push("Capability map:".to_owned());
    lines.extend(capability_map::DIRECTORY.iter().map(|entry| {
        let variant = entry
            .variant
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        format!(
            "  {}{variant}: {} -- {}",
            entry.tool,
            entry.requirements.label(),
            entry.description
        )
    }));
    lines
}

/// What `ghostlight policy simulate` prints where the history records receipts it never saved.
#[must_use]
pub fn history_gap(unconfirmed_receipts: u64) -> String {
    format!("History gap: {unconfirmed_receipts} receipts were not confirmed saved.")
}

/// What `ghostlight policy simulate` prints for a history line it cannot read.
#[must_use]
pub fn unreadable_line(line: usize) -> String {
    format!("Unreadable history entry at line {line}; skipped.")
}

/// One recorded request the candidate policy would refuse.
pub struct WouldDeny<'a> {
    /// The history line it was read from, counted from one.
    pub line: usize,
    /// The tool the request called.
    pub tool: &'a str,
    /// The capabilities it required, as the policy labels them.
    pub requirements: &'a str,
    /// The host it worked on, when it named one.
    pub host: Option<&'a str>,
    /// The rule that refused it, or the refusal reason when no rule did.
    pub rule: &'a str,
    /// The refusal's denial id, when it has one.
    pub denial_id: Option<&'a str>,
}

/// What `ghostlight policy simulate` prints for a recorded request the candidate would refuse.
#[must_use]
pub fn would_deny(denied: &WouldDeny<'_>) -> String {
    format!(
        "Would deny line {}: {} [{}] on {} -- {}{}",
        denied.line,
        denied.tool,
        denied.requirements,
        denied.host.unwrap_or("no host"),
        denied.rule,
        denied
            .denial_id
            .map(|id| format!(" ({id})"))
            .unwrap_or_default()
    )
}

/// What `ghostlight policy simulate` prints last: how many records it replayed against which
/// policy, and how many the policy would refuse.
#[must_use]
pub fn simulated(records: u64, policy: &manifest::Manifest, denied: u64) -> String {
    format!(
        "Simulated {records} record(s) against {} {}: {denied} would be denied.",
        policy.name, policy.version
    )
}

fn short_hash(hash: &str) -> &str {
    hash.get(..SHORT_HASH_CHARS).unwrap_or(hash)
}
