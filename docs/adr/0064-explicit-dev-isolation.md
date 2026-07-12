# ADR-0064: Explicit dev isolation (retire the auto-shadow dev override)

- Status: Accepted; AMENDED by ADR-0065 (one stack). The dev-stack-as-WORKFLOW half of this ADR is
  retired: the extension's identity-based host selection (Decision 1), the host-per-extension
  origin pairing (Decision 2), and the installed dev instance are gone -- there is one host, one
  endpoint, and the dev loop swaps which engine holds it. The STRUCTURAL half survives unchanged
  and is what makes the swap correct: no `Selection::Unpinned`, one endpoint per resolution
  (Decision 3), the per-instance hub key (Decision 4), and the single-target reconnect (Decision 5).
- Date: 2026-07-12
- Supersedes: ADR-0048 (development override -- the auto-shadow half)
- Builds on: ADR-0044 (named instances), ADR-0046/0051 (per-instance relay copy)
- Simplifies: ADR-0062 (browser-relay reconnect), ADR-0063 (deploy-quiesce lock)

## Context

ADR-0048 let a developer run a `dev` build alongside the installed `default` one WITHOUT any
reconfiguration: an UNPINNED client (no `--instance`) resolves its target at connect time, preferring
a live `dev` instance and falling back to `default`. The `dev` install is deliberately THIN -- it
registers NO native-messaging host of its own and reuses the single production host
`org.sylin.ghostlight`; dev-vs-default is decided client-side by trying `[dev, default]` in order.

The convenience -- your main browser / MCP client transparently hitting a dev build -- is exactly what
makes the system hard to reason about. Everything convoluted traces to that one auto-shadow:

- connect-time instance resolution and the `[dev, default]` candidate ordering (`instance.rs`,
  `ipc.rs`), with a "first present, else last" pick that grabs whichever instance happens to be up;
- a reconnect (ADR-0062) that, on a dev-service restart, re-picks and lands on the live `default`
  instead of snapping back to `dev` -- the exact failure a live test hit and could not cleanly show;
- a deploy-quiesce lock (ADR-0063) scoped to the exe DIRECTORY specifically to cover an unpinned
  adapter whose identity is `default` but whose binary is the dev build;
- a hub-key deliberately made instance-INDEPENDENT (`antisquat.rs`, `observability::shared_data_dir`)
  ONLY so a `default`-identity adapter can prove itself to a `dev` service;
- a doctor "Development override" section, and the `dev_thin` install short-circuit.

None of this is the persistent service, the relay bridge, or identity (ADR-0061) -- all of which are
sound. It is the auto-shadow, and no comparable tool does it. The universal pattern for local dev is
EXPLICIT: a browser extension dev-loop uses a separate unpacked extension with its OWN native-host
name pointing at the dev binary; language servers and local daemons take an explicit path/flag. One
target per client, chosen deliberately.

Crucially, the isolation machinery ALREADY exists: `Instance::from_name("dev")` derives a fully
separate stack (`org.sylin.ghostlight.dev` host, `…dev.v1` endpoint, `ghostlight-dev` dirs,
`ghostlight-relay-dev` copy). The auto-shadow is a thin layer bolted ON TOP of it. This ADR deletes
the layer and uses the isolation that is already there.

## Decision

**Retire the auto-shadow. Isolate dev explicitly, like every other local dev-loop.**

1. **The unpacked dev extension gets its own native-messaging host.** The shared `service-worker.js`
   selects its host by its own identity: `chrome.runtime.id === DEV_EXTENSION_ID` ->
   `org.sylin.ghostlight.dev`, else `org.sylin.ghostlight`. Same codebase; the unpacked build
   (pinned dev id `cjcm…` via the committed manifest key, ADR-0016) self-selects the dev host, the
   Web Store build (`lejc…`) selects production.

2. **A `dev` install registers its own host**, exactly as any non-default named instance already
   does: an `org.sylin.ghostlight.dev` manifest whose `path` is a `ghostlight-relay-dev` copy (which
   pins `instance=dev` from its own argv[0], ADR-0044/0046), and whose `allowed_origins` is the dev
   extension id. The `dev_thin` host short-circuit is removed. (`dev` may still skip the auto-start
   supervisor -- a developer runs the dev service from a terminal.)

