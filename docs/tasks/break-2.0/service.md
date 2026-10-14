# Break 2.0: service lane ledger

Lane: service. It owns `crates/orchestrator/src/**` (except the page assets under
`chromium/assets/` and `chromium/level4/assets/`) and this ledger. Ground rules are in
`BOOTSTRAP.md` when the connectors lane has landed it. The target is
`docs/adr/0197-contained-seams-and-the-2-0-break.md`; the wire is
`docs/design/adapter-protocol-4.md`.

BASE: `f28bcbd7` (dev when the lane started). Read pre-break sources with
`git show f28bcbd7:<path>`.

Gate: `cargo fmt -p ghostlight --check`, `cargo clippy -p ghostlight --all-targets -- -D
warnings`, `cargo test -p ghostlight`, with `CARGO_TARGET_DIR=.target-assessment`. No `--locked`.
Never stage `Cargo.lock` or a `Cargo.toml`.

## RESUME HERE

Next slice: `service-2`. The wire vocabulary is in place and nothing consumes it yet.

## Landed

| Slice | What | Commit |
|---|---|---|
| service-1-p4-wire | `chromium/level4/{mod.rs, wire.rs}`: protocol 4 frames, primitives, arguments, outcomes, `Failure`, `CallFailure`, and tolerant mirrors of the physical vocabulary. 23 inline tests from the spec's JSON examples. | see `git log` |

## Notes

- Every module on the path from `lib.rs` is `pub`, so the new items are not dead code before
  they are wired.
- Mirrors of types the extension sends (`DiagnosticEntry`, `RecordingSummary`, `Delivery`) convert
  into the physical types with `TryFrom`, because an `Unknown` arm has no physical counterpart.
  Mirrors of types only the service sends convert with `From`.
- The wire `RuntimeControlState` has no attention state. The physical `Attention` maps to
  `Held` until switch-11 removes it.
- Optional arguments follow the spec's Rust sketch literally: only the fields the spec marks
  `skip_serializing_if` are left out when absent; the others serialize as `null`. switch-2 binds
  both sides to the shared fixtures and reconciles any difference.
