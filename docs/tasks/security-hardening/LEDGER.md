# security-hardening LEDGER

Durable scope and progress for the [security-hardening epic](EPIC.md). This file is the
authority on what is agreed and what has actually happened. Recommendations remain labeled as
proposals until the discussion settles them.

Opened: 2026-09-06. Review baseline: `48ef29ece1ff0f1633daba62a03932a666f00ca5`.

## RESUME HERE

The owner requested a complete epic covering the assessment and all subsequent decisions, and
accepted H3 as the next implementation cycle. The [epic](EPIC.md), [bootstrap](BOOTSTRAP.md), and
[ideation agenda](IDEATION.md) now organize that work. Undecided packages require an ideation
session before their implementation cycle; do not silently adopt the earlier recommendations.

Next cycle: [H3 script effect truth](h3-script-effect-truth.md). Its behavior is agreed and the
September 6 amendment to ADR-0133 records the decision. Reproduce and fix unsafe script replay
and false no-effect reporting while preserving supported script behavior. H3 implementation and
validation are now in progress under the owner's "Onwards. Implement." instruction; its separate
commit and final evidence record are pending.

The owner identified flow stopping as a bug to fix. [H2a](h2a-flow-stop.md) now fixes the two
missing loop exits in the working tree, with regression evidence and 362 passing orchestrator
library tests. It is committed as `8103c69b`, with full workspace gates and the process journey
now passing; it is not deployed. Broader H2 aggregate reporting remains proposed.

H1's proposed local-audit correction and H2b's aggregate behavior go to ideation before their
cycles. H3's distinction from expected partial effects is now accepted. No transactional rollback
requirement has been added, and no deployment or publication has been made for this epic.

Client provenance remains reporting-first, with signer/hash admission conditional on a concrete
integration and verifiable subject. ADR-0105 records that direction. C1's detailed fields and
C2/C3's integration decisions are ideation items, not reasons to infer an upstream identity.

The owner then established the product focus: Ghostlight enables delight through integrated
tooling, and governance supplies dependable boundaries chosen by individuals and organizations.
The answers below apply that focus to the open questions. They remain recommendations except
where a later entry explicitly records agreement, including H6's policy choices.

The owner agreed that host authority applies to the document actually accessed (H6's subject
boundary), then accepted policy-selectable exclusion handling and separate human notice
preferences. The agreed choices and starting profile are recorded under H6 below and in the
[H6 design](h6-frame-coverage-ux.md). Existing grants decide access; coverage remains truthful.
Exact schema, UI details, and capture handling remain open. H6 is not implemented.

## Cycle readiness

| Package | Current disposition | Before its implementation cycle |
| --- | --- | --- |
| H2a | Fixed and committed as `8103c69b`; not deployed | Required gates and process journey pass. |
| H3 | IN PROGRESS; next separate commit | Finish its task brief and evidence; discuss only a newly discovered product tradeoff. |
| H1 | Included; remedy/priorities still proposed | Ideation I1. |
| H2b | Included; aggregate semantics still proposed | Ideation I2; H2a remains accepted. |
| H4 | Included; receipt design still proposed | Ideation I3; depends on H1 safe projection and H2b semantics. |
| H5 | Included; existing human-control contract retained | Ideation I4 for scope/recovery details and timing evidence. |
| H6 | Policy direction AGREED; details open; not implemented | Ideation I5 for scope evidence, schema, disclosure, notices, and capture. |
| H7 | Included; behavior still proposed | Ideation I6 for visible failure, strict policy, and recovery. |
| H8 | Included; investigation still proposed | Ideation I7 for bounded cases and evidence-driven controls. |
| C1 | Reporting direction AGREED; details open | Ideation I8 and a safe audit projection before adding durable fields. |
| C2/C3 | Admission direction CONDITIONAL; mechanism open | Ideation I9 chooses a concrete integration and connection proof. |

Ideation can proceed alongside independent agreed work. Inclusion in the epic does not imply
that all packages are ready for unattended implementation.

## Evidence at opening

The [assessment](../../design/security-assessment-2026-09-06.md) contains the full architecture
review, source links, probe inputs and outputs, and claim reconciliation inventory.

