# managed-5 batch: LEDGER

Single source of truth for batch progress. Update after EVERY task (BOOTSTRAP step 5). A fresh
executor resumes from RESUME HERE with no other context.

## RESUME HERE

Batch authored 2026-07-10; red-team re-read against the live tree completed the same day (T1/T2/
T3/T8 verified aligned; T4 caller-integration corrected -- print loop, not a lines vec; T6
precondition corrected -- multiple denial render sites exist, append at the pipeline emission
chokepoint; T7 anchors verified exactly and pinned). BATCH COMPLETE: T1 (5a02aaa), T2 (c395c42),
T3 (3a64c8f), T4 (032ec27), T5 (bb17e5b), T6 (07b8cbc), T7 (f0cdd0f), T8 (2b748d2). All eight tasks
DONE, tree clean, every global gate green (cargo test --workspace 0 failures, clippy clean, lightbox
9/9 ok). No blocks. Only deviation: T5 dev-1 (freshness_phrase catch-all arm; harmless, unreachable
by the pinned tests). NEXT (owner-directed): report deviations, then the ADR-0056 D3 legacy-27 e2e
migration batch, then record the lightbox `up` demo.

## Status

| Task | Title | Status | Commit | Deviations |
| --- | --- | --- | --- | --- |
| T1 | Bundle `kind` discriminator | DONE | 5a02aaa | none |
| T2 | ManagedStatus sidecar (single writer in managed::activate) | DONE | c395c42 | none |
| T3 | Presentation validation (additive-only limits) | DONE | 3a64c8f | none |
| T4 | doctor managed line (reads the sidecar) | DONE | 032ec27 | none |
| T5 | explain-tool Policy Passport section | DONE | bb17e5b | 1 |
| T6 | Denials-as-doors: org contact line | DONE | 07b8cbc | none |
| T7 | Audit provenance: policy_seq on tool-call records | DONE | f0cdd0f | none |
| T8 | Lightbox scenarios: passport-freshness + sidecar-propagation | DONE | 2b748d2 | none |

Status values: `pending` | `in-progress` | `DONE` | `BLOCKED`.

## Log

One entry per task as it closes (or blocks). Number every deviation from the task file.

### T1 -- Bundle `kind` discriminator (5a02aaa)
- Preconditions verified: BundleClaims had exactly seq/manifest/presentation (no kind); BundleError
  had the 7 named variants; verify_bundle + sign_bundle present.
- Implemented per spec: `kind` field first in BundleClaims (serde default_kind), `default_kind()`
  beside the struct, `BundleError::Kind(String)`, kind check in verify_bundle after claims parse,
  `kind: default_kind()` in sign_bundle. Added a `ed_envelope_from_claims` test helper to forge
  legacy/unknown-kind claims the signer never mints.
- Tests `kind_defaults_to_policy_for_old_claims` + `unknown_kind_is_rejected` pass; all 10 bundle
  tests green. Global gates: workspace tests pass, clippy clean, lightbox 7/7 ok.
- Deviations: none.

### T2 -- ManagedStatus sidecar (c395c42)
- Preconditions verified: activate signature + resolve_managed call; Reconciled/Freshness/
  StaleReason/write_cache present in cache.rs; paths.managed_cache: Option<PathBuf>; chrono is a
  core dep with the clock feature.
- New crates/core/src/governance/managed/status.rs: ManagedStatus struct (v/freshness/stale_reason/
  seq/fetched_at/source/presentation/last_error), from_reconciled with the exact snake_case mapping,
  sidecar_path, write_sidecar (reuses cache::write_cache atomic temp+rename), read_sidecar (None on
  absent/garbage). `pub mod status;` added; activate now best-effort writes the sidecar after
  resolve_managed (warn-and-continue on failure).
- Tests: snake_case_mapping_is_exact, sidecar_round_trips, read_sidecar_absent_or_garbage_is_none;
  extended activate_resolves_a_configured_local_bundle to assert freshness=="fresh", seq==Some(4).
  31 managed tests green (default) + 29 green (--no-default-features air-gap; status.rs touches no
  ureq/rustls). Global gates: workspace tests pass, clippy clean, lightbox 7/7 ok.
- Deviations: none.

### T3 -- Presentation validation (3a64c8f)
- Preconditions verified: Presentation{org_name,rationale,contacts} + Contact{kind,value,label} in
  bundle.rs; verify_and_parse calls verify_bundle then parse_manifest.
- bundle.rs: pub fn validate_presentation with the exact limits (org_name<=120, rationale<=400,
  contacts<=8, kind<=32, value<=256, label<=120 via chars()) and a control-character sweep
  (c<'\u{20}') across every present string field, verbatim error strings. verify_bundle runs it on
  Some(presentation) after the T1 kind check, mapping Err(msg)->BundleError::Claims(msg).
- Tests: oversized_org_name_is_rejected, control_character_in_contact_is_rejected,
  valid_presentation_passes (bundle.rs); bad_presentation_update_keeps_last_known_good (cache.rs,
  seq-6 bad-presentation update refused -> LastKnownGood(UpdateRejected), active seq==5). 45 bundle+
  managed tests green. Global gates: workspace tests pass, clippy clean, lightbox 7/7 ok.
- Deviations: none.

### T4 -- doctor managed section (032ec27)
- Preconditions verified: T2 status::{read_sidecar,sidecar_path,ManagedStatus} exist; doctor.rs
  governance section uses `println!("  {:<9}...")` style and the caller is the confirmed print loop
  at ~78-81.
