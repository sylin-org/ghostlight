# 0026. Release maturity and externalities sequencing

- Status: Accepted
- Date: 2026-07

## Context

The engine and the full governance layer are built and verified against a real
browser on Windows (CLAUDE.md current status). A second external assessment
confirmed that every significant engineering concern from the first pass is
resolved (the P0 startup outage, the transport-layer audit state machine, the
misleading tool stubs), plus hot-reload and the Chrome Web Store submission
artifacts. What remains is not design risk: it is the externalities layer any
project accrues on the way to a public release. A license, continuous
integration, a current authoritative spec, networked audit and central-management
delivery, automated coverage of the extension's JavaScript handlers, and a
systematic live-browser pass.

Most of these are already named as pending in the README ("Packaging (partial)":
CI, cross-platform, Linux live verification, syslog/http audit
destinations, a license decision). They were deferred on purpose behind engine
and governance stability, not overlooked. This ADR records the decisions that
schedule them, in a sequence where every prefix leaves a coherent tree, and pins
a trigger for each item held back.

This ADR does not reopen the v1 scope boundary in ADR-0014. It decides
sequencing, documentation currency, and which previously-deferred capabilities
enter the near-term build. One item it touches, `managed://` delivery, is an
implementation gap on a path ADR-0014 already endorses (`chrome.storage.managed`
as the tamper-resistant enterprise channel), not a change to the boundary.

## Decision

### 1. License: execute the open-core layout decided in ADR-0027

The license model is decided separately in ADR-0027: Ghostlight is open-core, a
dual Apache-2.0 OR MIT engine plus a source-available Ghostlight Commercial
License over `src/governance/**`, superseding ADR-0021's whole-repo permissive
stance. This maturity pass owns only the execution of that decision: create
repo-root `LICENSE-APACHE`, `LICENSE-MIT`, and `LICENSE-GOVERNANCE`, plus the
`LICENSE` notice stating the engine/governance split; set the crate license
fields per ADR-0027 Decision 4 (`publish = false` while single-crate); and replace
the stale "TBD (intended open-source)" strings in `Cargo.toml`, `README.md`, and
`docs/SPEC.md` with the open-core statement (open engine, commercial governance).
The private `.dev-key.pem` stays gitignored; it is a signing key, unrelated to the
source license. This is the highest-leverage unblocker: the Chrome Web Store
submission artifacts and any public release depend on a resolved license.

### 2. Continuous integration: Windows-and-Linux matrix plus release artifacts

A GitHub Actions workflow runs on every push and pull request across
`windows-latest` and `ubuntu-latest`: `cargo test`,
`cargo clippy -- -D warnings`, and `cargo fmt --check`. A separate release job,
triggered on a version tag, cross-compiles the `ghostlight` binary for the
shipping targets (Windows x86_64 and Linux x86_64) and uploads them as build
artifacts. The matrix closes the Linux gap at the
compile-and-unit layer, which is the bulk of cross-platform risk and was
previously unverified; a green matrix would have caught the dual-schema-gate
class of regression at push time; and the artifact job makes the "single
portable binary, zero runtime dependencies" promise checkable per platform.
Live-browser behavior and the extension's JavaScript handlers are out of this
workflow's scope; they are Decisions 6 and 7. The pinned extension id and the
native-host registration are not exercised by CI.

### 3. Authoritative spec: full schema-3 rewrite now

`docs/SPEC.md` is rewritten now to describe the current system as built, replacing
the v0.1 (2026-07-01) draft rather than banner-annotating it. The rewrite reflects
manifest schema-3 and the epistemic-capability model with per-action requirements
and host polarity (ADR-0022); the one loader, tool registry, and generic ingest
pipeline (ADR-0023, ADR-0024); live hot-reload (ADR-0025); the Ghostlight name
(ADR-0021); and the open-core licensing (ADR-0027). The prior v0.1 draft is
preserved through git history and the ADR trail, consistent with the "supersede,
do not silently edit" ethos, but the working spec a reader opens is current. The
accepted trade-off is re-drift: a rewritten spec can fall behind the next stage,
so the rewrite carries a standing "re-sync `docs/SPEC.md` on stage close" note,
and the spec header states the commit or stage it was last reconciled against.

### 4. Audit destinations: build syslog and none next; defer http with a trigger

The `syslog` audit destination is implemented next, together with the trivial
`none`, using the seams already in place (destination selection in the audit
layer and `Recorder::reload` hot-swap, ADR-0025). This moves the SIEM-forwarding
and healthcare-ready claim from architecture to in-tree capability. The `http`
destination stays deferred with a trigger: a concrete forwarding requirement that
syslog cannot satisfy. http is held back because a per-record HTTP destination
introduces a network dependency with retry and backpressure semantics, closer to
the remote-dependency caution in ADR-0014 than syslog's local socket is.

### 5. Central management: implement managed:// delivery

The `managed://` manifest source, currently a precise `ManagedNotSupported` error
(`src/governance/manifest/source.rs`), is implemented, completing the enterprise
delivery path ADR-0014 endorses (`chrome.storage.managed`, pushed via Intune or
GPO). This is a larger change than flipping the error, and it carries an open
mechanism sub-question to be pinned when the stage is scoped. The manifest source
loader lives in the binary, and every existing source (`file://`, `env://`, the
org policy file) is a binary-side read, so the consistent implementation is the
binary reading the OS-level Chrome managed-policy store for the pinned extension
id (Windows registry or Linux managed policy JSON), which preserves the
policy-free extension (ADR-0005). The fallback, if that store
proves unreliable to read out-of-process, is for the extension to read
`chrome.storage.managed` and forward the manifest blob to the binary over native
messaging (still mechanism, not a policy decision). This decision commits to
shipping `managed://` and to the binary-side-read direction; the platform-specific
reader and its interaction with the existing org-policy-file channel (whether
`managed://` subsumes that channel or sits beside it) are settled in the
implementing stage. `managed://` is the narrowest-audience item built now and is
scheduled after Decisions 1 through 4.