- Reproduced with real orchestrator APIs and an isolated fake browser: a flow continues after a
  reported stop; its child operations produce no individual audit records; a prior successful
  Read's synthetic page text appears in the failed flow's audit record; workspace A's repeated
  denials place workspace B under attention.
- Reproduced with the actual extension evaluator and a Node VM sender: an exception containing
  `Illegal return statement` causes two evaluations and two increments; a runtime-thrown
  `SyntaxError` after an increment is reported with false no-effect certainty.
- Source findings requiring further evidence: per-frame host coverage, ignored audit-write
  errors and their visible consequences, the observation-to-effect pause race, and local service
  resource bounds. These do not all have demonstrated live exploits.
- The earlier assessment ran 360 orchestrator library tests and 171 extension tests, all passing.
  It did not run the full workspace gates or a live Chromium security journey.

## Agreed boundary and client provenance (2026-09-06)

The owner accepted the following direction after discussing same-user host automation and
certificate-based client admission. This is product scope, not evidence of implementation.

1. Ghostlight governs its own invocation route. Independent host/desktop control, including
   mouse and keyboard automation, can bypass that route. Client admission does not claim to
   contain that attacker or provide endpoint-wide audit.
2. Preserve claimed application identity separately from the observed connecting executable,
   verified signer, executable digest, and verification result. Record unavailable evidence
   honestly. Newly observed certificates are not automatically trusted.
3. Permit optional policy admission by verified signer identity or exact executable SHA-256.
   Validate the executable's signature before trusting its certificate. The exact signer-key
   representation, policy spelling, and verification cache are still design work.
4. Preserve all-open defaults. Managed and local admission restrictions intersect and never raise
   browser capability ceilings. A required identity that cannot be verified refuses admission.
   Equivalent invocation routes must not bypass the restriction. Verification stays offline and
   states its revocation limitations.
5. Initially enforce against actual direct peers when a concrete integration can exercise that
   contract. In ordinary MCP usage the service peer is Ghostlight's connector; authenticating it
   does not authenticate the upstream application. A signed interpreter similarly does not prove
   the provenance of the script or instructions it runs.
6. If the desired rule names existing upstream MCP applications, first prove how that identity
   binds to the actual stdio connection. Reported names, supplied certificates, and parent-process
   observations are insufficient authorization. A direct signed integration or an enrolled
   integration credential are possible directions, not selected implementations. A credential
   proves possession, not executable provenance, and requires participating clients.
7. The signed-success verification lane needs a real signed subject. An unsigned artifact pinned
   by hash proves only the hash rule. This work does not require Ghostlight to procure a signing
   certificate as a blanket release-readiness gate.

| ID | Work package | State | Acceptance evidence |
| --- | --- | --- | --- |
| C1 | Accurate client-provenance reporting | AGREED DIRECTION; not implemented | Claimed and observed identities remain distinct through admission, workspace, and content-minimized audit. An ordinary MCP connection identifies the connector honestly. Unknown and failed verification have explicit outcomes. |
| C2 | Optional signer/hash admission for direct peers | AGREED, CONDITIONAL; not implemented | A concrete allowed peer and negative controls for a different signer/hash, tampering, unavailable evidence, and alternate invocation paths. Layer intersection and all-open remain intact. |
| C3 | Upstream MCP identity binding | AGREED PREREQUISITE if application-specific enforcement is pursued; mechanism open | A real supported client connection demonstrates an identity bound to that connection. The proof distinguishes application identity from connector, launcher, interpreter, and credential identity. |

C1 depends on a safe audit projection before extending durable records. C2 depends on C1 and its
concrete verification subject. C3 is a prerequisite for advertising upstream application admission,
not a prerequisite for honest reporting of a direct peer. No additional process, published service
protocol, client credential system, or product-wide certificate requirement is selected here.

## Work packages and priority

P0 means a demonstrated violation of an existing effect or confidentiality promise. P1 restores
complete enforcement and evidence or resolves a source finding with substantial possible impact.
P2 is focused hardening or further assurance after the demonstrated defects. These are scheduling
priorities, not CVSS scores. H3 is the selected next cycle. H2a is implemented locally, and
provenance/H6 have agreed direction. The order after H3 remains a recommendation to settle with
the relevant ideation session and dependencies.

