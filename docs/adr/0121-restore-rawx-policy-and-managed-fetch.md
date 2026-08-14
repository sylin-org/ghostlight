# ADR-0121: Restore RAWX policy and managed fetching for 1.0

- Status: Accepted
- Date: 2026-08-14
- Amends: ADR-0013, ADR-0022 through ADR-0025, ADR-0055, ADR-0060, ADR-0079,
  ADR-0096, ADR-0102, and ADR-0107 Decision 2
- Supersedes: the clean-room 1.0 flat version-1 policy implementation and the standing deferral of
  remote managed-policy fetching

## Context

The clean-room 1.0 rebuild preserved one executor and a governance facade, but it did not preserve
the policy product that made 0.8 ready for organizational adoption. The live implementation treats
Read, Action, Write, and Execute as an ordered scale, reduces each operation to one highest value,
and applies global host and capability lists. It has no schema-3 grants, per-grant hosts,
observe/enforce behavior, explain or simulation, policy-aware discovery, stable denial attribution,
hot authority swap, denial attention, or signed managed distribution.

That simplification is not a new product direction. It is lost behavior. In particular, classifying
a submitted form as Execute confuses declared form work with arbitrary page code, and selecting the
maximum capability for a sequence discards the independent-set model ADR-0022 established.

The owner requires the full policy capability map and remote policy fetching in 1.0. The restoration
must retain the 1.0 process boundaries and terse 22-tool language, avoid rebuilding Lightbox or old
demo machinery, and leave the all-open experience unchanged.

## Decision

### 1. RAWX is an independent capability set

Read, Action, Write, and Execute are independent facts, not ranks or tiers. An operation requires a
set. Authority admits the operation only when one applicable grant contains the complete set.
Absent classification denies. An empty requirement set admits without a capability grant, subject
to runtime control and permanent protected-resource ceilings.

One orchestrator-owned action directory is the source of classification for enforcement, audit,
policy-aware catalog projection, explain, simulation, and tests. Compound tools use exact variants:

| Operation | Required set |
| --- | --- |
| tabs list | Read |
| tabs focus | empty |
| tabs close | Action |
| navigate, including a new tab | Read |
| history back, forward, or reload | Action |
| window zoom | Read |
| window resize | empty |
| read, inspect, find, screenshot, scroll, hover, wait, diagnose | Read |
| click, type text, press key, drag | Action |
| fill form | Read + Write |
| fill form and submit | Read + Write + Action |
| upload files | Write |
| execute page JavaScript | Execute |
| dialog status | Read |
| dialog accept, dismiss, or respond | Action |
| recording start | Read |
| recording status, stop, or discard | empty |
| recording save to client or download | Read |
| recording save to a page target | Write |
| sequence wrapper | empty; each step is admitted and audited independently |
| policy explain | empty |

This amends ADR-0107's Action classification for window resize. Resizing browser chrome is not a
page observation, page mutation, remote side effect, or arbitrary-code execution. It remains a
visible physical action in presentation without consuming a RAWX grant.

### 2. Restore schema-3 grants and layered authority

A strict schema-3 manifest carries name, version, mode, identity, ordered grants, bounded settings,
and a canonical hash. Each grant has a stable id, host allow and deny patterns, an allowed RAWX set,
an optional description, and an optional mode.

Host resolution uses the established rules: exact patterns outrank longer suffix wildcards, which
outrank `*`; an exact tie denies. A grant's deny patterns shrink only that grant. Manifest grant
composition considers grants in declared order and returns the first grant that admits the complete
operation set. When none admits, the first applicable denial supplies attribution. Resource-less
operations use the union of capabilities across grants.

Managed-mandatory, organization, user, and request/session tiers compose by intersection and
deny-overrides. Sacred/protected resources compose by union. The strictest observe/enforce mode
wins. A lower tier and a caller restriction can only subtract authority. `preserve_target_names`
is monotonic false. Protected resources always enforce, including while ordinary policy observes.

Each invocation receives one immutable authority snapshot. Valid source changes atomically replace
future snapshots. An invalid reload keeps the last valid authority and emits a minimized transition
record. A configured tier with no valid initial authority fails closed.

