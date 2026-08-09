# ADR-0102: One Ghostlight surface and governed browser kernel

- Status: Superseded by ADR-0103
- Date: 2026-08-08
- Supersedes: ADR-0101 Decisions 2 through 5 and its vendor-profile rollout
- Amends: ADR-0022, ADR-0034, ADR-0060, ADR-0078, ADR-0080, ADR-0094, and ADR-0096
- Builds on: ADR-0005, ADR-0024, ADR-0037, ADR-0038, ADR-0042, ADR-0069, ADR-0093,
  ADR-0098, and ADR-0099

## Context

ADR-0101 correctly separated model-facing surfaces, service operations, and physical
browser mechanisms. Its next step was wrong. It proposed several model-facing dialects and chose
a twelve-tool Native surface by comparing existing vendor signatures. That preserved too much of
the accidental structure Ghostlight was meant to replace.

The clean-slate review in `docs/ubiquitous-language.md` found a simpler product language. A model
should express one browser job with a short, flat call. Ghostlight should materialize safe
defaults, refuse to guess authority or risky intent, report exactly what happened, and offer at
most two useful recovery moves. The same review of governance in `docs/governance-language.md`
found that policy, settings, protected hosts, runtime safety, audit, and model-facing recovery had
been described as one broad feature even though they have different authority and precedence.

Vendor imitation does not justify the resulting complexity. Vendor declarations change, are
only partly observable, and often do not publish an output contract. A guessed adapter creates a
maintenance and safety obligation without proving that a model completes work more reliably.
Ghostlight should be excellent in its own language. A vendor adapter may be proposed later only
with complete evidence and a measured improvement over the Ghostlight surface.

The service therefore needs one semantic browser kernel, one model-facing surface, and one
governance decision path.

## Decision

### 1. Ship one strategic surface

`ghostlight/v1` is Ghostlight's sole model-facing surface. It owns the
exact names, descriptions, flat input schemas, output schemas, defaults, examples, and recovery
copy defined by the accepted ubiquitous-language contract.

The inherited twenty-five-tool `ghostlight-legacy/v1` surface is removed. Its declarations,
decoder, renderer, selection path, agent guide, explain copy, goldens, and compatibility-only
tests are deleted. There is no installed tool-surface fallback and no period in which both
catalogs are advertised by the same build.

There are no Claude, Codex, Playwright, Gemini, or other vendor `SurfaceProfile` variants. There
is no client-name classifier, model-name classifier, version-range matcher, or automatic vendor
dialect selection. Claude, Codex, and other MCP clients use Ghostlight directly. Client-specific
installers, skills, or guidance may help a client connect, but they do not change the callable
browser language or semantic result.

Ghostlight is the unconditional surface. There is no surface
selection environment variable and no compatibility cutover switch.

A future vendor adapter requires a new ADR. It must provide dated primary evidence for inputs and
outputs, a complete mapping or omission ledger, exact fidelity tests, representative journeys,
and a measured reduction in invalid calls, turns, or unsafe repeats. Familiarity alone is not a
reason to add it.

### 2. Make the Ghostlight surface one-to-one with the kernel

The Ghostlight operation domain contains these twenty-four operations in this order:

1. `browser_get_status`
2. `browser_open_tab`
3. `browser_list_tabs`
4. `browser_focus_tab`
5. `browser_close_tab`
6. `browser_navigate`
7. `browser_go_back`
8. `browser_go_forward`
9. `browser_reload_page`
10. `browser_inspect_page`
11. `browser_read_page`
12. `browser_take_screenshot`
13. `browser_click`
14. `browser_hover`
15. `browser_scroll_to_target`
16. `browser_scroll_page`
17. `browser_press_key`
18. `browser_press_escape`
19. `browser_drag`
20. `browser_fill_form`
21. `browser_wait_for`
22. `browser_run_sequence`
23. `browser_get_dialog`
24. `browser_handle_dialog`

Each name identifies one Ghostlight operation. The service does not retain grouped product
families such as `browser.tabs`, `browser.act`, or `browser.dialog` as execution identities.
Small argument choices such as click count or dialog resolution remain typed fields within the
one operation; they are not surface names or policy lookup strings.

Specialist work remains outside the core until its pack has equally complete cards, schemas,
contracts, mechanisms, and tests. No stub or future-only operation is advertised.

### 3. Use typed Ghostlight calls and outcomes

The owner bridge carries a closed typed Ghostlight operation, not a surface name plus an untyped
argument object. Every operation has one Rust argument type with bounded fields and executable
defaults. Surface adapters validate external JSON, materialize defaults, and construct that type.
The service validates the typed semantic invariants again before admission.