| Order | ID | Priority | Work package | Evidence and reason |
| --- | --- | --- | --- | --- |
| 1 | H3 | P0 | Prevent script re-evaluation and false no-effect reports | AGREED NEXT. Reproduced SA-04; preserve supported scripts and prove the fix in Chromium. |
| 2 | H1 | P0 | Keep payloads out of audit | Reproduced SA-03: a failed flow persists earlier Read content. Ideation before implementing the proposed correction. |
| 3 | H2 | P0 | Make flow stopping and aggregate effects truthful | H2a stop fix agreed and verified locally; H2b aggregate reporting awaits ideation. |
| 4 | H4 | P1 | Complete child operation receipts | SA-02: flow reproduction and sequence source evidence. Policy checks happen, but terminal child records are missing. |
| 5 | H5 | P1 | Correct workspace attention and prove human-control timing | Reproduced SA-05 cross-workspace attention; SA-08 pause race still needs a controlled test. |
| 6 | H6 | P1 | Define and enforce composed-frame policy subjects | Per-document authority, selectable exclusion handling, and notice choices agreed; not implemented. Browser proof and detailed contracts remain outstanding. |
| 7 | H7 | P1 | Expose audit failure and define recovery | SA-07 source finding. Completion ignores append errors and UI history can outlive durable recording success. |
| 8 | H8 | P2 | Verify local admission and resource bounds | Review file permissions, unauthenticated idle connections, framing, cancellation, and simultaneous callers with bounded process tests. No exhaustion exploit is established. |

### H1: Audit confidentiality

Discussion clarification: partial input already received by the website is an expected effect of
a non-atomic browser operation. SA-03 concerns a different destination: Ghostlight copies an
earlier Read result into its local audit after a later child fails. Its priority remains under
discussion. The proposed correction does not remove prior website effects or promise rollback.

Replace unrestricted `facts` copying with a closed audit projection at its owning seam. Review
summary text and raw browser exceptions as well as structured facts. Pin permitted fields and
use synthetic sentinels in page content, scripts, form values, file paths, and error text. Exercise
ordinary and composite success/failure and serialization. Keep the existing bounded target-name
exception and its monotonic privacy setting explicit.

Exit evidence: forbidden sentinels appear in intended tool results where applicable and are absent
from every corresponding serialized audit record. No whole-result JSON reaches audit by default.
Do not erase existing audit files as part of the fix; any historical-data cleanup is separate work.

### H2: Flow control and aggregate truth

H2a is agreed and verified locally; see its execution record below. The remaining aggregate
status/count/wording work is H2b, has not been implemented, and goes through ideation I2.

Cover a governed refusal, a decoded-operation error, and a reference error followed by independent
work. Under stop, no later step dispatches. Under continue, the result still truthfully reports
which children failed and which effects happened. A denied child must remain denied; an already
applied effect must not be erased from the aggregate account.

Exit evidence: command logs, child statuses, completed counts, and aggregate status/effect agree
for all cases. Remove the current flag-only stop behavior and inconsistent aggregation rules.

### H3: Script effect truth

Agreed next cycle (2026-09-06): after the earlier discussion of partial effects, the owner accepted
the proposed H3 correction and requested the complete epic. SA-04 concerns an extra evaluation
initiated by Ghostlight after an error and false no-effect certainty after a runtime exception.
The September 6 amendment to ADR-0133 records the accepted behavior. See the
[H3 task brief](h3-script-effect-truth.md); implementation and fresh verification remain pending.

Choose the supported script interpretation without executing possibly effectful code twice.
Do not use exception text or a runtime exception class as proof of a parse-only failure. Preserve
uncertainty whenever evaluation may have begun.

Exit evidence: the two SA-04 scripts each evaluate once and never report a known no-effect result
after changing the sentinel. Genuine supported bare-return inputs still behave as documented.
Add an actual Chromium CDP lane after focused evaluator tests; label each lane separately.

### H4: Composition completion and receipts

