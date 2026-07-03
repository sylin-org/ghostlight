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
CI, cross-platform, macOS/Linux live verification, syslog/http audit
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

### 1. License: ratify the dual Apache-2.0 OR MIT already chosen

ADR-0021 fixed the org convention as dual "Apache-2.0 OR MIT." This ADR ratifies
that as the project's license and directs its execution: `LICENSE-APACHE` and
`LICENSE-MIT` at the workspace root, `license = "Apache-2.0 OR MIT"` in the
workspace `Cargo.toml`, and removal of the stale "TBD (intended open-source)"
strings in `Cargo.toml`, `README.md`, and `docs/SPEC.md`. This is the standard
Rust dual-license: the Apache-2.0 half carries an explicit patent grant that
enterprise adopters prefer, the MIT half maximizes permissive reuse, and a
downstream consumer may satisfy either. The private `.dev-key.pem` stays
gitignored; it is a signing key, unrelated to the source license. Because
ADR-0021 already made this choice, the decision here is to stop carrying it as
open and transcribe it.

### 2. Continuous integration: three-OS matrix plus release artifacts

A GitHub Actions workflow runs on every push and pull request across
`windows-latest`, `macos-latest`, and `ubuntu-latest`: `cargo test`,
`cargo clippy -- -D warnings`, and `cargo fmt --check`. A separate release job,
triggered on a version tag, cross-compiles the `ghostlight` binary for the
shipping targets (Windows x86_64, macOS x86_64 and aarch64, Linux x86_64) and
uploads them as build artifacts. The matrix closes the macOS/Linux gap at the
compile-and-unit layer, which is the bulk of cross-platform risk and was
previously unverified; a green matrix would have caught the dual-schema-gate
class of regression at push time; and the artifact job makes the "single
portable binary, zero runtime dependencies" promise checkable per platform.
Live-browser behavior and the extension's JavaScript handlers are out of this
workflow's scope; they are Decisions 6 and 7. The pinned extension id and the
native-host registration are not exercised by CI.

### 3. Authoritative spec: supersession banner plus a change map

`docs/SPEC.md` keeps its v0.1 body but gains, at the top, a supersession banner
marking the v0.1 draft (2026-07-01) as predating the current model, followed by a
short "What changed since v0.1" section that maps each superseded concept to the
ADR that controls it: manifest schema-1 to schema-3, and the observe/mutate tier
model to epistemic capabilities with per-action requirements and host polarity
(ADR-0022); single static manifest load to one loader, a tool registry, and the
generic ingest pipeline (ADR-0023, ADR-0024); no reload to live hot-reload
(ADR-0025); and the "Browser MCP" name to Ghostlight (ADR-0021). A full schema-3
rewrite of the spec body is deferred with a trigger: it is done when the
capability model is frozen for a public v1, after the current stage sequence
settles, so the rewrite does not re-drift on the next stage. The banner ends the
stale-authoritative-document liability immediately, and the change map gives a
reader an accurate mental model at a fraction of a rewrite's cost.

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
id (Windows registry, macOS managed plist, Linux managed policy JSON), which
preserves the policy-free extension (ADR-0005). The fallback, if that store
proves unreliable to read out-of-process, is for the extension to read
`chrome.storage.managed` and forward the manifest blob to the binary over native
messaging (still mechanism, not a policy decision). This decision commits to
shipping `managed://` and to the binary-side-read direction; the platform-specific
reader and its interaction with the existing org-policy-file channel (whether
`managed://` subsumes that channel or sits beside it) are settled in the
implementing stage. `managed://` is the narrowest-audience item built now and is
scheduled after Decisions 1 through 4.

### 6. Extension JavaScript coverage: extract pure logic, then a headless smoke

The algorithmic core currently inline in the extension's service worker and
content script is extracted into standalone JavaScript modules with no `chrome.*`
dependency at import time: shadow-DOM traversal for `form_input`, the screenshot
coordinate rescale (ADR-0010), and accessibility-tree construction for
`read_page` and `find`. These modules are unit-tested with a zero-dependency
runner (`node --test`) inside the three-OS CI matrix from Decision 2. A
headless-Chromium smoke (a Playwright-driven fixture page exercising `navigate`,
`read_page`, `computer` screenshot and click, and `form_input`) is the sequenced
follow-on and is not required for this ADR's acceptance. The extracted functions
are the algorithmic core and the likeliest bug sites, and their unit tests are
low-flake, durable, and cross-platform; the thin CDP glue remains the only
untested layer, covered by `live-demo.ps1` and the human live pass. Constraint:
extraction holds the policy-free, lean-extension line (ADR-0005). The modules
carry mechanism and algorithms only, not new responsibilities and not any access
decision.

### 7. Live-browser verification: record the true state, correct the stale ledger

The record is corrected to match reality. Stage-4's `t-live-1` was live-verified
against real Chrome and Claude Code in commit 44db1f3 (the pipeline rewrite,
hot-reload end to end, org policy swaps, corrupt-and-recover, and deletion back to
all-open, all with zero restarts). The closing statement in
`docs/tasks/stage-4/LEDGER.md` that predates that pass and says stage 4 is not
verified end to end is corrected by an appended note pointing to 44db1f3, rather
than an edit, keeping the ledger append-only. What remains owed to a human,
tracked in `BROWSER-TESTS.md`: the stage-3 `s-live-1` through `s-live-4`
consolidated pass, and macOS and Linux live checks. That owed surface shrinks as
Decision 6's automation grows. One known gap from the `t-live-1` pass stands on
the record: it could not confirm the expected ERROR-level server log line for the
invalid mid-edit, because that session had no access to the server's stderr; the
behavioral guarantee (the last good manifest keeps enforcing) was confirmed by an
identical denial id before and after the corrupt edit.

## Consequences

- Positive: the highest-leverage publishing blocker (no license) is removed by
  transcribing an already-made decision, and the repo becomes contributable and
  legally shippable.
- Positive: a three-OS CI matrix turns the largest untested surface (macOS and
  Linux) green at the compile-and-unit layer on every push, and gives per-platform
  release artifacts that make the zero-runtime-dependency claim checkable.
- Positive: the spec stops misleading readers immediately, without paying for a
  rewrite that would re-drift.
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
- Negative: the headless-Chromium smoke (the follow-on in Decision 6) will add CI
  flakiness and maintenance that unit tests do not; it is deliberately sequenced
  after the durable unit layer.
- Negative: the full schema-3 spec rewrite (Decision 3) and the http audit
  destination (Decision 4) remain deferred, so the spec body and the audit
  destination set are not yet complete.
- Out of scope, unchanged: the ADR-0014 exclusions (built-in IdP, remote per-call
  policy service, multi-session multiplexing, content DLP, manifest signing,
  cross-browser, `upload_image`) are not reopened here.
- Out of scope, deferred to its own decision: the Chrome Web Store listing type
  (unlisted, enterprise force-install, or public listing) is not settled by this
  ADR; it is gated on the native-host distribution story and the
  debugger-permission review risk noted in the store-prep work.
- Sequencing: Decisions 1 through 3 are do-now and independently landable.
  Decision 4 (syslog, none) precedes Decision 5 (`managed://`). Decision 6 lands
  its unit layer with Decision 2's CI and its smoke afterward. Decision 7 is a
  record correction plus a standing human-owed checklist.
