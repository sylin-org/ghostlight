// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `form_fill` matcher (ADR-0036 Decision 1/8, PINS.md SS13): pure functions matching a
//! model-supplied field-key map to the form structure the content script reads (`formStructure`,
//! PINS.md SS12). No I/O; the only dependency beyond `serde`/`serde_json` is the mirrored
//! structure types below.
//!
//! Matching, pinned precisely because the failure mode of a fuzzy matcher on a Write-class tool
//! is writing into the WRONG field (ADR-0036 Decision 1):
//! - **Sources, in priority order:** `<label>` text, `placeholder`, `name`/`id` attribute,
//!   `aria-label`.
//! - **Tiers:** exact (after normalization) > prefix (source starts with the key) > substring
//!   (either direction contains the other). A higher tier always beats a lower tier regardless of
//!   source; source priority only breaks a tie WITHIN one tier.
//! - **Resolution order:** keys resolve most-specific-first (longest normalized key first); each
//!   control is consumed at most once.
//! - **Ambiguity is surfaced, never guessed:** a substring-only tie across distinct controls goes
//!   to `unmatched` with the tied candidates; an exact/prefix tie resolves to document order.
//!
//! Multi-form scoping (ADR-0036 Decision 8): each form (plus `formless`, when non-empty) is
//! scored independently (`2 * exact + 1 * other` matched keys); the highest-scoring pool wins
//! (ties favor the lower `formIndex`, with `formless` losing every tie against a real form).
//! Keys that match only in a non-winning pool are reported in `unmatched`.

use serde::Deserialize;

/// One control's identity fields (PINS.md SS12), mirrored from the `formStructure` content-script
/// read. No field VALUES: the matcher needs identity only, so secrets never enter this type.
#[derive(Debug, Clone, Deserialize)]
pub struct Control {
    #[serde(rename = "ref")]
    pub ref_id: String,
    #[serde(rename = "type")]
    pub control_type: String,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub name: Option<String>,
    pub id: Option<String>,
    #[serde(rename = "ariaLabel")]
    pub aria_label: Option<String>,
    pub disabled: bool,
    pub readonly: bool,
    /// The adapter's structural credential classification. `None` means the covered adapter did
    /// not provide the fact, so Ghostlight must fail closed before mutation.
    #[serde(default)]
    pub sensitive: Option<bool>,
}

/// One submit candidate (PINS.md SS12): a button/input the content script judged submit-like.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitCandidate {
    #[serde(rename = "ref")]
    pub ref_id: String,
    pub label: Option<String>,
    pub kind: String,
    #[serde(default)]
    pub disabled: bool,
}

/// One `<form>`'s controls and submit candidates, in document order (PINS.md SS12).
#[derive(Debug, Clone, Deserialize)]
pub struct Form {
    #[serde(rename = "formIndex")]
    pub form_index: usize,
    pub controls: Vec<Control>,
    pub submits: Vec<SubmitCandidate>,
}

/// The whole form-structure read (PINS.md SS12): forms plus controls outside any `<form>`.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct FormStructure {
    pub forms: Vec<Form>,
    pub formless: Vec<Control>,
}

/// A tied or near-miss candidate reported alongside an unmatched key (ADR-0036 Decision 1).
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub label: Option<String>,
    pub ref_id: String,
    pub control_type: String,
}

/// A matched control, carrying just what `form_fill`'s orchestration needs to dispatch the fill
/// and render the result (ADR-0036 Decision 3/6): the ref, the discovered type, and whether it is
/// disabled/readonly (never filled; reported as `skipped` by the caller instead).
#[derive(Debug, Clone, PartialEq)]
pub struct ControlRef {
    pub ref_id: String,
    pub control_type: String,
    pub disabled: bool,
    pub readonly: bool,
    pub sensitive: Option<bool>,
}

