# Security assessment -- 2026-09-06

This is a dated review of Ghostlight's implemented security boundaries, failure behavior, and
evidence. It is independent of any submission, event, or marketing plan. Recommendations are
proposals, not accepted architecture or authorization to implement an epic.

Reviewed source: `48ef29ece1ff0f1633daba62a03932a666f00ca5`, service 1.3.4 and extension manifest
1.1.1. Current source, tests, and the four [1.0 contracts](../1.0/) take precedence over historical
implementation descriptions. Recheck these findings against the source before acting on them.

## Assessment

Ghostlight already has a substantial governance implementation. Its strongest security property
is centralized control of browser operations delegated through its own service: typed operations,
independent capability requirements, intersecting policy layers, workspace ownership, and signed
managed policy. Building a second policy engine or a cloud security service would miss that
existing foundation.

The most concrete hardening work is in composition, effect truth, and audit projection. Safe,
isolated probes reproduced continued flow execution after a stop condition, missing child audit
records, page text copied into a failed flow's audit record, script re-evaluation after an
attacker-controlled error, and denial attention crossing workspace boundaries. These are defects
in Ghostlight's own promises and do not require a compromised operating-system account.

Client attribution and host compromise need a different assessment. The local transport is not an
authenticated identity for the upstream model or harness. That limitation does not, by itself,
make stronger client attestation a required product feature. The value of such a feature depends
on an actual isolation boundary and a caller whose credentials cannot simply be reused by the
attacker. The owner's hardening discussion is still open; no new identity decision is adopted here.

## Threat model and responsibility

Locality describes where a transport terminates. It does not establish the effective privileges of
the model, the harness, a sandboxed process, another desktop application, or a hostile page. A
local harness may also be controlled by a remote model; Ghostlight does not own that relationship.

| Attacker or failure | What Ghostlight can reasonably protect |
| --- | --- |
| A page supplies malicious instructions to a model that can only use approved tools | Enforce the configured host and capability boundaries on the resulting operations, regardless of why the model requested them. This contains some consequences; it does not detect every injection or infer task intent. |
| A buggy or hostile admitted caller stays within the Ghostlight interface | Validate requests, preserve workspace and handle ownership, bound resource use, obey human controls, and report effects and policy decisions accurately. |
| A local process has less authority than the browser adapter | Preserve the admission boundary instead of silently granting that process the adapter's authority. Whether this is a distinct principal depends on deployment and OS isolation. |
| Arbitrary unsandboxed code already runs as the same OS user with access to Ghostlight's token and mutable state | Ghostlight cannot promise endpoint containment or enforcement over alternate browser controllers. A stronger boundary requires protection outside that same process/account trust domain. |
| An administrator or attacker can replace the service, change trusted bootstrap keys, or rewrite all local state | Trust in this installation is lost. Policy signatures and local audit files do not restore that trust on their own. |
| Malformed input, cancellation, disconnect, storage failure, or competing legitimate callers | Correct handling remains Ghostlight's responsibility even without a malicious principal. |