3. **Every client is pinned to exactly one instance; `Unpinned` is deleted.** No `--instance` /
   argv[0] / env signal means the DEFAULT instance (the reserved-word `default` semantics already
   give this). The bare `ghostlight-relay` pins default; `ghostlight-relay-dev` pins dev; an MCP
   adapter pins whatever its client config names. `Selection::Unpinned`, `Selection::candidates()`,
   the `[dev, default]` ordering, and `pick_native_host_endpoint` all go away. A client connects to
   its ONE endpoint (with the existing patient retry).

4. **The machinery the shadow forced collapses.** The hub-key becomes per-instance (anti-squat is
   cross-USER, not cross-instance -- each instance owns its key). The ADR-0062 reconnect simplifies
   to "wait for my one service to come back" (this is what makes a dev-service-restart reconnect
   correct BY CONSTRUCTION -- no re-pick, no wrong-instance). The ADR-0063 deploy-lock re-scopes to
   the instance data dir. Doctor's "Development override" section is removed. The production host's
   `allowed_origins` narrows to the store id (dev has its own host).

Production and dev are now two fully separate stacks that never compete. `dev-browser.ps1` already
launches a disposable Chrome with the unpacked extension; it now reaches dev through the explicit
host, not the shadow.

## Consequences

- The reconnect and deploy stories become correct by construction: one target per client, so a
  service restart is just "wait for it," and a deploy touches one instance's files with no
  cross-instance race. ADR-0062 and ADR-0063 stay, smaller and load-bearing only for real upgrades.
- **Behavior change (disclosed):** a plain `default` install no longer lets an MCP client (or your
  main browser) transparently hit a running dev service. To drive dev, you configure that client for
  dev explicitly (`--instance dev`, or `ghostlight --instance dev …`, or the dev extension in a dev
  browser). Explicit over magic -- the whole point.
- Net code REMOVAL: the `Selection` shadow layer, the candidate list, the pick, the instance-
  independent hub-key special case, and the doctor section all go. The `Instance` derivation and the
  Hub multiplexing (the parts with real value) are untouched.
- The ADR-0044 Decision 2 guard (`default_instance_is_byte_identical`) stays green throughout: only
  `dev`'s registration and the shadow change; the default identity is never touched. Existing prod
  installs keep working (they already allow the dev origin; a re-install narrows it).

## Provenance (decided; do not re-litigate)

- The auto-shadow's convenience was real but is the single source of the "which instance?" ambiguity;
  it is retired in favor of the industry-standard explicit dev isolation. ADR-0048's NAMED-instance
  identity (ADR-0044) and the per-user validation survive; only its `Unpinned`/prefer-live-dev
  half is superseded.
- The extension selects its host by `chrome.runtime.id` rather than shipping two builds: one
  codebase, identity-driven, and the dev id is already a fixed known constant.
- `dev` keeps no auto-start supervisor (developer-run); it is not a gap.

## Execution (five landable phases; the shadow keeps working until Phase 3)

1. **Dev host registration (additive).** Remove the `dev_thin` host short-circuit
   (`install/mod.rs`); a dev install writes its `org.sylin.ghostlight.dev` manifest + `ghostlight-
   relay-dev` copy. Shadow still intact -- nothing breaks.
2. **Extension host-by-identity.** `service-worker.js` picks the host from `chrome.runtime.id`. The
   dev extension now uses the explicit dev host.
3. **Delete the shadow.** Remove `Selection::Unpinned`/`candidates()`/`DEV_INSTANCE`,
   `pick_native_host_endpoint`, the `[dev,default]` ordering; relay resolvers return a pinned
   `Instance`; `endpoint_candidates` -> one endpoint; single-endpoint reconnect. Update the tests.
4. **Collapse machinery.** Per-instance hub-key; simplify the ADR-0062 reconnect and ADR-0063 lock;
   drop doctor's dev-override section; production `allowed_origins` -> store id only.
5. **Docs/tooling/memory.** dev-loop note, CHANGELOG, mark ADR-0048 Superseded.
