//! Per-call grant enforcement (ADR-0022 Decision 5): the pure decision core. This IS the
//! `PolicyDecisionPoint::decide` the a2 seam anticipated; [`LocalPdp`] is the concrete,
//! in-process implementation `Governance::governed` uses once a manifest is active, alongside
//! the `NoopPdp` (a2) all-open placeholder.
//!
//! Pure: no I/O, no async, no clock. Host polarity evaluation is injected as a function pointer
//! (`evaluate_host: fn(host, allow, deny) -> HostRuleOutcome`), supplied by the composition
//! root using the browser plugin's real ADR-0022 Decision 4 evaluator, so this core module
//! never names the browser plugin directly (the a7 arch-test forbids it).

use crate::governance::denial;
use crate::governance::manifest::document::Grant;
use crate::governance::ports::{
    Capability, Decision, DecisionRequest, Denial, EffectiveMode, GoverningResource,
    HostRuleOutcome, PolicyDecisionPoint,
};

/// The in-process policy decision point wrapping [`check_call`]. `Governance::governed` uses
/// this once a manifest is active.
pub struct LocalPdp {
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
}

impl LocalPdp {
    /// `evaluate_host(host, allow, deny)`: the ALREADY-NORMALIZED `host` against one grant's
    /// host rules (the browser plugin's real ADR-0022 Decision 4 evaluator).
    pub fn new(evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome) -> Self {
        Self { evaluate_host }
    }
}

impl PolicyDecisionPoint for LocalPdp {
    fn decide(&self, req: &DecisionRequest) -> Decision {
        check_call(
            &req.grants,
            &req.tool,
            req.action.as_deref(),
            &req.requires,
            &req.resource,
            &req.manifest_hash,
            self.evaluate_host,
            req.manifest_mode,
            req.config_mode,
        )
    }
}

/// Resolve the effective enforcement mode of one decision (shared format doc section 3.4,
/// g15): a resolving grant's own `mode` wins when set, else the manifest-level `mode`, else
/// the resolved `governance.mode`. `config` is never optional: the layered resolver always
/// defines `governance.mode` (the built-in Minimal preset is the floor), so resolution never
/// fails to produce a mode.
pub fn effective_mode(
    grant: Option<EffectiveMode>,
    manifest: Option<EffectiveMode>,
    config: EffectiveMode,
) -> EffectiveMode {
    grant.or(manifest).unwrap_or(config)
}

/// Wrap a raw `check_call` verdict into its final form (g15, ADR-0020 commitment 4): `Allow`
/// passes through unchanged (there is nothing to shadow); a `Deny` becomes `ShadowDeny` when
/// the effective mode resolves to `Observe`, else stays `Deny`. The resolving grant's own
/// `mode`, if any, is looked up by the denial's own `grant_id` -- `check_call` never needs to
/// thread a second grant reference through its internal helpers for this. Sacred-domain
/// denials never reach this function at all (they are a separate, always-on code path at the
/// dispatch chokepoint that never touches `Decision`/`check_call`), so every `Deny` this
/// function ever sees is eligible for the mode switch; there is no `sacred` rule to carve out
/// here.
pub(crate) fn apply_mode(
    decision: Decision,
    grants: &[Grant],
    manifest_mode: Option<EffectiveMode>,
    config_mode: EffectiveMode,
) -> Decision {
    let Decision::Deny(denial) = decision else {
        return decision;
    };
    let grant_mode = denial
        .grant_id
        .as_deref()
        .and_then(|id| grants.iter().find(|g| g.id == id))
        .and_then(|g| g.mode);
    match effective_mode(grant_mode, manifest_mode, config_mode) {
        EffectiveMode::Enforce => Decision::Deny(denial),
        EffectiveMode::Observe => Decision::ShadowDeny(denial),
    }
}