Same-user arbitrary execution is therefore a threat-model limit, not a blanket critical defect in
client provenance. Conversely, "a local process could control the browser anyway" is not a
universal platform fact. For example, Chrome changed its remote-debugging switches starting in
136 so they no longer debug the default data directory. This illustrates why the privileges of an
already-authorized browser adapter should not be assumed equivalent to every local process; it
does not establish that Ghostlight currently isolates those processes.
See [Chrome's explanation](https://developer.chrome.com/blog/remote-debugging-port).

The proposed boundary for the hardening discussion is: Ghostlight enforces and truthfully records
browser operations entrusted to it, within the endpoint's trust assumptions. It does not secure
the endpoint against arbitrary same-user code or govern actions taken through other controllers.

## Implemented architecture and controls

```text
MCP client
  -> stdio MCP connector
  -> authenticated loopback service bridge
  -> Rust orchestrator and local workbench
  -> authenticated loopback browser bridge
  -> native-messaging browser connector
  -> Chromium extension
  -> browser APIs, DOM observation, and CDP
```

The orchestrator owns language, governance, workspaces, browser coordination, and terminal
outcomes. The MCP connector owns protocol lifecycle and generic rendering. The browser connector
relays typed traffic. The extension owns the browser's physical mechanisms and presentation,
without owning policy. The Windows peer-observation crate is a narrow platform seam.

### Admission and attribution

- [Service startup](../../crates/orchestrator/src/service/mod.rs) binds two ephemeral IPv4
  loopback listeners and publishes a random runtime token. Both listeners use that token.
- [Runtime discovery](../../crates/bridge/src/runtime.rs) applies Unix mode `0600` when creating
  its temporary file. Windows access relies on the directory/file ACLs inherited by this path;
  this review did not independently test installed ACLs or cross-user access.
- Service hello verifies the token and protocol major before workspace admission. This proves
  possession of a local secret, not which upstream model or application initiated the work.
- `client_label`, intake channel, and supplied session markers are claims. On Windows, the
  service can separately observe the socket peer's bounded executable basename. In the normal
  MCP topology that peer is Ghostlight's connector, not the upstream harness or model.
- [ADR-0105](../adr/0105-scripted-intake-channels.md) already distinguishes attribution from
  authorization, rejects executable-name allowlists and per-channel tokens, and explains why
  signing an interpreter does not authenticate what it is instructed to do. Its signer gate
  remains deferred. Its historical claim that `client_label` reaches audit does not match the
  current `AuditRecord`, which has channel and optional peer image but no client-label field.
- [ADR-0106](../adr/0106-caller-owned-sessions.md) uses process markers or declared session keys
  for continuity. Current workspace resumption matches the supplied marker; it does not
  authenticate an independent principal. Ordinary MCP sessions send no marker. These sessions
  must not be described as hostile-tenant isolation.
- Native-host registration names allowed extension origins. That narrows the browser's native
  messaging route; it does not authenticate every process able to use the local service token.

No direct unauthenticated remote MCP endpoint was found. Loopback still warrants input bounds,
admission checks, and protection against accidental exposure. A per-connection thread and an
invocation thread per call are visible in the service; pre-authentication deadlines and service
resource limits need a focused review. No resource-exhaustion attack was run.

### Policy and capabilities

The [capability directory](../../crates/orchestrator/src/language/capability_map.rs) is the source
of truth. RAWX are independent mechanism permissions, not an ordered privilege scale or a
classification of business consequences.

| Operation | Current requirement |
| --- | --- |
| Navigate, read, inspect, find, screenshot, scroll, hover, wait | Read |
| Click, type text, press key, drag, history, close a tab, resolve a dialog | Action |
| Upload explicitly named files | Write |
| Fill a form | Read + Write |
| Fill and submit a form | Read + Write + Action |
| Evaluate page JavaScript | Execute |
| Flow or sequence wrapper | Empty; children authorize separately |

Denying Write does not prevent typing through Action. Denying Execute does not prevent a
transaction completed with clicks and typing. Read does not promise that a destination's GET,
hover handler, or other page behavior has no server-side consequences. This is consistent with
Ghostlight owning browser mechanisms rather than inferring the user's larger intent.

Schema-3 policy is strictly decoded. Ordered grants match exact hosts, suffix wildcards, or all
hosts and admit a complete requirement set. Managed, user, and request restrictions intersect.
An invocation uses an immutable authority snapshot; valid updates affect later invocations and
invalid hot reloads retain last-known-good authority. Observe mode can admit ordinary would-deny
work and records that distinction. Hard scheme and configured sacred-host ceilings remain.

With no authored policy, HTTP(S) work is all-open. Localhost, loopback, and link-local addresses
are deliberately included by [ADR-0155](../adr/0155-policy-owned-local-destinations.md). Do not
reintroduce an address ban as an assumed hardening requirement. Current host grants are not
scheme/host/port origin rules, DNS/IP/CIDR enforcement, a browser network firewall, or DLP.

### Signed managed policy

The [managed-policy implementation](../../crates/orchestrator/src/governance/managed/) already
verifies domain-bound Ed25519 signatures. Configuring an ML-DSA-65 key requires both signature
legs. The administrator supplies the bootstrap and verification keys; Ghostlight embeds no
vendor policy authority.

Updates reject lower publish sequences and conflicting envelopes at the same sequence. Verified
cache replacement is atomic, and cached bundles are verified again on read. A configured cold
start without valid authority denies work. HTTPS fetches refuse redirects, bound response size
and time, and support an organization CA and bearer token. There is no clock expiry; failed
refreshes retain verified policy and expose freshness separately.

These are meaningful supply and update protections. They do not establish protected freshness
against an OS adversary able to roll back both cached state and the service's remembered sequence,
or replace bootstrap trust. Such an adversary is outside the same local trust boundary. Replacing
the existing cryptography is not a supported priority from this review.

### Browser data, visibility, and distribution

Ghostlight deliberately operates in an already-authenticated browser profile. There is no exposed
cookie-extraction tool, but ordinary browser requests carry that profile's authentication and
the debugger adapter has substantial authority. Ordinary text readers avoid editable values and
closed roots; screenshots and explicit page scripts have different disclosure surfaces.

Upload accepts explicitly supplied regular-file paths with count, byte, and change-detection
checks. It does not provide a human file-picker authorization boundary or a directory allowlist.
The fact that a model supplied a path is not evidence that a person approved that disclosure.

The model-facing catalog is authored by the orchestrator. The page-declared tools in
[ADR-0134](../adr/0134-governed-page-declared-tools.md) are a design, not an implemented dynamic
tool catalog. Current page results are untrusted content; the MCP result envelope is not a
prompt-injection detector or an authenticated source of instructions.

In-page borders, cursor effects, the workbench, browser controls, and debugger release provide
visibility and local intervention. Presentation is best effort. A page can interfere with its
own DOM, so visual effects are not cryptographic evidence or a human approval interlock.
Recordings have bounded browser-memory storage and explicit export. Process diagnostics are
separate, optional, and bounded; their retention must not be confused with audit retention.

There is no vendor runtime, telemetry, or audit uploader. Optional managed-policy HTTPS is
customer-configured. SIEM integration uses an external local-file collector. Release packaging
already supplies checksums, SBOMs, and keyless GitHub build-provenance attestations. The npm
launcher validates downloaded binaries before launch. This is provenance for Ghostlight's own
artifacts, not proof of the provenance or behavior of arbitrary MCP clients. Some supply-chain
documentation still describes an older component count; verify current release jobs before
restating those counts. This review did not establish a dependency or CI compromise.

## Findings requiring product work

"Reproduced" below means an isolated test against current implementation seams, not a live
browser exploit. "Source finding" means the code path was inspected but its complete browser or
process journey was not exercised. Priority is proposed remediation order, not a CVSS rating.

### SA-01: Flow reports a stop but runs later steps

Reproduced. In [flow.rs](../../crates/orchestrator/src/work/flow.rs), a non-successful child under
`on_error: "stop"` sets `stopped = true` but does not break the loop. A decoded-operation error
also sets the flag and continues. Some reference-resolution error branches do break, so the
failure behavior depends on where the failure occurs.

A Read-only flow executed Read, refused Execute, then opened another allowed page. It returned
`stopped: true` with step statuses `succeeded`, `blocked`, `succeeded`. The refused script did not
reach the browser. The defect is continued work after the promised stop, not a bypass of the
child's Execute check.

Fix the composition control flow and define aggregate status/effect from the actual child
outcomes. Cover both a governed refusal and a decode failure followed by an independent step.

### SA-02: Composite children bypass terminal audit completion

Reproduced for flow; source finding for sequence. Flow invokes `self.run` for each decoded child.
[Sequence](../../crates/orchestrator/src/work/sequence.rs) invokes individual operation handlers.
Neither takes each child through the top-level `finish` path in
[work/mod.rs](../../crates/orchestrator/src/work/mod.rs).

Policy checks still happen in those handlers. The missing part is a complete per-child terminal
record and the completion behavior attached to it. The tested flow produced one wrapper record,
with empty capabilities and an allowed/permitted wrapper decision, despite its denied child.
That record cannot serve as a complete account of what the composition attempted.

The root fix belongs at a shared operation-completion seam that supports ordinary and composed
work. Preserve the parent's authority snapshot and workspace lease; recursively calling the
top-level executor would risk taking a new snapshot or reacquiring the same lease. Record child
identity/order and requirements explicitly. Define wrapper aggregation separately.

### SA-03: Failed flow persists page content in audit

Reproduced. `finish` copies the entire terminal `facts` value into `refusal_facts` whenever status
is not `succeeded`. A flow's facts contain complete child result envelopes, including a prior
successful Read. The synthetic page sentinel appeared in the resulting audit record.

This contradicts the metadata-only audit promise in project memory and the trust documents. It
requires no host compromise. The same seam also deserves review for browser error descriptions:
page-derived exception text can enter failure facts or summary sentences. The demonstrated leak
is the flow result; this review did not claim every error path was exercised.

Use a closed audit projection with only permitted fields, rather than copying arbitrary result
JSON or adding a blacklist of known sensitive keys. Review summary text as well as facts. A
negative test should place unique sentinels in page text, input, paths, and exceptions and prove
they cannot reach serialized audit through success, failure, or composition.

### SA-04: Script error text can cause repeated effects and false effect certainty

Reproduced against [script-evaluator.js](../../extension/lib/script-evaluator.js) using a Node VM
stub of the CDP sender. The evaluator retries a script in an async wrapper if an exception
description contains `Illegal return statement`. A script can produce that message after doing
work. The retry therefore runs the work twice.

Separately, any exception whose class is `SyntaxError` is treated as a parse failure with
`effectUnknown = false`. A running script can deliberately throw that class after changing state.
The extension preserves that false value when returning the failure to the service.

```javascript
globalThis.effects += 1;
throw new Error('Illegal return statement');
// Probe: 2 evaluations, 2 applied increments, effectUnknown true.

globalThis.effects += 1;
throw new SyntaxError('runtime-thrown exception');
// Probe: 1 evaluation, 1 applied increment, effectUnknown false, invalid_script.
```

Do not infer safe replay or pre-execution failure from attacker-controlled exception text or
class. Decide the supported script form before executing, or otherwise obtain evidence that
distinguishes parsing from evaluation without repeating possibly effectful code. Add a real
Chromium lane after the focused evaluator regressions. The Node probe proves the adapter logic;
it does not substitute for a live CDP compatibility test.

### SA-05: One workspace's denial attention blocks another

Reproduced. Denial counting is keyed by workspace, but the threshold calls
`RuntimeControls::require_attention` on the shared governance facade. That state is one shared
atomic value. Three matching denials in workspace A caused an otherwise permitted operation in
workspace B to return `attention_required` with reason `runtime_attention`.

This conflicts with the documented promise to pause the affected workspace. Keep deliberate
human global controls distinguishable from automatically triggered workspace attention. Test
the shared executor/facade with multiple workspaces, not only the workspace-keyed counter.

### SA-06: Composed observation lacks a per-frame host decision

Source finding; no live cross-origin disclosure probe was run. The service authorizes the
selected top-level tab URL. The extension's `httpFrameIds` enumerates HTTP(S) frames, retains
their IDs, and drops their URLs before `readDocument` and related composed observation paths
merge results. An HTTP(S) child frame can therefore have a different policy host from the
authorized parent without an equivalent per-frame policy decision on this path.

This matters if host policy is promised to constrain all observed document content, rather than
only top-level navigation. It is not proof that all browser subrequests are governed; they are
not. Define the intended subject boundary, then carry browser-observed frame/site information
through the typed browser port so that the orchestrator can decide. Do not put policy into the
extension. Include accessible child frames, frame-targeted actions, and screenshot semantics in
that decision instead of fixing only one text reader.

### SA-07: Audit append failures are discarded; local records are not tamper proof

Source finding. `JsonlAuditSink` appends JSON and a newline under a mutex and flushes the file.
Service startup requires it to open, but invocation completion discards subsequent errors with
`let _ = self.audit.record(&record)`. The workbench projection can still receive a record when
durable recording failed. A successful UI entry is therefore not proof of durable audit.

Expose audit health and specify the behavior when a write fails. Any configured fail-closed
behavior must acknowledge effects that already happened; it cannot turn a lost receipt into a
claim that the browser action never occurred. Prove the behavior with a failing sink.

The JSONL file has no hash chain, external checkpoint, protected append-only store, or audit
retention mechanism. A local writer with sufficient filesystem access can edit it. That is a
trust limit to describe accurately, not a reason to build a vendor collector. Local hash chaining
alone would not stop an attacker who can rewrite the whole chain and its unprotected anchor.

### SA-08: Pause between observation and effect needs a race test

Source concern, not a reproduced failure. The executor checks runtime authority in `authorize`,
but some paths then perform a browser observation before dispatching the effect. Generic
`dispatch` does not perform the same runtime check. Extension control-state presentation does
not itself establish an effect-admission interlock.

Use a browser test port that pauses after target observation and before the effect, trigger human
hold/end, and release the observation. Establish exactly which effects may still occur. The
existing [reference-experience intent](../MEMORY.md) already distinguishes current hold behavior
from the stronger intended pause/stop contract. Do not turn this source concern into a claim of a
proven universal pause bypass.

## Claim and evidence gaps

| Claim or proposed feature | Defensible statement at the reviewed source |
| --- | --- |
| Every composed action has a complete RAWX audit receipt | Children authorize, but flow/sequence completion loses child receipts. SA-02. |
| Audit never contains arbitrary page content | Failed flow leaks prior Read content. SA-03. |
| Uncertain effects are never automatically replayed | Script error fallback can evaluate effectful code twice. SA-04. |
| Repeated denials pause only the affected workspace | Threshold counting is local; the attention state is shared. SA-05. |
| Host policy is an origin/network boundary | Policy matches hosts at implemented decision points; full frame/network coverage is not established. SA-06. |
| Audit identifies an authenticated agent | It records workspace, claimed channel, and optional observed peer basename. Upstream agent identity is not authenticated. |
| Every allowing grant is recorded as positive provenance | Current attribution is strongest for denials; successful `Decision::allow` paths do not generally retain the deciding grant. |
| Signed policy defeats all rollback | It rejects rollback against retained trusted sequence state. It cannot protect that state from an adversary controlling its storage and execution. |
| Local audit is immutable or guaranteed durable | It is an ordinary local append file, and later write errors are discarded. SA-07. |
| A signed MCP connector proves a trusted upstream client | It authenticates an artifact, not who drives it or what that controller intends. |
| UI effects prove authorization or prevent prompt injection | They provide visibility and local controls, not those stronger guarantees. |

The relevant public descriptions are [security-overview.md](../trust/security-overview.md),
[data-flows.md](../trust/data-flows.md), and the [SIEM guide](../guides/siem-integration.md).
Recording this assessment does not repair their overclaims. Remediation must bring code and
active claims into agreement; historical ADRs remain evidence and are not silently rewritten.

## Verification performed

The existing orchestrator library and extension suites passed at the reviewed source:

```text
cargo test -p ghostlight --lib --locked --target-dir .target-ghostlight-1.0
360 passed

npm test --prefix extension
171 passed
```

These results did not detect the defects above. Full workspace gates, live browser journeys,
installed ACL behavior, fuzzing, and penetration testing were not performed as part of this
assessment. No production code, deployed binary, browser registration, or external service was
changed during the review.

The Rust probe used the real `ApplicationExecutor`, `GovernanceFacade`, and `WorkspaceStore`
through public crate APIs, an in-memory audit sink, a no-op presentation port, and a fake browser
port. It used a temporary explicit policy and diagnostics path, not the machine's configuration.
The policy granted only Read on `example.com`. The fake browser returned a synthetic sentinel
and logged physical commands; it never contacted a site.

After opening an initial tab, the probe submitted:

```json
{
  "on_error": "stop",
  "steps": [
    {"id": "read", "tool": "browser_read", "arguments": {}},
    {"id": "blocked", "tool": "browser_execute", "arguments": {"script": "42"}},
    {"id": "after", "tool": "browser_navigate", "arguments": {
      "url": "https://example.com/", "new_tab": true
    }}
  ]
}
```

Observed output, formatted without changing values:

```json
{
  "audit_records_including_setup": 2,
  "flow_audit_allowed": true,
  "flow_audit_capabilities": [],
  "flow_audit_reason": "permitted",
  "flow_effect": "applied",
  "flow_status": "unknown",
  "page_text_in_audit": true,
  "physical_commands": ["open_tab", "read_document", "open_tab"],
  "reported_stopped": true,
  "step_statuses": ["succeeded", "blocked", "succeeded"]
}
```

The same harness then issued three direct Execute calls denied in the first workspace and
attempted Read-authorized navigation in a second workspace:

```json
{
  "other_workspace_reason": "runtime_attention",
  "other_workspace_status_after_first_workspace_denials": "attention_required"
}
```

The script probe loaded the actual extension evaluator, supplied CDP-shaped exception details
from a Node VM, and counted evaluation calls and synthetic increments. Its two inputs and
results are recorded in SA-04. These probes were exploratory, external to the repository test
suites; the scenarios must become durable regression tests when the relevant fixes are made.

## Proposed hardening order and open decisions

First settle the trust boundary. Do not equate a tool-limited malicious request with arbitrary
same-user native execution, and do not imply that installing Ghostlight constrains other browser
controllers. Client provenance is useful when it answers a concrete admission or diagnostic
question; no evidence here makes client signing the first hardening milestone.

Then prioritize the demonstrated failures in existing promises:

1. Repair flow stopping and replace unrestricted audit projection. Add regression cases that
   prove refused children do not dispatch, later steps do not run under stop, and payloads never
   reach the audit projection.
2. Route child operations through a shared completion seam with truthful child and aggregate
   receipts. Preserve one authority snapshot and lease, and test equivalent direct/composed work.
3. Remove script replay based on exception text and repair effect uncertainty. Prove no duplicate
   execution and add a Chromium confirmation lane.
4. Separate workspace attention from deliberate global human controls, and establish the
   observation-to-effect pause contract with a controlled race test.
5. Define frame/site coverage and carry sufficient browser facts to the orchestrator. Add a
   parent/child-host fixture whose content makes unintended disclosure observable.
6. Make audit failure visible and document its recovery behavior. Improve claimed/observed
   attribution only where it has a defined use, and review positive grant provenance.

A focused first milestone should prove one complete story: a permitted operation, a refused
operation, correct stop behavior, and a content-minimized receipt for each actual attempt. A
synthetic hostile page and deterministic model-request trace can make that evidence reproducible.
Do not describe such a trace as a successful live prompt-injection experiment unless one was run.

No epic ledger or new ADR is created by this review. The owner intends a hardening epic and is
discussing its scope. Client admission, caller identity, frame-policy semantics, human control,
and audit durability should be decided on their own product merits before tasks are committed.

## Follow-up: scope recorded in a ledger (2026-09-06)

After this assessment was recorded, the owner agreed to the bounded client-provenance direction
and requested a ledger entry. The [security-hardening ledger](../tasks/security-hardening/LEDGER.md)
now owns that agreement, conditional admission work, and the proposed remaining priorities.
The September 6 amendment to ADR-0105 records the decision. The earlier closing paragraph above
describes the state when the assessment was first written; it is not the current epic status.

The ledger review also corrected a stale project-memory statement about human controls. Current
`language/outcome.rs` contains both pause and stop directives, and ADR-0126 already decides refusal
on pause rather than a suspended caller. SA-08 remains a source concern about enforcement timing;
it is not evidence that pause/stop language is missing or that its product semantics are undecided.

## Follow-up: flow-stopping fix (2026-09-06)

The owner accepted SA-01's continuation defect as a bug to fix. Ledger task H2a adds the missing
loop exits after child-decode and execution failures under stop. Two executor regression tests
failed against the reviewed implementation, then passed after the fix; all 362 orchestrator
library tests passed. The change is local, uncommitted, and not deployed. Broader aggregate
reporting and the other assessment findings remain unresolved.

The same discussion distinguished expected partial effects at a website from the additional
local-audit copy in SA-03 and the automatic re-evaluation/false effect certainty in SA-04. Nothing
in those findings requires atomic browser transactions or promises to undo data already sent.

## Follow-up: complete hardening epic and next cycle (2026-09-06)

The owner subsequently accepted configurable handling of excluded embedded content, separate
notice preferences, and H3's script-correctness work as the next cycle. The
[epic](../tasks/security-hardening/EPIC.md) now contains the complete work breakdown and decision
index. Undecided packages receive [ideation](../tasks/security-hardening/IDEATION.md) before
their implementation cycle. ADR-0133 and ADR-0151 record the script and frame decisions.

The ledger remains the authority on actual fixes and evidence. Creating the epic does not change
the assessment's reproduction limits or establish that H3 or frame enforcement is implemented.

## Follow-up: H3 implementation evidence (2026-09-06)

The owner's execution instruction led to the H3 correction in
[script-evaluator.js](../../extension/lib/script-evaluator.js): source is parsed locally before
one effectful evaluation, and browser exceptions no longer authorize replay or a known no-effect
claim. The two new SA-04 regressions failed before the fix and now pass. The
[execution record](../tasks/security-hardening/h3-script-effect-truth.md) records full workspace
gates, 183 extension tests, and 19 real Chromium/MCP cases with explicit test-adapter limitations.
H2a is committed separately as `8103c69b`. Neither fix is deployed or published, and the other
assessment findings remain open under the epic's ideation process.

## Follow-up: H1 readable audit correction (2026-09-06)

After ideation, the owner accepted readable bounded history with the existing target-name policy,
separate display and retention concerns, and no new profile selector or diagnostic capture mode.
The [H1 record](../tasks/security-hardening/h1-readable-audit.md) documents the implemented typed
audit projection, regressions that reproduced SA-03 before the fix, and actual JSONL verification
through the MCP/process journey. All 447 Rust tests and 183 extension tests pass. Browser error text
also stays out of new retained summaries. Earlier records remain untouched and may contain the
historical content; this is a local source correction, not a deployment or publication claim.
Other findings, including aggregate truth and missing child receipts, retain their open scope.