/// The outcome of [`match_fields`]: matched keys (each paired with the control it resolved to),
/// unmatched keys with their tied/near-miss candidates, and the form the match committed to
/// (`None` when the page has no `<form>` elements at all, so matching ran against `formless`
/// only).
#[derive(Debug, Default)]
pub struct MatchOutcome {
    pub matched: Vec<(String, ControlRef)>,
    pub unmatched: Vec<(String, Vec<Candidate>)>,
    pub form_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Substring,
    Prefix,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourcePriority {
    Label,
    Placeholder,
    NameOrId,
    AriaLabel,
}

/// Casefold + trim + collapse internal whitespace (ADR-0036 Decision 1).
fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The match tier between an already-normalized `source` and `key`, or `None` if they do not
/// match at all. `key` is expected pre-normalized by the caller (recomputing it per control per
/// source would be wasted work over the same key).
fn tier_for(source: &str, key: &str) -> Option<Tier> {
    if source == key {
        return Some(Tier::Exact);
    }
    if source.starts_with(key) {
        return Some(Tier::Prefix);
    }
    if source.contains(key) || key.contains(source) {
        return Some(Tier::Substring);
    }
    None
}

/// This control's sources, normalized, in source-priority order (skipping empty/absent fields).
fn control_sources(c: &Control) -> Vec<(SourcePriority, String)> {
    let mut v = Vec::new();
    if let Some(s) = c.label.as_deref().filter(|s| !s.trim().is_empty()) {
        v.push((SourcePriority::Label, normalize(s)));
    }
    if let Some(s) = c.placeholder.as_deref().filter(|s| !s.trim().is_empty()) {
        v.push((SourcePriority::Placeholder, normalize(s)));
    }
    if let Some(s) = c.name.as_deref().filter(|s| !s.trim().is_empty()) {
        v.push((SourcePriority::NameOrId, normalize(s)));
    }
    if let Some(s) = c.id.as_deref().filter(|s| !s.trim().is_empty()) {
        v.push((SourcePriority::NameOrId, normalize(s)));
    }
    if let Some(s) = c.aria_label.as_deref().filter(|s| !s.trim().is_empty()) {
        v.push((SourcePriority::AriaLabel, normalize(s)));
    }
    v
}

/// The best (tier, source priority) this control offers for an already-normalized `key`, or
/// `None` if no source matches at all. Tier always wins; source priority breaks a same-tier tie
/// (lower [`SourcePriority`] value wins).
fn best_match(control: &Control, key: &str) -> Option<(Tier, SourcePriority)> {
    let mut best: Option<(Tier, SourcePriority)> = None;
    for (prio, source) in control_sources(control) {
        let Some(tier) = tier_for(&source, key) else {
            continue;
        };
        best = match best {
            None => Some((tier, prio)),
            Some((bt, bp)) if tier > bt || (tier == bt && prio < bp) => Some((tier, prio)),
            Some(existing) => Some(existing),
        };
    }
    best
}

fn control_ref(c: &Control) -> ControlRef {
    ControlRef {
        ref_id: c.ref_id.clone(),
        control_type: c.control_type.clone(),
        disabled: c.disabled,
        readonly: c.readonly,
        sensitive: c.sensitive,
    }
}

fn candidate_of(c: &Control) -> Candidate {
    Candidate {
        label: c.label.clone(),
        ref_id: c.ref_id.clone(),
        control_type: c.control_type.clone(),
    }
}

/// One pool's (a form's, or `formless`'s) matching outcome plus the tier counts used for scoring
/// (ADR-0036 Decision 8).
struct PoolMatch {
    matched: Vec<(String, ControlRef)>,
    unmatched: Vec<(String, Vec<Candidate>)>,
    exact_count: usize,
    other_count: usize,
}

/// Match every key against one pool of controls, independently: longest-normalized-key-first,
/// single consumption per control, substring-only ties reported as unmatched with candidates,
/// exact/prefix ties resolved to document order (the lowest control index).
fn match_pool(keys: &[String], controls: &[Control]) -> PoolMatch {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(normalize(&keys[i]).chars().count()));

    let mut consumed = vec![false; controls.len()];
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    let mut exact_count = 0usize;
    let mut other_count = 0usize;

    for ki in order {
        let key_norm = normalize(&keys[ki]);
        let mut candidates: Vec<(usize, Tier, SourcePriority)> = Vec::new();
        for (ci, c) in controls.iter().enumerate() {
            if consumed[ci] {
                continue;
            }
            if let Some((tier, prio)) = best_match(c, &key_norm) {
                candidates.push((ci, tier, prio));
            }
        }

        if candidates.is_empty() {
            unmatched.push((keys[ki].clone(), Vec::new()));
            continue;
        }

        let max_tier = candidates.iter().map(|(_, t, _)| *t).max().unwrap();
        let mut top: Vec<&(usize, Tier, SourcePriority)> = candidates
            .iter()
            .filter(|(_, t, _)| *t == max_tier)
            .collect();

        if top.len() > 1 && max_tier == Tier::Substring {
            let cands: Vec<Candidate> = top
                .iter()
                .map(|(ci, _, _)| candidate_of(&controls[*ci]))
                .collect();
            unmatched.push((keys[ki].clone(), cands));
            continue;
        }

        // A single candidate, or an exact/prefix tie: resolve to document order (lowest index).
        top.sort_by_key(|(ci, _, _)| *ci);
        let (ci, tier, _) = *top[0];
        consumed[ci] = true;
        matched.push((keys[ki].clone(), control_ref(&controls[ci])));
        if tier == Tier::Exact {
            exact_count += 1;
        } else {
            other_count += 1;
        }
    }

    PoolMatch {
        matched,
        unmatched,
        exact_count,
        other_count,
    }
}