/// The pure per-call grant-resolution decision (ADR-0022 Decision 5). STEP 0 (no manifest ->
/// allow) lives at the caller ([`crate::governance::dispatch::Governance::decide`]); this
/// function always assumes a manifest is active. Order is load-bearing (the denial id depends
/// on the rule string, so the first failing rule must be deterministic):
///
/// 1. `requires.is_empty()` short-circuits to `Allow` BEFORE any resource matching or grant
///    walk (ADR-0022 Decision 5 step 2).
/// 2. Resource-kind dispatch (`AlwaysAllow`/`OutOfScope`/`Indeterminate`/`None`/`Resource`).
/// 3. For a resolved host, grant resolution (host polarity, first `Allowed` wins; remember the
///    first `Denied`), THEN the capability (subset containment) check.
#[allow(clippy::too_many_arguments)]
pub fn check_call(
    grants: &[Grant],
    tool: &str,
    action: Option<&str>,
    requires: &[Capability],
    resource: &GoverningResource,
    manifest_hash: &str,
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
    manifest_mode: Option<EffectiveMode>,
    config_mode: EffectiveMode,
) -> Decision {
    if requires.is_empty() {
        return Decision::Allow { grant_id: None };
    }

    let raw = match resource {
        GoverningResource::AlwaysAllow => Decision::Allow { grant_id: None },
        GoverningResource::OutOfScope(scheme) => {
            Decision::Deny(scheme_denial(scheme, manifest_hash))
        }
        GoverningResource::Indeterminate => {
            Decision::Deny(unmatched_domain_denial("(unknown)", manifest_hash))
        }
        GoverningResource::Resource(host) => decide_for_host(
            grants,
            tool,
            action,
            requires,
            host,
            manifest_hash,
            evaluate_host,
        ),
        GoverningResource::None => decide_no_page(grants, tool, action, requires, manifest_hash),
    };
    apply_mode(raw, grants, manifest_mode, config_mode)
}

/// Grant resolution for a resolved host (ADR-0022 Decision 5 step 4): walk grants in manifest
/// order, evaluating each one's host polarity. The first grant whose polarity evaluates to
/// `Allowed` is the resolving grant (stop walking); a grant returning `Denied` does not cover
/// the host, but its id is remembered (first one only) for the `denied_domain` attribution if
/// no grant ever resolves.
fn resolve_grant<'a>(
    grants: &'a [Grant],
    host: &str,
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
) -> (Option<&'a Grant>, Option<&'a Grant>) {
    let mut first_denying: Option<&Grant> = None;
    for grant in grants {
        match evaluate_host(host, &grant.hosts.allow, &grant.hosts.deny) {
            HostRuleOutcome::Allowed => return (Some(grant), first_denying),
            HostRuleOutcome::Denied => {
                if first_denying.is_none() {
                    first_denying = Some(grant);
                }
            }
            HostRuleOutcome::Unmatched => {}
        }
    }
    (None, first_denying)
}

fn decide_for_host(
    grants: &[Grant],
    tool: &str,
    action: Option<&str>,
    requires: &[Capability],
    host: &str,
    manifest_hash: &str,
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
) -> Decision {
    let (resolving, first_denying) = resolve_grant(grants, host, evaluate_host);
    let Some(grant) = resolving else {
        return Decision::Deny(match first_denying {
            Some(g) => denied_domain_denial(g, host, manifest_hash),
            None => unmatched_domain_denial(host, manifest_hash),
        });
    };
    if !crate::governance::ports::capability_subset(requires, &grant.allowed) {
        return Decision::Deny(capability_denial(
            grant,
            tool,
            action,
            requires,
            host,
            manifest_hash,
        ));
    }
    Decision::Allow {
        grant_id: Some(grant.id.clone()),
    }
}