A sequence contains typed Ghostlight child operations from its closed child set. It never carries
an external tool name. Every child argument is known before execution, all children use one tab,
and each child re-enters the normal operation contract. The whole sequence validates before any
browser work. A sequence has two through ten steps and no nested sequence.

The Ghostlight outcome has these independent facts:

- `status`: `ok`, `partial`, `not_met`, `blocked`, `held`, `attention_required`, `cancelled`,
  `not_dispatched`, `outcome_unknown`, or `unavailable`;
- `effect`: `none`, `dispatched`, `committed`, or `unknown`;
- `repeat`: `safe`, `unsafe`, or `after_state_change`;
- optional readiness, governed tab/workspace facts, provenance, safety-park receipt, and problem;
- exactly one typed per-operation result after a successful or partially successful dispatch;
- zero to two service-authored suggested next steps.

`repeat` is required on every Ghostlight outcome. `problem` is required for every non-normal
outcome. A dispatched `ok`, `partial`, or `not_met` result must carry the matching per-operation
payload. An outcome with unknown effect never suggests replay. Page content, adapter errors, and
secrets never author summaries or suggestions.

Execution returns one `OperationCompletion`. It contains the proven terminal disposition and one
closed `OperationResult` variant matching the admitted operation. Mechanism payloads are private
adapter evidence and are parsed exactly once by the operation that owns the work. They never cross
the owner bridge, never become operation state, and never serve as a later projection source.

Each operation owns its whole unit of work. A simple read may have one dispatch and one result. A
compound operation has an explicit lifecycle whose states retain every committed sub-effect. For
example, opening a tab with a URL distinguishes not started, tab created, navigation dispatched,
landing committed, navigation failed, and navigation outcome unknown. Once creation is proven,
every later terminal result retains the exact created tab and reports the creation effect even when
navigation fails or becomes uncertain.

One completion chokepoint validates the operation/result pairing, binds workspace-owned opaque
handles, applies final governed landing facts, and serializes the already-constructed result. It
does not infer operation facts from status, readiness, prose, mutable workspace state, or incidental
adapter fields. Direct calls and sequence children invoke the same operation executor and carry the
same result variants. There is no sequence-only result reconstruction path.

The extension owns policy-free physical transactions when Chrome requires observation to be armed
before an effect. `browser_open_tab` with a URL therefore maps to one physical open transaction:
arm lifecycle and top-level commit observation, create the tab directly at the requested URL,
return its exact identity and ordered document evidence, then let the service govern each landing.
It must not expose an intermediate blank page or implement a create-then-navigate workaround.
`browser_open_tab` without a URL preserves the browser's ordinary new-tab experience.

Runtime end-session before dispatch is `not_dispatched/none/after_state_change` with problem
`session_ended`. End-session after dispatch uses the same committed-or-unknown cancellation truth
as any other interruption. `not_met` includes a requested condition not becoming true, absent
dialog resolution, and an unavailable history move.

### 4. Make the operation contract exhaustive

The service registry owns one exhaustive contract row for every Ghostlight operation. `R` means
read, `I` means interact, `W` means write, and `X` means execute. `current` means the exact owned
top-level tab URL is resolved under the retained execution lease. `landing` means every committed
top-level document is governed before readiness or content observation.

| Operation | Capability | Resource | Workspace | Lane | Physical plan | Sequence |
| --- | --- | --- | --- | --- | --- | --- |
| `browser_get_status` | none | none | optional | local | service local | no |
| `browser_open_tab` | I | supplied URL when present | creates | topology | create, optional navigate plus readiness | no |
| `browser_list_tabs` | R | each returned tab | uses | topology | inspect inventory, then filter facts | no |
| `browser_focus_tab` | I | named tab current URL | uses | topology | focus | no |
| `browser_close_tab` | I | named tab current URL | uses | topology | close | no |
| `browser_navigate` | I | target URL plus every landing | creates or uses | surface | navigate plus readiness | yes |
| `browser_go_back` | I | current plus every landing | uses | surface | back plus readiness | yes |
| `browser_go_forward` | I | current plus every landing | uses | surface | forward plus readiness | yes |
| `browser_reload_page` | I | current plus every landing | uses | surface | reload plus readiness | yes |
| `browser_inspect_page` | R | current | uses | surface | snapshot or find | yes |
| `browser_read_page` | R | current | uses | surface | read text | yes |
| `browser_take_screenshot` | R | current | uses | surface | viewport or resolved-target crop | yes |
| `browser_click` | I | current | uses | surface | resolve, cue, click, settle | yes |
| `browser_hover` | I | current | uses | surface | resolve, cue, hover, settle-if-changed | yes |
| `browser_scroll_to_target` | I | current | uses | surface | resolve, cue, scroll target | yes |
| `browser_scroll_page` | I | current | uses | surface | bounded wheel scroll | yes |
| `browser_press_key` | I | current | uses | surface | resolve, cue, key press, settle | yes |
| `browser_press_escape` | I | current | uses | surface | Escape key press, settle-if-changed | yes |
| `browser_drag` | I | current | uses | surface | resolve both, cue, drag, settle | yes |
| `browser_fill_form` | W | current | uses | surface | preflight, revalidate/write each, optional exact submit, settle | yes |
| `browser_wait_for` | R | current | uses | surface | condition wait | yes |
| `browser_run_sequence` | union | one child tab | creates or uses | composition | Ghostlight children | no |
| `browser_get_dialog` | R | current | uses | surface | inspect dialog | yes |
| `browser_handle_dialog` | I | current | uses | surface | accept, dismiss, or respond | yes |

