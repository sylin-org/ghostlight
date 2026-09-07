# C1: Provenance reporting foundation

The owner accepted option A and directed implementation on 2026-09-07: correct attribution and
expose useful connection details first. [ADR-0161](../../adr/0161-connection-bound-provenance.md)
owns that decision. The reporting foundation is implemented and verified locally. This record
accompanies `fix(provenance): preserve each action's connection evidence`; Git owns the commit hash.
Signature and hash verification are deferred. This cycle is the first part of C1, not completion
of its full reporting direction or C2/C3 admission.

## Accepted behavior

- An action retains its original connection's evidence even when another connection shares its
  workspace, it waits in a queue, or the originating connection closes.
- Familiar application labels remain visible as claims in sessions and bounded live history.
  Durable records omit the raw claim and retain narrow observed attribution.
- Connection details distinguish reported application, observed executable, and unchecked
  signature. Missing or unsupported observations have explicit explanations.
- Sessions show plural active connections. Parent receipts and recorded children retain their
  own attribution. Details stay collapsed until requested and preserve expansion and focus.
- Ordinary operation remains quiet. No routine notification, trust badge, verifier, admission
  rule, or user setting is introduced.

## Current implementation seams

- `crates/orchestrator/src/provenance.rs` separates transient `ConnectionEvidence` from serializable
  `Attribution`. It bounds claims and basenames, issues fresh connection IDs and observation times,
  and represents unsupported, unavailable, and historical observations explicitly.
- `language/provenance.rs` authors human details and explains the distinction between the direct
  executable and the application or instructions using it.
- `crates/win-peer/src/table.rs` fixes TCP-table lookup direction so the observed PID belongs to
  the remote connection's endpoint instead of the service. Distinct-process tests cover this seam.
- `service/mod.rs` captures evidence at admission. `workspace/mod.rs` tracks active connections
  individually while preserving caller-owned session lifetime.
- `work/mod.rs` keeps evidence with prepared invocations and composed execution contexts.
  `work/receipt.rs` feeds the same provenance into terminal and preparation-failure receipts.
- `audit.rs` sends only safe durable attribution to storage and supplies the bounded live claim
  separately to the workbench. `workbench` retains those details with operations and grouped
  receipts, including restored records that have no retained claim. Legacy `peer_image` values
  are not presented as remote-executable observations; their original files remain untouched.
- `ui/lib/entries.js` and `ui/lib/view.js` render each action's evidence instead of resolving its
  identity through current session state. Existing detail/focus handling and the connections strip
  provide progressive disclosure; `ui/styles.css` keeps evidence neutral and wrapping.
- The MCP connector's diagnostics omit raw labels. The service bridge, browser connector, and
  extension contracts do not change for this reporting feature.

## Validation

Completed validation covers:

1. Immutable attribution across simultaneous shared-session connections, direct and queued work,
   composed child receipts, preparation failures, and refusals.
2. Fresh evidence on reconnect, independent active-connection removal, and preserved action
   details after the origin disconnects.
3. Narrow durable serialization and reload: raw claim sentinels, paths, PIDs, command lines, and
   certificate material do not enter new provenance records. Legacy gaps remain explicit.
4. Native Windows socket observation across distinct processes, including Node, a renamed Node
   executable, and the actual MCP connector, plus explicit unavailable/unsupported states. No test
   labels an unchecked executable trusted or turns reporting into admission.
5. Actual process-boundary and repository commit gates against the new build.

The UI implementation passes 66 surface checks, including six C1 checks, plus the
isolated Chromium history journey covering C1 and the existing H4-H8 behavior. The C1 journey
changes current-session identity while an earlier receipt remains visible, preserves its original
claim and executable, opens plural connection details, and verifies keyboard focus and 720-pixel
layout through refresh. The connection-details screenshot was visually inspected. These are
synthetic workbench projection fixtures in the real bundled UI, not an installed Tauri deployment.

Final source validation on Windows:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| Workspace Clippy, all targets, `-D warnings` | Passed |
| `cargo test --workspace` | 508 passed, including 10 added tests |
| Extension `npm test` | 192 passed |
| Changed JavaScript syntax | Seven files passed |
| C1 real-process journey | 12 receipts; distinct Node/renamed Node/MCP connector observations; immutable queued/direct/child/preparation/refusal/reconnect evidence; claim canaries absent from audit and both diagnostic streams |
| Existing process journey | Reconnect, primitives, audit write failure/repair/cold start passed; previous raw-claim diagnostic assertion replaced with bounded connection attribution and a negative claim assertion |
| CLI journey | Governed calls, shared batch session, audit attribution, channel refusal passed |
| H8 continuity journey | Queue/control isolation, stalled peers, cancellation, private runtime publication passed |
| Workbench surface and policy grammar | 66 surface checks and grammar checks passed |
| Isolated Chromium UI | H4-H8/C1 history, original claims after session change, plural connection details, focus and narrow layout passed |
| Sylin/MV3 regression | All 29 checks passed with Chrome 152.0.7977.82 |

The C1 regression is wired into both Windows and Linux process CI; this session ran Windows only.
Runtime journeys used `.target-ghostlight-1.0/debug` explicitly through `GHOSTLIGHT_BIN_DIR`.
The C1 process fixture uses synthetic Sylin browser receipts; the separate frame-browser journey
uses real Chromium, the source MV3 adapter, and public Sylin content with isolated local forms.
Its native-port discovery substitution remains the existing H6 test limitation. No owner's browser
profile or installed stack was changed.

The fresh-build cross-process fixture first failed because the old Windows lookup matched
`find_row(local, peer)`, returning the service-owned accepted-socket row. The correction matches
`find_row(peer, local)` to find the remote endpoint's owner. The previous same-process socket
test could not distinguish those directions. This is a root correction to observation itself,
in addition to the immutable attribution change. Old audit basenames cannot establish which
remote program connected, so their human projections report Not recorded while preserving the
historical files.

## Limits and next cycle

Windows uses the corrected socket-owner observer. Linux's lack of observation at this seam is
explicit; no Linux observation implementation or live Linux verification is added. Basename
observation is not executable authenticity, upstream identity, or instruction provenance.

Signature/hash verification remains deferred until the next C1 ideation selects its concrete
subject and verification behavior. Signer identity, exact-hash representation, offline result
states and limits, cache invalidation, and C2/C3 integration/binding remain open. No code-signing
certificate procurement requirement or client credential protocol follows from this cycle.

This is source work only. The installed service and extensions have not been replaced by this
cycle. No deployment, push, publication, or event submission is included.