/// The `NoPage` union rule (ADR-0022 Decision 5 step 6, domain-less calls with non-empty
/// `requires`): allow iff ANY grant's `allowed` covers `requires`, attributed to the first such
/// grant; else deny rule `capability` attributed to the first grant; with zero grants, rule
/// `unmatched_domain` over `"(unknown)"`.
fn decide_no_page(
    grants: &[Grant],
    tool: &str,
    action: Option<&str>,
    requires: &[Capability],
    manifest_hash: &str,
) -> Decision {
    let Some(first) = grants.first() else {
        return Decision::Deny(unmatched_domain_denial("(unknown)", manifest_hash));
    };
    if let Some(grant) = grants
        .iter()
        .find(|g| crate::governance::ports::capability_subset(requires, &g.allowed))
    {
        return Decision::Allow {
            grant_id: Some(grant.id.clone()),
        };
    }
    Decision::Deny(capability_denial(
        first,
        tool,
        action,
        requires,
        "(unknown)",
        manifest_hash,
    ))
}

fn unmatched_domain_denial(domain: &str, manifest_hash: &str) -> Denial {
    let rule = "unmatched_domain".to_string();
    let denial_id = denial::denial_id(manifest_hash, "", &rule);
    let message = format!(
        "Denied ({denial_id}): no grant covers {domain}. Tool use is limited to domains your \
         policy grants. Give this denial id to your administrator if access to {domain} is \
         needed."
    );
    Denial {
        rule,
        grant_id: None,
        denial_id,
        domain: domain.to_string(),
        message,
    }
}

fn denied_domain_denial(grant: &Grant, domain: &str, manifest_hash: &str) -> Denial {
    let rule = "denied_domain".to_string();
    let denial_id = denial::denial_id(manifest_hash, &grant.id, &rule);
    let message = format!(
        "Denied ({denial_id}): {domain} is excluded by grant '{grant_id}': your policy denies \
         this site explicitly. Do not retry or work around this; ask the user or an \
         administrator if access is needed.",
        grant_id = grant.id
    );
    Denial {
        rule,
        grant_id: Some(grant.id.clone()),
        denial_id,
        domain: domain.to_string(),
        message,
    }
}

fn scheme_denial(scheme: &str, manifest_hash: &str) -> Denial {
    let rule = format!("scheme/{scheme}");
    let denial_id = denial::denial_id(manifest_hash, "", &rule);
    let message = format!(
        "Denied ({denial_id}): the URL scheme '{scheme}:' is not permitted under the active \
         policy. Only http and https pages can be automated."
    );
    Denial {
        rule,
        grant_id: None,
        denial_id,
        domain: String::new(),
        message,
    }
}

/// The denial for a call whose action directory lookup missed (`requires` returned `None`: an
/// unknown tool, or a `computer` call with a missing/unknown action). Under a manifest, an
/// unclassifiable call is never authorized. Public: [`crate::governance::dispatch
/// ::Governance::authorize`] (the caller) builds this BEFORE constructing a `DecisionRequest`,
/// since without a resolved `requires` set there is no request to build.
pub fn unknown_action_denial(tool: &str, action: Option<&str>, manifest_hash: &str) -> Denial {
    let rule = "unknown_action".to_string();
    let denial_id = denial::denial_id(manifest_hash, "", &rule);
    let label = crate::governance::ports::call_label(tool, action);
    let message = format!(
        "Denied ({denial_id}): no grant permits '{label}'. Give this denial id to your \
         administrator to request '{label}'."
    );
    Denial {
        rule,
        grant_id: None,
        denial_id,
        domain: "(unknown)".to_string(),
        message,
    }
}