Depends on H1 for safe projection and H2 for stable flow semantics. Route ordinary and composed
children through one completion seam under the parent's snapshot and lease. Capture child
identity/order, actual RAWX requirements, policy decision, and terminal effect. Define aggregate
receipts without losing refusals or implying that an allowed empty wrapper approved every child.
Review positive grant provenance alongside the existing denial attribution.

Exit evidence: equivalent direct and composed operations produce equivalent child decisions and
safe terminal receipts; each attempted child completes once; unattempted later steps are not
reported as executed. No recursive top-level execution reacquires the lease or changes authority.

### H5: Runtime control and workspace scope

Keep deliberately global human controls separate from workspace-local automatic attention.
Verify two workspaces through the shared executor/facade, then cover composed denials after H4.
Use a gated browser port to trigger pause/end after target observation but before effect dispatch;
verify cancellation and already-dispatched effects retain their correct terminal account.

ADR-0126 already decides refusal on pause and the fixed stop directive. Both directive constants
exist in current `language/outcome.rs`; rebuilding those features is not this package's purpose.

Exit evidence: A's denial threshold does not stop B; human controls have their intended scope;
the controlled timing test establishes what can dispatch and preserves truthful prior effects.

### H6: Frame/site coverage

Agreed scope (2026-09-06): apply the chosen boundary to the document actually accessed. The
September 6 amendments to ADR-0151 record that principle and the subsequently accepted policy
direction. The owner requested that these choices be added to the ledger:

- Exclusion handling: use permitted content; require complete access for the operation; or
  require complete access for the page.
- Human notices, managed separately: on demand; when work is affected; or whenever content is
  excluded. Quiet presentation retains truthful structured coverage for the client.
- Starting profile: use permitted content, with human notices when work is affected.
- Existing host/RAWX grants decide access, including readable fields with forbidden edits.
  Handling and notice settings do not override those grants. Lower tiers cannot weaken an
  organizational requirement; the effective setting and its source are visible.
- All-open remains straightforward. These choices neither prevent website network activity nor
  make multi-step browser operations atomic.

This direction is agreed, not implemented. Exact setting keys, result schemas, detailed UI,
bounded host disclosure, and reliable capture exclusions remain open in
[h6-frame-coverage-ux.md](h6-frame-coverage-ux.md).

Start with a permitted-parent/denied-child fixture and verify exactly which observation and
interaction routes cross the child's host boundary. Settle the policy subject for text, semantic
targets, and screenshots before choosing the contract. Browser adapters report frame facts and
perform physical filtering; the orchestrator makes the policy decision. Do not accidentally claim
browser-wide network filtering, DLP, or semantic transaction authorization.

Exit evidence: a real composed-frame journey proves the selected policy contract and all-open
still observes the complete supported page. This source finding may move up in priority if the
probe establishes broader disclosure than the currently demonstrated defects.

### H7: Durable audit health

Inject a failing sink and verify what the tool result, workbench, and durable file actually say.
Define visible health and recovery, including whether a configured strict policy affects later
admission. Failure after an effect cannot be reported as if that effect never occurred. Coordinate
with H1/H4 so both parent and child receipt failures have an honest account.

Exit evidence: durable failure is visible, prior effects stay truthful, and recovery behavior is
specified and tested. Local append-only JSONL is not described as tamper proof. No vendor audit
collector or local hash-chain feature is assumed necessary.

### H8: Bounded local service

Confirm installed discovery-file access on supported platforms without reading owner-local notes.
Inspect pre-authentication waits, frame limits, thread/invocation admission, cancellation, and
reconnect cleanup. Use bounded isolated process tests, including another legitimate workspace
remaining usable under malformed or abandoned requests. Add limits for demonstrated gaps rather
than inventing a general scheduling framework.

## Cross-cutting acceptance

Each implemented package updates affected active trust claims with its evidence. The final proof
should connect a permitted operation, a refused operation, correct continuation/stop semantics,
and a content-minimized receipt for each actual attempt. A deterministic hostile-page request
trace is valid fixture evidence; call it a live prompt-injection result only if that was tested.

Every implementation task records its own commit and required checks under the bootstrap. Full
workspace or live-browser success is never inferred from the earlier assessment's partial suites.