/// Match `keys` (the model-supplied field labels) against `structure` (ADR-0036 Decision 1/8).
pub fn match_fields(keys: &[String], structure: &FormStructure) -> MatchOutcome {
    if structure.forms.is_empty() {
        let pm = match_pool(keys, &structure.formless);
        return MatchOutcome {
            matched: pm.matched,
            unmatched: pm.unmatched,
            form_index: None,
        };
    }

    // Score: one candidate pool per form, plus `formless` when non-empty (Decision 8).
    let mut scored: Vec<(Option<usize>, PoolMatch, usize)> = Vec::new();
    for form in &structure.forms {
        let pm = match_pool(keys, &form.controls);
        let score = 2 * pm.exact_count + pm.other_count;
        scored.push((Some(form.form_index), pm, score));
    }
    if !structure.formless.is_empty() {
        let pm = match_pool(keys, &structure.formless);
        let score = 2 * pm.exact_count + pm.other_count;
        scored.push((None, pm, score));
    }

    scored.sort_by(|a, b| {
        b.2.cmp(&a.2).then_with(|| match (a.0, b.0) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        })
    });

    let (winner_index, winner_pool, _) = scored.remove(0);
    MatchOutcome {
        matched: winner_pool.matched,
        unmatched: winner_pool.unmatched,
        form_index: winner_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control(ref_id: &str, label: Option<&str>, name: Option<&str>) -> Control {
        Control {
            ref_id: ref_id.to_string(),
            control_type: "text".to_string(),
            label: label.map(str::to_string),
            placeholder: None,
            name: name.map(str::to_string),
            id: None,
            aria_label: None,
            disabled: false,
            readonly: false,
            sensitive: Some(false),
        }
    }

    fn one_form(controls: Vec<Control>) -> FormStructure {
        FormStructure {
            forms: vec![Form {
                form_index: 0,
                controls,
                submits: Vec::new(),
            }],
            formless: Vec::new(),
        }
    }

    #[test]
    fn specificity_and_single_consumption() {
        let structure = one_form(vec![
            control("ref_1", Some("Email Address"), Some("email")),
            control("ref_2", Some("Confirm Email Address"), None),
            control("ref_3", Some("Password"), None),
            control("ref_4", Some("Confirm Password"), None),
        ]);
        let keys = vec![
            "Confirm Password".to_string(),
            "Password".to_string(),
            "Email".to_string(),
        ];
        let outcome = match_fields(&keys, &structure);
        assert!(
            outcome.unmatched.is_empty(),
            "unmatched: {:?}",
            outcome.unmatched
        );
        let by_key: std::collections::HashMap<&str, &str> = outcome
            .matched
            .iter()
            .map(|(k, c)| (k.as_str(), c.ref_id.as_str()))
            .collect();
        assert_eq!(by_key["Confirm Password"], "ref_4");
        assert_eq!(by_key["Password"], "ref_3");
        assert_eq!(by_key["Email"], "ref_1");
    }

    #[test]
    fn substring_tie_goes_unmatched() {
        let structure = one_form(vec![
            control("ref_7", Some("First name"), None),
            control("ref_8", Some("Last name"), None),
        ]);
        let keys = vec!["name".to_string()];
        let outcome = match_fields(&keys, &structure);
        assert!(outcome.matched.is_empty(), "matched: {:?}", outcome.matched);
        assert_eq!(outcome.unmatched.len(), 1);
        let (key, candidates) = &outcome.unmatched[0];
        assert_eq!(key, "name");
        let refs: Vec<&str> = candidates.iter().map(|c| c.ref_id.as_str()).collect();
        assert_eq!(refs, vec!["ref_7", "ref_8"]);
    }

    #[test]
    fn exact_on_name_attr_beats_prefix_on_label() {
        let structure = one_form(vec![
            control("A", Some("Email Address"), None),
            control("B", None, Some("email")),
        ]);
        let keys = vec!["email".to_string()];
        let outcome = match_fields(&keys, &structure);
        assert_eq!(outcome.matched.len(), 1);
        assert_eq!(outcome.matched[0].1.ref_id, "B");
    }

    #[test]
    fn form_scoring_picks_majority_form() {
        let structure = FormStructure {
            forms: vec![
                Form {
                    form_index: 0,
                    controls: vec![
                        control("ref_1", Some("Email"), None),
                        control("ref_2", Some("Password"), None),
                    ],
                    submits: Vec::new(),
                },
                Form {
                    form_index: 1,
                    controls: vec![control("ref_3", Some("Search Box"), None)],
                    submits: Vec::new(),
                },
            ],
            formless: Vec::new(),
        };
        // "Email" and "Password" match EXACTLY in form 0 (score 4); "Box" matches only via
        // substring in form 1 (score 1). Form 0 wins; "Box" (unavailable there) is unmatched.
        let keys = vec![
            "Email".to_string(),
            "Password".to_string(),
            "Box".to_string(),
        ];
        let outcome = match_fields(&keys, &structure);
        assert_eq!(outcome.form_index, Some(0));
        let matched_keys: Vec<&str> = outcome.matched.iter().map(|(k, _)| k.as_str()).collect();
        assert!(matched_keys.contains(&"Email"));
        assert!(matched_keys.contains(&"Password"));
        assert!(
            outcome.unmatched.iter().any(|(k, _)| k == "Box"),
            "unmatched: {:?}",
            outcome.unmatched
        );
    }
}