`browser_fill_form` remains W even when `submit_target` is omitted. A page may assign broader
business consequences to a declared write; Ghostlight does not claim those consequences are
bounded. Hover and scroll are I because they send page input and may change state. No core
operation requires X.

The contract also owns result type, page-output provenance, safety annotations, and final
admission requirements. Architecture tests exhaust the operation enum against the contract,
mechanism plan, Ghostlight declaration, sequence child set, and result type.

### 5. Preserve scheduler and runtime-control authority

The action pipeline keeps ADR-0080's order:

1. validate the typed operation and identify its scheduling resource;
2. admit it to the bounded fair queue and record the authority epoch;
3. acquire the required execution lease;
4. retire it without dispatch if the epoch changed while queued;
5. capture the current immutable authority snapshot and request restriction;
6. resolve governing resource evidence under the lease;
7. evaluate protected hosts, service policy, and request restriction;
8. perform final live ownership, browser generation, hold, attention, and panic checks;
9. dispatch physical mechanisms;
10. govern every committed landing or result resource, reconcile the Ghostlight outcome, audit,
    and render.

Ownership is live state and is never frozen inside the authority snapshot. The immutable snapshot
contains resolved settings, normalized policy, source identity, organization presentation, and
epoch. The request restriction is immutable work context beside it.

The extension remains policy-free. It receives only typed physical mechanism/control messages and
returns browser evidence. It never receives a surface profile, policy rule, capability, decision,
or model-facing suggestion.

### 6. Separate policy, settings, protected hosts, runtime controls, and audit

Ghostlight capability names are `read`, `interact`, `write`, and `execute`. The legacy schema-3
capability `action` normalizes to `interact` without changing legacy bytes.

External policy and settings formats are compatibility inputs. They normalize into one internal
model before becoming authority. A policy is an ordered ruleset. A rule explicitly allows or
denies covered hosts and carries a capability set when that distinction applies. The schema-3
`allowed: []` case remains lossless: it normalizes to an explicit deny barrier and stops ordered
evaluation exactly where the inherited grant did.

Policy selection and settings collection are separate:

- collect managed-required, machine-required, user, managed-default, machine-default, and product
  default settings independently;
- select the first present policy in managed, machine, explicit user source, environment user
  source, then no-policy order;
- a settings-only source does not stop policy fallback;
- a present policy with zero rules does stop fallback and means governed block-all;
- no policy means browser work is permitted, while audit remains separately configured.

Ordinary settings use the stated precedence. `safety.protected_hosts` is a deny ceiling and uses a
normalized bounded union of product, user, machine-required, managed-required, and request
restriction entries. It is not legal in a default/recommended organization layer because such an
entry would not be overridable and therefore would not be a default.

Runtime hold, denial attention, and end session remain live controls outside policy. Presentation
quieting never changes authority.

Audit has one enable decision and one selected destination. Destination-specific settings are
validated only when that destination is active. The Ghostlight audit record separates ordered
governance decisions, complete capability set, restriction identity, runtime control, execution
status, effect, repeat, scheduling, and correlation. It never records content payloads or opaque
workspace authority.

### 7. Make resource failure and tab inventory fail closed

A page-scoped operation must prove the current top-level resource before authorization. Missing
ownership, browser-channel failure, an indeterminate URL, and an unsupported URL class are
different typed failures. An indeterminate protected-host check never falls through as all-open.

`browser_list_tabs` is authorized to inspect topology, but URL and title are per-tab page facts.
The service evaluates each tab's exact current resource under the same authority snapshot and
request restriction before exposing those facts. A protected, denied, or indeterminate tab keeps
only its opaque owned handle and a stable redaction reason. It exposes no URL or title. The list is
bounded and preserves owned-tab order.

Opening a tab returns only the created-tab receipt. It does not return the entire existing
inventory.
The browser adapter must identify the created tab in the operation's root receipt. A shared tab
inventory may confirm and atomically claim related tabs, but it can never select the creator's
result tab. Missing exact creator identity fails closed instead of guessing from the active tab,
the first inventory row, or the workspace current tab.