### 6. Extension JavaScript coverage: extract pure logic and add a headless smoke

Two layers of coverage land in this pass. First, the algorithmic core currently
inline in the extension's service worker and content script is extracted into
standalone JavaScript modules with no `chrome.*` dependency at import time:
shadow-DOM traversal for `form_input`, the screenshot coordinate rescale
(ADR-0010), and accessibility-tree construction for `read_page` and `find`; these
modules are unit-tested with a zero-dependency runner (`node --test`) inside the
Windows-and-Linux CI matrix from Decision 2. Second, a headless-Chromium smoke (a
Playwright-driven fixture page exercising `navigate`, `read_page`, `computer`
screenshot and click, and `form_input`) is also built now, exercising the thin CDP
glue end to end that the unit layer cannot reach. The extracted functions are the
algorithmic core and the likeliest bug sites; the smoke covers the wiring. The
accepted cost is that the headless smoke adds CI flakiness and maintenance the
unit layer does not, and it runs Chromium in CI on at least one OS (Linux) rather
than both. Constraint: extraction holds the policy-free, lean-extension line
(ADR-0005). The modules carry mechanism and algorithms only, not new
responsibilities and not any access decision.

### 7. Live-browser verification: record the true state, correct the stale ledger

The record is corrected to match reality. Stage-4's `t-live-1` was live-verified
against real Chrome and Claude Code in commit 44db1f3 (the pipeline rewrite,
hot-reload end to end, org policy swaps, corrupt-and-recover, and deletion back to
all-open, all with zero restarts). The closing statement in
`docs/tasks/stage-4/LEDGER.md` that predates that pass and says stage 4 is not
verified end to end is corrected by an appended note pointing to 44db1f3, rather
than an edit, keeping the ledger append-only. The 44db1f3 pass also covered the
stage-3 `s-live-1` through `s-live-4` checks and `t01-1`, `t05-1`, `t06-1`, and
`t06-2`, all PASS. What remains owed to a human, per the not-covered notes in
`docs/tasks/stage-2/BROWSER-TESTS.md`: `g13-1` steps 4 and 5, `g13-3`'s governed
half, `g15-1` and `g15-2` (mode switch), and Linux live checks. That
owed surface shrinks as Decision 6's automation grows. One known gap from the
`t-live-1` pass stands on the record: it could not confirm the expected
ERROR-level server log line for the
invalid mid-edit, because that session had no access to the server's stderr; the
behavioral guarantee (the last good manifest keeps enforcing) was confirmed by an
identical denial id before and after the corrupt edit.

## Consequences

- Positive: the highest-leverage publishing blocker (no license) is removed by
  executing the open-core layout from ADR-0027, and the repo becomes contributable
  and legally shippable, with the engine open and the governance module commercial.
- Positive: a Windows-and-Linux CI matrix turns the previously untested Linux surface
  green at the compile-and-unit layer on every push, and gives per-platform
  release artifacts that make the zero-runtime-dependency claim checkable.
- Positive: the spec becomes an accurate current description of the built system
  rather than a stale v0.1 draft that misleads readers.
- Positive: syslog makes the audit and SIEM story real, and `managed://` completes
  the enterprise delivery path ADR-0014 endorses; the two together let a real
  enterprise pilot run end to end.
- Positive: extracting and unit-testing the extension's algorithmic core closes
  the largest coverage gap in the way least prone to flakiness, and runs it
  cross-platform.
- Negative: `managed://` is more than an error-to-implementation flip; it carries
  a platform-specific reader and the open mechanism sub-question in Decision 5,
  and it serves the narrowest audience of the items built now, so it is the most
  likely to slip if scope tightens.
- Negative: the headless-Chromium smoke built in Decision 6 adds CI flakiness and
  maintenance the unit layer does not, and runs Chromium in CI (Linux at least);
  the durable unit layer is the low-flake foundation under it.
- Negative: the full schema-3 spec rewrite (Decision 3) can re-drift on the next
  stage, an accepted cost carried by a standing re-sync note; the http audit
  destination (Decision 4) remains deferred, so the audit destination set is not
  yet complete.
- Out of scope, unchanged: the ADR-0014 exclusions (built-in IdP, remote per-call
  policy service, multi-session multiplexing, content DLP, manifest signing,
  cross-browser, `upload_image`) are not reopened here.
- Out of scope, deferred to its own decision: the Chrome Web Store listing type
  (unlisted, enterprise force-install, or public listing) is not settled by this
  ADR; it is gated on the native-host distribution story and the
  debugger-permission review risk noted in the store-prep work.
- Sequencing: Decision 1 (execute ADR-0027's license layout) and Decision 2 (CI)
  are the do-now, independently landable unblockers. Decision 3 (full spec
  rewrite) is do-now but larger. Decision 4 (syslog, none) precedes Decision 5
  (`managed://`). Decision 6 lands its unit layer with Decision 2's CI and its
  headless smoke alongside. Decision 7 is a record correction plus a standing
  human-owed checklist.
