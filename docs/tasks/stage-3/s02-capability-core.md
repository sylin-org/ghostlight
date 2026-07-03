# S02: capability vocabulary in the governance core

## Goal

Add the ADR-0022 Decision 1 capability taxonomy to the domain-agnostic governance core as a
pure, additive type: the `Capability` enum, its name helpers, and the subset-containment
helper that later enforcement builds on. Nothing consumes any of it in this task (s05 wires
it into grants and enforcement); `RwClass` is untouched and stays fully in force (s06
deletes it). After this task the tree behaves byte-identically: same tests, same binary
output, plus three new unit tests over the new type.

## Authority

ADR-0022 (`docs/adr/0022-intent-calibrated-capabilities.md`) is normative; where this
prompt and the ADR disagree, THE ADR WINS (record the deviation in the ledger). Decision 1
defines the taxonomy, the lowercase wire vocabulary, the "action is NOT a weaker write"
rule, and the "execute is never implied" rule that this task encodes.

## Depends on

s01 (BOOTSTRAP sequence order). There is NO compile-time dependency: this task only adds
new items to `src/governance/ports.rs`, which s01 does not touch. But BOOTSTRAP order is
absolute: if `docs/tasks/stage-3/LEDGER.md` RESUME HERE does not show s01 committed, STOP
and execute s01 first.

## Current behavior (verify against the tree before editing)

- `src/governance/ports.rs` exists and is registered as `pub mod ports;` in
  `src/governance/mod.rs`. The crate has a lib target (`src/lib.rs` declares
  `pub mod governance;`), so new `pub` items raise no dead-code warnings while unconsumed.
- House style for small axis enums, set by `RwClass` and `EffectiveMode` in that file:
  `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` plus a
  `#[serde(rename_all = ...)]` wire form; a `pub fn as_str(&self) -> &'static str`
  returning the bare wire string so callers do not round-trip through `serde_json`; parse
  helpers on the type itself (`EffectiveMode::from_config_str`); unit tests in the
  `#[cfg(test)] mod tests` module at the bottom of the same file (it opens with
  `use super::*;`), including `rw_and_mode_wire_names_are_lowercase`.
- No type named `Capability` exists anywhere under `src/` (the only occurrence of the word
  is a CLI doc comment in `src/main.rs` about the manifest source; unrelated).
- `tests/architecture.rs` scans the raw text of every `.rs` file under `src/governance/`
  (comments and string literals included) and fails on `crate::browser`,
  `crate::transport`, `crate::mcp`, `crate::native` path tokens or `url`-crate references.

## Required behavior

### 1. The `Capability` enum and helpers

Add to `src/governance/ports.rs`, in the "Supporting placeholder and axis types" section,
immediately after the `impl EffectiveMode` block and before the `ToolId` newtype.
Transcribe exactly (doc comments included; they carry ADR-mandated statements):

    /// One capability primitive of the ADR-0022 Decision 1 taxonomy. Capabilities classify
    /// an operation by EPISTEMIC STATUS -- what the governor can PROVE about it -- never by
    /// its (unknowable) downstream effect. `Read` is provably retrieval/observation only;
    /// `Action` dispatches UI input whose effect is page-determined and unknowable; `Write`
    /// is a declared mutation; `Execute` is unbounded arbitrary code. `Action` is NOT a
    /// weaker `Write`: it encompasses the ability to CAUSE writes (a click can submit a
    /// form). `Execute` is never implied by any other capability. Capabilities are
    /// independent primitives, not ordered tiers. Wire/file names are lowercase: `"read"`,
    /// `"action"`, `"write"`, `"execute"`. Nothing consumes this type yet: s05 wires it
    /// into grants and enforcement; until then `RwClass` remains the classification in
    /// force.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Capability {
        Read,
        Action,
        Write,
        Execute,
    }

    impl Capability {
        /// The wire/file vocabulary (ADR-0022 Decision 1): exactly `"read"`, `"action"`,
        /// `"write"`, or `"execute"`. Matches the `#[serde(rename_all = "lowercase")]`
        /// form, provided directly so callers do not round-trip through `serde_json` for
        /// the bare string.
        pub fn as_str(&self) -> &'static str {
            match self {
                Capability::Read => "read",
                Capability::Action => "action",
                Capability::Write => "write",
                Capability::Execute => "execute",
            }
        }

        /// Parse one capability name. Exact lowercase only: any other casing, whitespace,
        /// or unknown word returns `None` (fail closed; the wire vocabulary is lowercase).
        pub fn from_name(name: &str) -> Option<Capability> {
            match name {
                "read" => Some(Capability::Read),
                "action" => Some(Capability::Action),
                "write" => Some(Capability::Write),
                "execute" => Some(Capability::Execute),
                _ => None,
            }
        }
    }

    /// True iff every element of `requires` appears in `allowed` -- the subset containment
    /// that enforcement evaluates (ADR-0022 Decision 3). An empty `requires` is a subset of
    /// everything, including an empty `allowed`. Duplicates in either slice do not change
    /// the result. No capability implies another: `Execute` in `requires` is satisfied only
    /// by `Execute` in `allowed`.
    pub fn capability_subset(requires: &[Capability], allowed: &[Capability]) -> bool {
        requires.iter().all(|r| allowed.contains(r))
    }