- doctor.rs: added `use ...status::{read_sidecar, sidecar_path, ManagedStatus}`; managed_section_lines
  (production paths, not-configured / no-data-dir / no-status / render arms) and pure
  render_managed_status producing the `  {:<9}` label lines (seq/freshness/reason/fetched, source,
  org, note). Caller: added a second print loop immediately after the governance loop (no lines vec;
  matches the pinned integration).
- Tests: managed_line_renders_fresh + managed_line_renders_guardian_door, both first-line-exact.
  Global gates: workspace tests pass, clippy clean, lightbox 7/7 ok.
- Deviations: none.

### T5 -- explain-tool Policy Passport (bb17e5b)
- Preconditions verified: T2 status:: items exist; the MCP explain TOOL's text is composed at the
  single Handler::Local closure in browser/directory.rs (~1283, emitting directory::explain_text());
  sacred_domains config key hits under governance.
- explain.rs: pub fn managed_passport(&ManagedStatus)->String (pure, trailing newline), exact lines
  per spec (active / Governed by / Policy version+freshness_phrase / Why / sacred line / Questions?).
  directory.rs explain handler: append "\n"+passport only when managed_bootstrap exists AND sidecar
  reads Some; else byte-identical (all_open_golden + advertised-set goldens stayed green).
- Tests: passport_renders_fresh (full-string exact) + passport_renders_guardian. Global gates:
  workspace tests pass (goldens green), clippy clean, lightbox 7/7 ok.
- Deviation 1: the freshness_phrase match has a catch-all arm ("enforcing your last verified policy
  from {fetched_at}" with no parenthetical) for states the spec does not enumerate (no_policy or an
  unknown freshness string). The four spec-defined arms (fresh + the three last_known_good reasons)
  are the reachable states when a policy is active; the catch-all only guards a degenerate sidecar
  and is never hit by the pinned tests.

### T6 -- denial org-contact door line (07b8cbc)
- Preconditions verified: T2 sidecar readable; the denial-emission chokepoint is
  `render_outcome`'s `CallOutcome::Denied { message, .. }` arm in mcp/pipeline.rs (~83) -- the single
  point where any denial message becomes tool-result text via `text_content(message)`. Chosen line
  site: pipeline.rs render_outcome Denied arm.
- denial.rs: pub fn org_contact_line(Option<&str>, &str)->String (pure, no trailing newline).
  pipeline.rs: with_org_contact_line reads the production-paths sidecar and appends "\n"+line only
  when bootstrap exists AND sidecar Some AND presentation has a non-empty contacts vec; else returns
  the message byte-identical (this file is outside src/governance/, so a7 does not constrain it).
- Tests: contact_line_with_org_name + contact_line_without_org_name; existing denial tests
  unchanged. tool_enforcement suite green (11 passed, denials there run with no managed bootstrap so
  strings are unchanged). Global gates: workspace tests pass, clippy clean, lightbox 7/7 ok.
- Deviations: none.

### T7 -- policy_seq audit provenance (f0cdd0f)
- Preconditions verified: set_license_stamp def at audit/mod.rs ~127 and hub call site at
  hub/mod.rs ~464 inside the governance_operational block; license_stamp field ~40, tool-call-only
  gate in write_serialized, four constructors. mcp/server.rs policy-subscription task has the
  concrete Arc<Recorder> in scope at the spawn site.
- audit/mod.rs: policy_seq: Mutex<Option<u64>> field (init None in all four constructors);
  set_policy_seq mirroring set_license_stamp; write_serialized now has a parallel tool-call-only seq
  gate and appends "policy_seq":<n> (fast byte-identical path when both stamp+seq are None; only-
  license case unchanged).
- Hub wiring (hub/mod.rs, inside governance_operational): for ManifestOrigin::Managed, read the T2
  sidecar and recorder.set_policy_seq(status.seq). Live wiring (server.rs policy-subscription): added
  a concrete seq_recorder = Arc::clone(&recorder) at the spawn site; on each change, Managed re-reads
  the sidecar and sets the seq, any other origin clears it (set_policy_seq(None)).
- Tests: policy_seq_stamps_tool_call_records_only ("policy_seq":6 on tool-call, absent on session
  event) + no_seq_no_field. all_open_golden + audit_recorder + every audit-shape test green (591 lib
  tests pass). Global gates: workspace tests pass, clippy clean, lightbox 7/7 ok.
- Deviations: none (the one-line concrete recorder clone in server.rs is the sanctioned addition
  named in the task precondition).

### T8 -- lightbox scenarios (2b748d2)
- Preconditions verified: T2..T7 DONE; scenarios.rs registry() + support helpers TempRoot/
  BundleServer/sign/manifest/write_bootstrap present; all consumed core items (managed_passport,
  status::{sidecar_path,read_sidecar,ManagedStatus}, bundle::{sign_bundle,Presentation,Contact}) are
  already pub -- no crates/core change needed.
- scenarios.rs: added `use ...managed::status;`; two registry entries. sidecar_propagation (serve
  seq-5 -> sidecar fresh/seq5; set_bundle seq-6 -> sidecar seq6; drop server -> sidecar
  last_known_good/source_unreachable/seq stays 6). passport_freshness (local presentation bundle ->
  activate -> read sidecar -> managed_passport contains "Governed by: Acme Security.", "Policy
  version 3,", the sacred line, and "security@acme.example").
- Verification: lightbox run --all = 9/9 ok; clippy -p ghostlight-lightbox clean. Global gates:
  workspace tests 0 failures, clippy clean.
- Deviations: none.
