# L02: disk resolution and the Recorder license stamp

## Goal

ADR-0028 Decision 3's two mechanical halves: resolve the license file from its two pinned
locations, and give the audit Recorder the appended-`license`-field stamp mechanism.
Still no wiring (l04 connects them at startup).

## Authority

ADR-0028 Decision 3; 00-design.md "Disk resolution (l02)" and "Recorder stamp (l02)".

## Depends on

l01 (the license module exists). STOP preconditions:
`rg -n "pub fn resolve_bytes" src/governance/license.rs` matches;
`rg -n "fn write_serialized" src/governance/audit/mod.rs` matches;
`rg -n "license_stamp" src/governance/audit/mod.rs` prints nothing. If any fails, STOP.

## Current behavior (verified 2026-07-03 pre-batch; locate by content, line numbers are advisory)

- src/governance/config/load.rs defines `pub fn org_policy_path() -> std::path::PathBuf`
  (fixed per platform: `%ProgramData%\ghostlight\policy.json` on Windows,
  `/etc/ghostlight/policy.json` on Linux) and
  `pub fn user_config_path() -> Option<PathBuf>` returning
  `dirs::config_dir()?/ghostlight/config.json`.
- src/governance/audit/mod.rs: `pub struct Recorder { inner: Mutex<Option<Inner>> }` with
  constructors `from_config`, `disabled`, `to_file`, `to_stderr`; `fn write_serialized`
  serializes via `serde_json::to_string(record)` then dispatches on `Inner`
  (File/Stderr/Syslog arms), warn-and-swallow on failure.
- serde_json is built with `preserve_order` (Cargo.toml), so `serde_json::Value` objects
  preserve insertion order and an inserted key lands last.
- tests/audit_recorder.rs pins the serialized record shape for UNSTAMPED records and is
  never-touch: it must stay green without modification.

## Required behavior

### 1. license.rs disk resolution (append to the module; l02 owns this addition)

Implement exactly the three functions pinned in 00-design.md "Disk resolution (l02)":
`org_license_path`, `user_license_path`, `resolve_from_disk`. Read failures on an
existing file resolve to `Invalid(format!("unreadable license file: {e}"))` with the
path still returned.

### 2. Recorder stamp (src/governance/audit/mod.rs; sole owner in this batch)

Per 00-design.md "Recorder stamp (l02)":

- Add field `license_stamp: Mutex<Option<&'static str>>` to `Recorder`, initialized
  `None` in ALL FOUR constructors.
- Add `pub fn set_license_stamp(&self, stamp: Option<&'static str>)` with a doc comment
  citing ADR-0028 Decision 3.
- Rework `write_serialized`: `serde_json::to_value(record)` instead of `to_string`; when
  the stored stamp is `Some(s)`, insert `("license", s)` into the top-level object map;
  serialize the value with `serde_json::to_string`. Failure handling (warn-and-swallow,
  same fields) unchanged. When the stamp is None the emitted bytes must be identical to
  the previous implementation's output.

### 3. New unit tests in audit/mod.rs cfg(test), by name

- `unstamped_records_are_byte_identical_to_direct_serialization`: build a recorder via
  `Recorder::to_file(temp path)`, record one `sample_record("navigate", None, "read")`,
  read the line back, assert it equals `serde_json::to_string(&record)` of an identical
  record value (field-for-field equal; reuse the existing `sample_record` helper and
  compare via parsed `serde_json::Value` equality on all fields plus identical key
  order).
- `stamped_record_appends_license_as_the_last_key`: same recipe with
  `set_license_stamp(Some("unlicensed"))` before recording; parse the line; assert
  `v["license"] == "unlicensed"` and the object's LAST key (iterate
  `as_object().unwrap().keys()`) is `"license"`, and the first key is `"event_id"`.
- `stamp_applies_to_session_events_too`: stamped recorder, `record_session_event` with
  the existing `sample_session_event()` helper; assert the line's `license` field equals
  the stamp and `event` is `"session_killed"`.
- `clearing_the_stamp_stops_stamping`: set a stamp, record, clear with `None`, record
  again to the same file; first line has `license`, second does not
  (`v.get("license").is_none()`).

### 4. Tests for disk-path shape (in license.rs cfg(test))

- `org_license_path_is_a_sibling_of_the_org_policy_file`: assert
  `org_license_path().parent() == crate::governance::config::load::org_policy_path().parent()`
  and the file name is exactly `license.json`.
- `user_license_path_ends_with_ghostlight_license_json`: when `Some(p)`, the last two
  components are `ghostlight` then `license.json` (mirror the shape of the existing
  `default_audit_path_ends_with_ghostlight_audit_jsonl` test in audit/mod.rs).

## Constraints

Only src/governance/license.rs and src/governance/audit/mod.rs change.
tests/audit_recorder.rs and every golden stay untouched and green. No wiring into
server.rs or doctor.rs yet. ASCII only.

## Verification

`cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
`cargo clippy --all-targets --features license-admin -- -D warnings`; `cargo test`
(record delta: previous + 6 new; tests/audit_recorder.rs must pass UNCHANGED -- if it
fails, STOP and restore per the failure protocol); ASCII diff scan; ledger entry; commit.

Commit subject: `feat(license): disk resolution and audit-record license stamp (ADR-0028 D3)`

## Out of scope

CLI, fixture, server wiring, doctor; hot-reload of the license file (explicitly v1-out
per 00-design.md); any change to record STRUCTS in ports.rs (the stamp is applied at
serialization time in the Recorder, never on the types).