### 2. Nothing else changes

Do NOT add imports elsewhere, do NOT re-export from `mod.rs` or `lib.rs`, do NOT touch
`RwClass`, `EffectiveMode`, `DecisionRequest`, or any other existing item. `pub mod ports;`
already exposes the new items at `crate::governance::ports::Capability` and
`crate::governance::ports::capability_subset`; s05 imports them from there.

## Constraints

1. Additive only: the ONLY source file modified is `src/governance/ports.rs` (plus the
   LEDGER update per BOOTSTRAP rule 6). `git diff --stat` before committing must show
   exactly those files, and the `ports.rs` diff must contain only added lines -- every
   existing line byte-unchanged.
2. `tests/architecture.rs` must stay green trivially: the added code and doc comments name
   none of the forbidden tokens (they do not; transcribe as given).
3. All existing tests keep passing unmodified; `tests/tool_schema_fidelity.rs`,
   `tests/all_open_golden.rs`, and `tests/mcp_protocol.rs` are untouched and green.
4. ASCII only; no new dependencies; one commit, message:
   `feat(governance): s02 capability vocabulary in the governance core`.

## Tests (minimum)

Append these three tests to the existing `#[cfg(test)] mod tests` module at the bottom of
`src/governance/ports.rs` (it already has `use super::*;`), with exactly these names:

1. `capability_wire_names_round_trip` -- for each pair (`Capability::Read`, `"read"`),
   (`Capability::Action`, `"action"`), (`Capability::Write`, `"write"`),
   (`Capability::Execute`, `"execute"`): `serde_json::to_string(&cap).unwrap()` equals the
   JSON-quoted name (for example `"\"read\""`); `serde_json::from_str::<Capability>` of the
   JSON-quoted name yields the variant; `cap.as_str()` equals the name;
   `Capability::from_name(name)` equals `Some(cap)`.
2. `capability_from_name_rejects_unknown_and_case_variants` -- `Capability::from_name`
   returns `None` for each of exactly these five inputs: `"Read"`, `"READ"`, `""`, `"all"`,
   `"observe"`.
3. `capability_subset_truth_table` -- assert each of the following, in this order:
   - `capability_subset(&[], &[])` is true (empty is a subset of empty);
   - `capability_subset(&[], &[Capability::Read, Capability::Action, Capability::Write,
     Capability::Execute])` is true;
   - `capability_subset(&[Capability::Read], &[Capability::Read])` is true;
   - `capability_subset(&[Capability::Read], &[Capability::Action, Capability::Write])` is
     false;
   - `capability_subset(&[Capability::Execute], &[Capability::Read, Capability::Action,
     Capability::Write])` is false (execute is never implied);
   - `capability_subset(&[Capability::Execute], &[])` is false;
   - `capability_subset(&[Capability::Action, Capability::Write], &[Capability::Read,
     Capability::Action, Capability::Write])` is true;
   - `capability_subset(&[Capability::Action, Capability::Write], &[Capability::Action])`
     is false;
   - `capability_subset(&[Capability::Read, Capability::Read], &[Capability::Read])` is
     true (duplicate in requires);
   - `capability_subset(&[Capability::Read], &[Capability::Read, Capability::Read])` is
     true (duplicate in allowed).

## Verification

`cargo fmt` then `cargo fmt --check` clean; `cargo clippy --all-targets -- -D warnings`
clean; `cargo test` green, including the three new tests and the unchanged
`tests/architecture.rs`; ASCII scan clean
(`rg -n "[^\x00-\x7F]" src/governance/ports.rs` prints nothing). No browser verification
applies (a pure type addition); append NO BROWSER-TESTS.md entry for this task.

## Out of scope

- The action directory and its per-action `requires` table (s03).
- Host polarity (s04).
- Grant shape, enforcement, advertisement, `DecisionRequest` changes, or ANY consumption of
  `Capability` (s05).
- Deleting or altering `RwClass`, `browser/classify.rs`, or the audit `rw` field (s06).
- The `explain` tool and any `tools.json` change (s07).
- Documentation outside this file's code and doc comments (s08).