## Proposed answers under the product focus (2026-09-06)

The owner asked how the product focus changes the answers to the earlier questions. The following
recommendations turn it into expected behavior. The existing accepted scope and H2a fix remain
as recorded above. H3's correction and H6's policy/notice choices were subsequently accepted; other additional
design choices remain proposed unless explicitly recorded otherwise.

| Open issue | Recommended answer | Intended user experience |
| --- | --- | --- |
| Non-atomic work and recovery | Preserve completed, failed, unattempted, and uncertain work distinctly. Retry only with evidence that repetition is safe; an error message is insufficient evidence. | A failure explains where work ended without undo claims or unexpected repeated effects. |
| Audit confidentiality | Keep a closed metadata record of operations, policy decisions, and effects. Preserve the existing governed target-name exception. Exclude full page results, entered values, and arbitrary exception text from durable audit. | Useful history without creating an unexpected content archive. |
| Composite receipts | Give each attempted child a safe terminal receipt and group those receipts under one user-level operation. Clearly mark unattempted steps; do not fabricate execution records for them. | The default history stays readable, with enough detail available to explain a failure. |
| Audit-write failure | Default operation continues with visible degraded history and bounded recovery. If durable audit is explicitly required by policy, a known recording failure refuses new browser work until recording recovers; status and human controls remain available. Never erase an already-dispatched effect from the account or claim perfect atomic coupling between effects and disk writes. | An individual gets an honest degradation; an organization gets the prerequisite it deliberately configured. |
| Mixed-host frames | Apply authored host/RAWX rules to each document. Make the response to exclusions selectable: useful permitted content, complete operation scope, or complete page scope. Choose human notice intensity separately; results retain truthful coverage. | People and organizations choose how partial access affects work and attention, with the effective choice and its source visible. |
| Screenshots of restricted content | Apply the same disclosure boundary. Omit restricted areas only where that exclusion can be enforced reliably; otherwise refuse the capture and name a permitted alternative where one exists. | A screenshot does not quietly bypass the restriction on reading the same content. |
| Workspace and human controls | Automatic denial attention stays with its workspace. Explicit human global controls retain global scope. Prevent subsequent dispatch while preserving truthful facts about already-dispatched work. | One troubled task does not interrupt unrelated tasks; a person's deliberate stop remains dependable. |
| Client identity and certificate lifecycle | Keep the agreed reporting-first approach. Use verified cryptographic identities rather than display names. Approved signers may cover ordinary updates; identity rotation needs a policy update; exact hashes remain an optional tighter pin. Required evidence that a platform cannot supply refuses the restricted admission. Cache verification only with sound binding to an unchanged artifact. | Familiar updates need little maintenance, and changes to the actual trust identity are legible. |
| Local resilience | Bound abandoned connections, requests, and recovery at existing seams, using demonstrated failure cases to choose limits. | A misbehaving client cannot consume the service indefinitely or make other work mysteriously stall. |

These frame recommendations cover Ghostlight's own observations and actions, not every network
request made by a website. All-open operation still covers the complete supported composed page.
Mechanism capabilities remain RAWX; Ghostlight does not infer whether a generic click means
buying, sending, or another business action. No general intent classifier or per-action approval
ritual is introduced by this focus.

H3 is now the selected next implementation cycle. The recommended subsequent grouping is audit
confidentiality and aggregate truth, then composition, frame boundaries, workspace control, audit
health, and local resilience. Honest provenance reporting uses the repaired receipt seam;
conditional signer admission follows a concrete integration. The epic maps those dependencies.
Run the affected package's ideation session before implementing its undecided choices.

## Decisions still open

- H1's proposed local-audit correction and the task order after the now-agreed H3 cycle.
- Broader H2 aggregation and other implementation tasks beyond the completed local H2a fix.
- Exact provenance fields, verification status vocabulary, signer pin representation, rotation,
  verification caching, and supported-platform behavior.
- Which concrete integration justifies C2; whether C3 is needed and which participating client
  can prove it. No client-private-key or handshake design has been selected.
