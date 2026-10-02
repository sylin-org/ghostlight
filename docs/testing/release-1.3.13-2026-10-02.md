# Ghostlight 1.3.13 release -- 2026-10-02

The owner authorized bump, changelog, commit, push and publication across configured
channels. This release uses the committed `f2e42368` browser/workbench candidate as
its base and adds ADR-0193's fixed-path download installation. Unfinished workspace
UI edits are excluded. Public state is reconciled from observed channel delivery.

## Local source gates

The isolated release checkout passes formatting, strict workspace clippy, workspace
tests, 404 extension tests, 11 npm tests and four MCPB tests. Versions are 1.3.13
across the workspace, desktop, adapter, npm launcher and MCPB. No dependency changed.
The public truth guard still observes release/adapter 1.3.12. Repository integrity
passes after restoring a referenced review and identifying temporary evidence paths
as local references, without removing their recorded results.

Freshly built Windows binaries pass fixed-path legacy migration, running-image
replacement, unchanged connector continuity, runtime reacknowledgement, retained
history, offline PowerShell download rejection and the complete process journey.

The Chromium 152.0.7977.82 browser lane passes script, real MV3 frames, history and
modern/legacy quiet-coexistence fixtures. One fixture now asserts the specific
`script_exception` cause while retaining unknown-effect and no-replay checks. The
visual fixture uses its own observer session after cancellation releases the
product's debugger lease.

The initial ZIP was withdrawn from staged review after CI exposed native-input
ordering. The final ZIP hash will be bound to the corrected workflow candidate.
This is local packaging evidence until the workflow candidate binds and attests
the same bytes. Local `cargo audit` was unavailable; the required CI dependency
job must pass before publication. The workflow also runs Linux fixed-installation
and downloader fixtures and Windows release-binary installation checks.

No everyday browser profile, installed service, settings or grants were deployed
by these isolated source gates. Synthetic native framing and test Chrome profiles
remain distinct from installed-user and clean-machine acceptance.

## Publication

Pending the provenance-bound candidate and independent channel reconciliation.
GitHub, npm, Chrome and MCP Registry access checks succeeded. Candidate, publication
results, package-manager submissions and website delivery will be recorded here.

## CI corrections

Initial source `b4340ba0` failed release run 37056235264 and CI run 37056236218.
Linux sysinfo process discovery included thread IDs. Replacement now requests
processes without tasks; killing a task ID could fail after its process exited.
Chrome can deliver an extension-requested activation after tabs.update resolves.
Native selection now awaits that exact event before freezing the revision, with
a bounded conservative refusal if delivery is missing. Foreground/takeover fences
remain enforced. New unit cases cover delayed and missing activation delivery.

The opaque native relay now drains its reader through EOF after a failed write.
Immediate reconnect could discard the old generation's final Ended/error frames.
The real legacy adapter journey remains the platform acceptance gate for this.
Read cancellation fixtures now assert cancelled/none, matching ADR-0190's existing
read-only account, and separately cancel a dispatched tab mutation to prove unknown
effects and unsafe repeat. The CI screenshot fixture requests a 30000 ms capture
budget; its short denied-absence observation budget and assertions stay unchanged.
The new candidate and complete CI must pass before public publication.

Corrected Windows source passes fmt, strict workspace Clippy, all workspace tests,
406 extension tests, process and local-resilience journeys, fixed replacement and
the complete browser lane. The local deterministic adapter ZIP SHA256 is
`a7821d3fed44da845a1022e80b1c3682b9c0a70a9971bf79ee6fc88bff9527b3`.
Linux acceptance remains pending the corrected workflow.

Run 37058240300 passes the Linux source gate, fixed replacement and native Linux
package build. CI run 37058202430 passes both Rust/extension lanes, dependency
policy and Linux process journeys. It confirms legacy Ended/error delivery on Linux.
The remaining browser failures occur at native mouse focus on Xvfb without a window
manager and inactive Windows screenshot access. A Windows reconnect fixture also
issued its first effect before browser runtime readiness.

The follow-up fixture uses Openbox in the disposable Linux display, keeps inactive
Chrome timers/rendering available and probes browser readiness read-only before
the first post-restart mutation. It preserves foreground, trusted-effect and
no-replay assertions. These fixture arguments are distinct from installed browser
evidence. Local source gates, process and full Windows browser lanes pass with the
fixture changes. Corrected CI acceptance is still required.

Candidate run 37059666240 passes both native package lanes, source gates and Linux
package smoke. CI run 37059666479 passes both process lanes, Rust, extension and
dependency checks, but fails the browser lanes. Openbox does not resolve Linux
background mouse focus: the owned window becomes focused and subsequent input is
correctly refused. Windows still fails the excluded-frame screenshot. Publication
remains held, and the corrected Chrome staged review was also cancelled.

The native startup path now finishes its asynchronous attendance query before
exposing the port. Browser events could previously precede hello while that query
was pending. A new VM regression test proves hello is first under that interleave;
407 extension tests and the Rust source gates pass. Failure-only browser probes
record bounded native-packet/focus ordering and raw screenshot dispatch errors.
The Linux probe compares mouse dispatch, synthetic mouse tap and owned tab selection
in the disposable fixture after failure, then rethrows the original failure.
It cannot turn a failed acceptance lane into success. The first local run stalled
when the diagnostic worker was evaluated before native readiness; evaluation now
waits for the fixture handshake. These diagnostics are test-only.
The corrected local frame journey passes all 69 checks with pinned Chrome 152;
the other browser component lanes passed in that same local source run.