Cross-origin child-frame targets remain excluded from the core. Inspection may return top-level
and same-origin targets only. A later multi-resource frame design requires a new ADR with separate
child-origin authorization, ref binding, routing, and audit.

### 8. Keep Ghostlight input simple and output useful

The exact Ghostlight declarations come from the accepted ubiquitous-language schema catalog. The
model-facing schemas remain flat and typo-closed. Cross-field contradictions are checked by the
edge with one corrective example instead of conditional JSON Schema trees.

Safe defaults are Ghostlight semantics:

- omitted `tab` uses the workspace current tab only when one is established;
- `browser_navigate` may create the first workspace/tab, but never an additional tab by accident;
- navigation settles adaptively for at most ten seconds;
- interactions use a short adaptive post-action settle where relevant;
- page reads default to 20,000 characters;
- inspection defaults to interactive detail;
- wait requires one target or text condition and is never a generic delay;
- fill never submits unless `submit_target` names the exact control;
- credentials, close, submit, unsafe execution, and authority are never inferred.

Ordinary success returns no suggestion unless a new useful choice exists. Recoverable failures
return at most two safe, schema-valid moves. Stale targets suggest inspection. An unavailable tab
suggests listing tabs before opening another when inventory is unknown. Credential targets suggest
human entry. A committed readiness timeout suggests a specific observation, never replay.

### 9. Preserve protocol and browser-adapter boundaries

MCP revision state stays in `ghostlight-mcp-connector`. The service receives and returns only the
typed Ghostlight product vocabulary.

Both supported MCP revisions expose the same Ghostlight declaration and result semantics. MCP
`2025-11-25` retains its initialized connection lifecycle. MCP `2026-07-28` retains explicit
request-local workspace continuity. Neither revision selects or stores a tool-surface profile.

The physical mechanism wire, adapter feature negotiation, and compatibility serializer remain
independently versioned under ADR-0093. Rebuilding the operation kernel does not create an
extension flag day.

Feature evidence is exact and additive. A service must never infer support for a new mechanism
from a related older feature. Each independently introduced physical behavior has one exact
browser-identity feature, captured with the browser-session generation before serialization and
rechecked at final enqueue. Atomic URL tab opening therefore requires `mechanismRequestV1`,
`navigationReadinessV1`, and its own `atomicTabOpenV1`; an older loaded worker receives no frame and
the Ghostlight operation returns a no-effect capability-unavailable result.

## Implementation order

1. Correct the ubiquitous-language and governance-language contracts and accept this ADR.
2. Add typed Ghostlight calls and outcomes to transport; bump the owner bridge major.
3. Replace the grouped operation registry with the exhaustive twenty-four-row contract.
4. Rebuild the pipeline around typed admission, authority, execution, and reconciliation.
5. Normalize inherited policy/settings inputs into Ghostlight governance decisions and audit.
6. Implement all twenty-four Ghostlight declarations, decoders, renderers, and sequence children.
7. Remove the inherited surface, grouped-operation draft, selector, and vendor-adapter scaffolding.
8. Make Ghostlight the only catalog and run the complete gate set.

## Required evidence

- exact Ghostlight declaration, description, schema, default, ordering, and agent-guide goldens;
- exhaustive operation-contract, mechanism-plan, result-payload, and sequence-child tests;
- direct and nested equivalence for every sequence-eligible operation;
- policy compatibility tests for schema-3 host polarity, first match, `allowed: []`, and
  `action -> interact`;
- settings source/precedence and protected-host union tests;
- governed mixed-origin tab inventory tests;
- scheduler/authority/final-admission race tests;
- navigation ready, timeout, unavailable, refused landing, unknown, cancellation, and park tests;
- terminal and recovery tests for hold, attention, end session, stale target, credentials,
  browser outage, closed tab, partial commit, and unknown effect;
- both MCP revision journeys with the same Ghostlight semantics;
- new-service/covered-old-adapter and covered-old-service/new-adapter Lightbox scenarios;
- lower-capability model journeys measuring invalid calls, redundant waits/reads, repeated
  failures, unsafe repeats, turns, and returned bytes;
- full formatting, strict Clippy, Rust workspace, extension JavaScript, schema, diff, and ASCII
  gates.

## Consequences

Ghostlight owns a coherent product language instead of maintaining speculative vendor dialects.
The service and governance model become easier to reason about because every core tool is one
typed operation with one contract and one result. Clients receive the same truthful semantics and
the same concise recovery language.

The migration is intentionally large. The bridge, registry, pipeline, governance records, Ghostlight
surface, tests, and documentation all change together. The mechanism wire limits the browser-side
blast radius, and the extension remains a thin policy-free executor.

Vendor familiarity is no longer assumed to be valuable. It must earn future complexity through
complete evidence and evaluation.