- H6's exact setting keys, detailed tier resolution and scope evidence, result schemas, notice
  rendering, scoped negative answers, host disclosure, and capture exclusions. The three handling
  choices, three notice choices, starting profile, and no-weaker-lower-tier rule are agreed.
- Audit-write failure behavior and any configurable strictness after a failed append.

The [ideation agenda](IDEATION.md) expands these questions into package-specific sessions,
including H4/H5/H8. H3's correction, existing pause/stop semantics, provenance boundaries, and
H6's selected modes remain accepted context rather than unresolved questions.

## Activity record

| Date | Work | Result |
| --- | --- | --- |
| 2026-09-06 | Preserved the architecture/security review independently of events | Dated assessment with isolated reproductions and source-only findings; no production change. |
| 2026-09-06 | Discussed host control and client provenance; owner agreed and requested ledger entry | C1-C3 scope recorded; ADR-0105 amended with accepted direction and explicit deferrals. |
| 2026-09-06 | Ranked remaining work and checked current human-control contract | H1-H8 proposed. Corrected stale MEMORY wording: pause/stop directives already exist under ADR-0126; H5 concerns enforcement timing and scope. |
| 2026-09-06 | Owner accepted flow stopping as a bug; challenged framing of partial effects | Clarified H1/H3 without assuming agreement. H2a implemented and verified locally; no rollback mechanism added. |
| 2026-09-06 | Owner framed delight in integrated tooling and dependable chosen boundaries as the product purpose | Proposed concrete answers for partial effects, private audit, logging failure, frame coverage, scoped controls, and provenance. No further implementation or acceptance inferred. |
| 2026-09-06 | Owner agreed to document-specific host authority and asked how denied embeds should affect UX and trust | ADR-0151 amended for the subject boundary. H6 proposes permitted scoped results, explicit exclusions, truthful negative answers, quiet human notices, and consistent capture restrictions; no implementation started. |
| 2026-09-06 | Owner challenged fixed partial-result behavior and proposed policy flags for delight | H6 revised around selectable exclusion handling and separate notice preferences, composed through existing authority. Defaults and schema remain open; no production change. |
| 2026-09-06 | Owner accepted the policy choices and requested a ledger entry and current inventory | H6 handling modes, separate notice choices, and the use-permitted-content/notice-when-affected starting profile recorded as agreed. Detailed contracts and implementation remain outstanding. |
| 2026-09-06 | Owner accepted H3 next and requested one epic containing all discussion and decisions, with ideation before undecided cycles | EPIC, IDEATION, and the H3 task brief created; bootstrap, decision index, and readiness states reconciled. ADR-0133 amended for script correctness. No new production implementation or test run in this planning turn. |

## H2a execution record (2026-09-06)

Status: implemented and committed as `8103c69b`; not deployed.

- Exploration used the current flow, sequence, decoder, result types, executor fixture, and fake
  browser. ADR-0133 Decision 9 already requires the stop behavior; no new decision was needed.
- Before the fix, both added regression tests failed with two OpenTab commands instead of one:
  after a denied Execute and after a Read argument resolved to an invalid numeric value.
- `crates/orchestrator/src/work/flow.rs` now exits immediately after recording a child-decode
  failure or non-success child when `on_error` is `stop`. The existing reference-error exits
  provide the same behavior. The production change is two loop exits.
- `crates/orchestrator/src/work/mod.rs` adds two executor regression tests, including a matrix
  of decode/reference failures under explicit stop and continue. They verify physical command
  counts, retained earlier effects, failure rows, partial effect on stop, and no repeat-safe claim.
- Validation: `cargo test -p ghostlight --lib --locked --target-dir .target-ghostlight-1.0`
  passed all 362 tests. `cargo fmt --all -- --check` and `git diff --check` passed.
- H2a changes no extension, connector, policy, audit-projection, or script-evaluator code. Before
  its separate commit, formatting, workspace Clippy, workspace tests, extension tests, changed
  JavaScript syntax, and the fresh-build process journey passed. The orchestrator-only fix uses
  the deterministic fake-browser regression rather than claiming a live flow experiment.
- H1, H3, H4, and the broader H2 aggregate account remain unresolved. This entry completes the
  specific continuation bug, not the full hardening package.