### 3. Make governance understandable and adaptive

Every enforced denial has a deterministic `D-` id derived from the effective manifest hash, grant
id, and rule. Audit carries the complete required set, effective authority identity, deciding
grant, mode, managed publish sequence when present, and denial id without page contents or policy
payloads.

The orchestrator advertises a tool under configured authority only when at least one of its variants
could be admitted by a single grant, or that variant has an empty requirement set. This is a
discovery optimization, never an enforcement boundary. A changed projection increments one narrow
catalog generation and the generic connector emits MCP `notifications/tools/list_changed`.
All-open returns the canonical 22-tool catalog unchanged.

An always-available policy explain operation renders the same directory and current policy
passport. Local CLI commands validate, explain, and simulate without browser work. Simulation is
audit-free and uses the production parser and decision engine.

Repeated enforced denials restore ADR-0079's workspace-local, memory-only attention circuit: three
matching denials in 60 seconds or five enforced denials in 120 seconds. The existing runtime and
workbench presentation carry keep paused, resume, resume quietly, and end session dispositions.

### 4. Remote managed policy is an opt-in product feature

Managed policy activates only through an administrator-provisioned bootstrap. With no bootstrap,
Ghostlight performs no policy network I/O. Ghostlight hosts no policy service and contacts no vendor
endpoint. This is customer-configured governance traffic within ADR-0028's never-phone-home
boundary.

The bootstrap names a local file or HTTPS bundle source and the organization's public verification
key. It may include a bearer token, an organization CA pin, and a polling interval. Local file, USB,
and HTTPS bytes enter one verification path. HTTPS uses conditional requests with ETag,
deterministic process jitter, and capped retry backoff. The default steady-state interval is 900
seconds.

Bundles use the ADR-0055 signed envelope with a monotonic publish sequence, schema-3 manifest, and
optional additive organization presentation. Ed25519 is required. An optional ML-DSA-65 key makes
both signature legs mandatory. Signing uses the `ghostlight/policy` domain. The organization owns
the keys; Ghostlight embeds no policy trust anchor.

Every verified bundle is atomically persisted as last-known-good and is verified again when read
from cache. A lower sequence is rejected visibly. Unreachable, malformed, unsigned, bad-signature,
bad-schema, and rollback responses retain the active verified bundle. A configured cold start with
no valid cache or source fails closed. Signed last-known-good policy has no automatic expiry;
staleness is displayed rather than converted into lost protection.

Policy key generation, signing, public-key inspection, and publishing are customer-facing local
commands and are never license-gated. Publish advances sequence automatically. A content-minimized
status sidecar and workbench passport show organization, verification, sequence, freshness, last
success, last attempt, source class, and contact channels without revealing credentials or rules.

### 5. Keep the restoration narrow

The orchestrator owns all policy semantics and model-facing explanation. The extension remains a
policy-free renderer and browser adapter. The browser connector remains a byte relay. The MCP
connector owns only generic protocol rendering and catalog notification. No generic event bus,
workflow engine, configuration registry, policy daemon, push service, vendor control plane, or
inbound management listener is introduced.

The restoration does not revive Lightbox, the 0.8 demo or demo-brief, licensing code, obsolete
tool names, or old process architecture. Relevant 0.8 scenarios are re-expressed at pure,
service-bridge, process, and visible-browser seams. Documentation and Trust Center claims change
only after their behavior exists and passes.

## Consequences

- Organizations recover host-scoped, identity-bearing, explainable policy rather than a global
  allow/deny switch.
- All-open users keep the exact product catalog and incur no policy fetch, reload, or presentation
  tax.
- Managed customers can use a dumb HTTPS or file host without trusting the transport as policy
  authority.
- Temporary source failure cannot erase verified organizational protection.
- The bridge receives one additive catalog-change message, but no policy types cross into it.
- The audit schema changes from one artificial highest capability to the truthful requirement set.
- The current unsigned flat managed file is intentionally replaced before 1.0; migration is
  documented rather than preserving an insecure compatibility path.
- Windows and Linux both require process and live-browser evidence before release readiness is
  claimed.