fn capability_denial(
    grant: &Grant,
    tool: &str,
    action: Option<&str>,
    requires: &[Capability],
    domain: &str,
    manifest_hash: &str,
) -> Denial {
    let rule = "capability".to_string();
    let denial_id = denial::denial_id(manifest_hash, &grant.id, &rule);
    let label = crate::governance::ports::call_label(tool, action);
    let missing = requires
        .iter()
        .find(|c| !grant.allowed.contains(c))
        .expect("capability_denial is only called when the subset check failed")
        .as_str();
    let allowed = if grant.allowed.is_empty() {
        "no capabilities".to_string()
    } else {
        grant
            .allowed
            .iter()
            .map(Capability::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let message = format!(
        "'{label}' needs the '{missing}' capability on {domain}, and grant '{grant_id}' allows \
         {allowed}. Give this denial id to your administrator to request '{missing}' access.",
        grant_id = grant.id
    );
    let message = format!("Denied ({denial_id}): {message}");
    Denial {
        rule,
        grant_id: Some(grant.id.clone()),
        denial_id,
        domain: domain.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::document::HostRules;

    /// A stand-in for the real ADR-0022 Decision 4 evaluator: exact string equality, or a `*.`
    /// prefix meaning "any host ending in `.suffix`", or the bare `*` token -- just enough
    /// grammar for these pure tests, never the authoritative grammar (that lives in
    /// `browser::polarity`'s own exhaustive tests).
    fn stub_evaluate_host(host: &str, allow: &[String], deny: &[String]) -> HostRuleOutcome {
        fn matches(pattern: &str, host: &str) -> bool {
            pattern == "*"
                || match pattern.strip_prefix("*.") {
                    Some(suffix) => host.ends_with(&format!(".{suffix}")),
                    None => pattern == host,
                }
        }
        let allowed = allow.iter().any(|p| matches(p, host));
        let denied = deny.iter().any(|p| matches(p, host));
        match (allowed, denied) {
            (false, false) => HostRuleOutcome::Unmatched,
            (true, false) => HostRuleOutcome::Allowed,
            (false, true) => HostRuleOutcome::Denied,
            (true, true) => HostRuleOutcome::Denied,
        }
    }

    fn grant(id: &str, allow_hosts: &[&str], allowed: &[Capability]) -> Grant {
        Grant {
            id: id.to_string(),
            hosts: HostRules {
                allow: allow_hosts.iter().map(|d| d.to_string()).collect(),
                deny: Vec::new(),
            },
            allowed: allowed.to_vec(),
            description: None,
            mode: None,
        }
    }

    fn grant_with_deny(id: &str, allow_hosts: &[&str], deny_hosts: &[&str]) -> Grant {
        Grant {
            id: id.to_string(),
            hosts: HostRules {
                allow: allow_hosts.iter().map(|d| d.to_string()).collect(),
                deny: deny_hosts.iter().map(|d| d.to_string()).collect(),
            },
            allowed: vec![Capability::Read],
            description: None,
            mode: None,
        }
    }

    /// The pre-g15-style convenience wrapper: always `manifest_mode: None, config_mode:
    /// Enforce`, so every test asserting Deny does so unshadowed. Tests that specifically
    /// exercise the g15 mode switch use [`check_with_mode`] instead.
    fn check(
        grants: &[Grant],
        tool: &str,
        requires: &[Capability],
        resource: &GoverningResource,
    ) -> Decision {
        check_with_mode(
            grants,
            tool,
            requires,
            resource,
            None,
            EffectiveMode::Enforce,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn check_with_mode(
        grants: &[Grant],
        tool: &str,
        requires: &[Capability],
        resource: &GoverningResource,
        manifest_mode: Option<EffectiveMode>,
        config_mode: EffectiveMode,
    ) -> Decision {
        check_call(
            grants,
            tool,
            None,
            requires,
            resource,
            "hash",
            stub_evaluate_host,
            manifest_mode,
            config_mode,
        )
    }

    fn host(h: &str) -> GoverningResource {
        GoverningResource::Resource(h.to_string())
    }

    #[test]
    fn requires_empty_short_circuits_before_any_grant_walk() {
        // A grant slice whose evaluation would deny (no domain covers example.com) -- if the
        // short-circuit did not run BEFORE the grant walk, this would deny.
        let grants = vec![grant("g", &["other.com"], &[])];
        assert_eq!(
            check(&grants, "tabs_create_mcp", &[], &host("example.com")),
            Decision::Allow { grant_id: None }
        );
        assert_eq!(
            check(&grants, "update_plan", &[], &GoverningResource::None),
            Decision::Allow { grant_id: None }
        );
        // Even with zero grants at all.
        assert_eq!(
            check(&[], "tabs_create_mcp", &[], &host("evil.com")),
            Decision::Allow { grant_id: None }
        );
    }

    #[test]
    fn subset_containment_allow_and_deny_per_capability() {
        let read_grant = vec![grant("r", &["example.com"], &[Capability::Read])];
        let all_grant = vec![grant(
            "a",
            &["example.com"],
            &[Capability::Read, Capability::Action, Capability::Write],
        )];

        match check(
            &read_grant,
            "form_input",
            &[Capability::Write],
            &host("example.com"),
        ) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "capability");
                assert_eq!(d.grant_id.as_deref(), Some("r"));
            }
            other => panic!("expected capability deny, got {other:?}"),
        }
        assert!(matches!(
            check(
                &read_grant,
                "read_page",
                &[Capability::Read],
                &host("example.com")
            ),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            check(
                &all_grant,
                "form_input",
                &[Capability::Write],
                &host("example.com")
            ),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            check(
                &all_grant,
                "computer",
                &[Capability::Action],
                &host("example.com")
            ),
            Decision::Allow { .. }
        ));
    }

    #[test]
    fn denied_domain_attribution_is_the_first_denying_grant() {
        let grants = vec![
            grant_with_deny("first", &["*"], &["evil.com"]),
            grant_with_deny("second", &["*"], &["evil.com"]),
        ];
        match check(&grants, "read_page", &[Capability::Read], &host("evil.com")) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "denied_domain");
                assert_eq!(d.grant_id.as_deref(), Some("first"));
                assert!(d.message.contains("evil.com"));
                assert!(d.message.contains("first"));
            }
            other => panic!("expected denied_domain, got {other:?}"),
        }
    }

    #[test]
    fn unmatched_vs_denied_precedence() {
        // No grant mentions the host at all: unmatched_domain, no grant id.
        let grants = vec![grant("g1", &["example.com"], &[Capability::Read])];
        match check(&grants, "read_page", &[Capability::Read], &host("evil.com")) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "unmatched_domain");
                assert_eq!(d.grant_id, None);
            }
            other => panic!("expected unmatched_domain, got {other:?}"),
        }

        // A grant's deny matches but no grant's allow ever does: denied_domain, attributed.
        let deny_only = vec![grant_with_deny("d1", &["*"], &["evil.com"])];
        match check(
            &deny_only,
            "read_page",
            &[Capability::Read],
            &host("evil.com"),
        ) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "denied_domain");
                assert_eq!(d.grant_id.as_deref(), Some("d1"));
            }
            other => panic!("expected denied_domain, got {other:?}"),
        }
    }

    #[test]
    fn no_page_union_rule_including_zero_grants() {
        let read_grant = vec![grant("r1", &["example.com"], &[Capability::Read])];
        assert!(matches!(
            check(
                &read_grant,
                "tabs_context_mcp",
                &[Capability::Read],
                &GoverningResource::None
            ),
            Decision::Allow { grant_id: Some(ref g) } if g == "r1"
        ));

        let write_only = vec![grant("w1", &["example.com"], &[Capability::Write])];
        match check(
            &write_only,
            "tabs_context_mcp",
            &[Capability::Read],
            &GoverningResource::None,
        ) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "capability");
                assert_eq!(d.grant_id.as_deref(), Some("w1"));
            }
            other => panic!("expected capability deny, got {other:?}"),
        }

        // Zero grants: unmatched_domain over "(unknown)".
        match check(
            &[],
            "tabs_context_mcp",
            &[Capability::Read],
            &GoverningResource::None,
        ) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "unmatched_domain");
                assert_eq!(d.domain, "(unknown)");
                assert_eq!(d.grant_id, None);
            }
            other => panic!("expected unmatched_domain, got {other:?}"),
        }
    }

    #[test]
    fn scheme_and_about_blank() {
        let grants = vec![grant(
            "g",
            &["example.com"],
            &[Capability::Read, Capability::Action, Capability::Write],
        )];
        for scheme in ["chrome", "file", "javascript"] {
            match check(
                &grants,
                "navigate",
                &[Capability::Read],
                &GoverningResource::OutOfScope(scheme.to_string()),
            ) {
                Decision::Deny(d) => assert_eq!(d.rule, format!("scheme/{scheme}")),
                other => panic!("expected scheme deny, got {other:?}"),
            }
        }
        assert_eq!(
            check(
                &grants,
                "navigate",
                &[Capability::Read],
                &GoverningResource::AlwaysAllow
            ),
            Decision::Allow { grant_id: None }
        );
    }

    #[test]
    fn unknown_fails_closed() {
        let grants = vec![grant(
            "g",
            &["example.com"],
            &[Capability::Read, Capability::Action, Capability::Write],
        )];
        match check(
            &grants,
            "read_page",
            &[Capability::Read],
            &GoverningResource::Indeterminate,
        ) {
            Decision::Deny(d) => {
                assert_eq!(d.rule, "unmatched_domain");
                assert_eq!(d.grant_id, None);
            }
            other => panic!("expected unmatched_domain deny, got {other:?}"),
        }
    }

    #[test]
    fn unknown_action_denies_via_unknown_action_rule() {
        let denial = unknown_action_denial("no_such_tool", None, "hash");
        assert_eq!(denial.rule, "unknown_action");
        assert_eq!(denial.grant_id, None);
        assert!(denial.message.starts_with("Denied (D-"));
    }

    #[test]
    fn capability_denial_message_is_exact() {
        let g = grant("r", &["example.com"], &[Capability::Read]);
        let denial = capability_denial(
            &g,
            "form_input",
            None,
            &[Capability::Write],
            "example.com",
            "hash",
        );
        let expected_tail = "'form_input' needs the 'write' capability on example.com, and \
             grant 'r' allows read. Give this denial id to your administrator to request \
             'write' access.";
        assert!(
            denial.message.ends_with(expected_tail),
            "{}",
            denial.message
        );
        assert!(denial.message.starts_with("Denied (D-"));
    }

    #[test]
    fn capability_denial_no_capabilities_wording() {
        let g = grant("r", &["example.com"], &[]);
        let denial = capability_denial(
            &g,
            "read_page",
            None,
            &[Capability::Read],
            "example.com",
            "hash",
        );
        assert!(
            denial.message.contains("allows no capabilities."),
            "{}",
            denial.message
        );
    }

    #[test]
    fn denied_domain_message_is_exact() {
        let g = grant_with_deny("d1", &["*"], &["evil.com"]);
        let denial = denied_domain_denial(&g, "evil.com", "hash");
        let expected_tail = "evil.com is excluded by grant 'd1': your policy denies this site \
             explicitly. Do not retry or work around this; ask the user or an administrator if \
             access is needed.";
        assert!(
            denial.message.ends_with(expected_tail),
            "{}",
            denial.message
        );
        assert!(denial.message.starts_with("Denied (D-"));
    }

    // --- g15: shadow enforcement (the mode switch) ---

    #[test]
    fn effective_mode_precedence_covers_every_combination() {
        assert_eq!(
            effective_mode(
                Some(EffectiveMode::Observe),
                Some(EffectiveMode::Enforce),
                EffectiveMode::Enforce
            ),
            EffectiveMode::Observe
        );
        assert_eq!(
            effective_mode(
                Some(EffectiveMode::Enforce),
                Some(EffectiveMode::Observe),
                EffectiveMode::Observe
            ),
            EffectiveMode::Enforce
        );
        assert_eq!(
            effective_mode(None, Some(EffectiveMode::Observe), EffectiveMode::Enforce),
            EffectiveMode::Observe
        );
        assert_eq!(
            effective_mode(None, Some(EffectiveMode::Enforce), EffectiveMode::Observe),
            EffectiveMode::Enforce
        );
        assert_eq!(
            effective_mode(None, None, EffectiveMode::Observe),
            EffectiveMode::Observe
        );
        assert_eq!(
            effective_mode(None, None, EffectiveMode::Enforce),
            EffectiveMode::Enforce
        );
    }

    #[test]
    fn mode_switch_yields_shadow_deny_under_observe_with_the_identical_grant_and_denial_id() {
        let read_grant = vec![grant("r", &["example.com"], &[Capability::Read])];

        let enforce = check_with_mode(
            &read_grant,
            "form_input",
            &[Capability::Write],
            &host("example.com"),
            None,
            EffectiveMode::Enforce,
        );
        let observe = check_with_mode(
            &read_grant,
            "form_input",
            &[Capability::Write],
            &host("example.com"),
            None,
            EffectiveMode::Observe,
        );
        match (enforce, observe) {
            (Decision::Deny(d_enforce), Decision::ShadowDeny(d_observe)) => {
                assert_eq!(d_enforce.rule, "capability");
                assert_eq!(d_enforce.grant_id, d_observe.grant_id);
                assert_eq!(d_enforce.denial_id, d_observe.denial_id);
            }
            other => panic!("expected (Deny, ShadowDeny) for the capability rule, got {other:?}"),
        }

        let enforce = check_with_mode(
            &read_grant,
            "form_input",
            &[Capability::Write],
            &host("evil.com"),
            None,
            EffectiveMode::Enforce,
        );
        let observe = check_with_mode(
            &read_grant,
            "form_input",
            &[Capability::Write],
            &host("evil.com"),
            None,
            EffectiveMode::Observe,
        );
        match (enforce, observe) {
            (Decision::Deny(d_enforce), Decision::ShadowDeny(d_observe)) => {
                assert_eq!(d_enforce.rule, "unmatched_domain");
                assert_eq!(d_enforce.grant_id, None);
                assert_eq!(d_enforce.grant_id, d_observe.grant_id);
                assert_eq!(d_enforce.denial_id, d_observe.denial_id);
            }
            other => panic!("expected (Deny, ShadowDeny) for unmatched_domain, got {other:?}"),
        }
    }

    #[test]
    fn mode_switch_never_touches_an_allow() {
        let all_grant = vec![grant(
            "a",
            &["example.com"],
            &[Capability::Read, Capability::Action, Capability::Write],
        )];
        let observe = check_with_mode(
            &all_grant,
            "form_input",
            &[Capability::Write],
            &host("example.com"),
            None,
            EffectiveMode::Observe,
        );
        assert!(matches!(observe, Decision::Allow { .. }));
    }

    #[test]
    fn grant_level_mode_overrides_manifest_and_config() {
        let mut observe_grant = grant("g", &["example.com"], &[Capability::Read]);
        observe_grant.mode = Some(EffectiveMode::Observe);
        let decision = check_with_mode(
            &[observe_grant],
            "form_input",
            &[Capability::Write],
            &host("example.com"),
            Some(EffectiveMode::Enforce),
            EffectiveMode::Enforce,
        );
        assert!(
            matches!(decision, Decision::ShadowDeny(_)),
            "the grant's own observe mode must win over an enforcing manifest and config: {decision:?}"
        );
    }

    #[test]
    fn unclassifiable_call_goes_through_the_same_mode_switch() {
        let denial = unknown_action_denial("no_such_tool", None, "hash");
        let grants: Vec<Grant> = Vec::new();
        let shadowed = apply_mode(
            Decision::Deny(denial.clone()),
            &grants,
            None,
            EffectiveMode::Observe,
        );
        assert!(matches!(shadowed, Decision::ShadowDeny(d) if d.denial_id == denial.denial_id));
        let enforced = apply_mode(
            Decision::Deny(denial),
            &grants,
            None,
            EffectiveMode::Enforce,
        );
        assert!(matches!(enforced, Decision::Deny(_)));
    }
}
