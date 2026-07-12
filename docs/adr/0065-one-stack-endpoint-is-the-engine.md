# ADR-0065: One stack -- the engine is whoever holds the endpoint

- Status: Accepted
- Date: 2026-07-12
- Amends: ADR-0064 (explicit dev isolation -- retires its dev-stack-as-workflow half)
- Builds on: ADR-0045/0062 (relay reconnect), ADR-0063 (deploy-quiesce lock), ADR-0061 (browser identity + slots)

## Context

ADR-0064 replaced ADR-0048's auto-shadow with explicit, fully isolated named stacks: the unpacked
dev extension self-selected its own native host (`org.sylin.ghostlight.dev`), a `dev` install
registered that host plus a `ghostlight-relay-dev` copy, and every MCP client pinned one instance
(`ghostlight` vs `ghostlight-dev` entries). Exercising it end-to-end surfaced the real costs of ANY
two-stack model, isolated or shadowed:

- **Tool-surface duplication.** Two pinned MCP server entries in one editor doubles the advertised
  tool surface (20+ tools each, `mcp__ghostlight__*` + `mcp__ghostlight-dev__*`). A model choosing
  among near-identical duplicated tools is unreliable at best.
- **Browser-side pairing rules.** Which extension, in which browser, against which host, needed a
  convention (dev browser vs prod browser) that is workflow, not architecture -- and a disposable
  dev-browser profile was rejected outright (an unauthenticated profile cannot test the real,
  authenticated scenarios that are Ghostlight's whole point).
- **A parallel installed surface.** A dev instance had to be INSTALLED (host manifests, registry
  keys, a relay copy, client entries) and later cleaned up. Dev state leaked into system state.

The owner's directive, verbatim in spirit: no `-dev` anything; no dev service install; both the
dev extension and the released extension work with whatever engine is locally available (prod or a
fresh build); the internal layers handle it.

The pieces that make this safe already shipped: the agent relay reconnects and replays the MCP
handshake across a service swap (ADR-0045); the browser relay reconnects and replays the
extension's identity frame, keeping Chrome's native port alive (ADR-0062); `deploy.lock` quiesces
relay self-heal while a binary is being replaced (ADR-0063); and multiple browsers/extensions
attaching concurrently each get their own identity-keyed session and tab slot (ADR-0061).

## Decision

**There is ONE stack: one native host, one endpoint, one MCP server entry. The "engine" is
whichever `ghostlight` service currently holds the endpoint. Nothing selects an engine; ownership
of the endpoint IS the selection.**

1. **The extension never picks a host.** `service-worker.js` connects to `org.sylin.ghostlight`,
   period. The `chrome.runtime.id` host selection (ADR-0064 Decision 1) is removed.

2. **The one host manifest allows both known extension builds.** `HostManifest::resolve` always
   emits `allowed_origins` = [store id, unpacked dev id] (+ an optional `--extension-id` extra).
   The instance-aware origin narrowing (ADR-0064 Decision 2's host-per-extension pairing) is
   removed. Either extension build, in any Chromium browser, reaches whatever engine is up.

3. **Dev is not installed.** No dev host, no relay copy, no `ghostlight-dev` client entries, no
   dev service registration. The `DEV_INSTANCE` constant and the install's dev-specific
   supervisor-skip are removed (`--no-supervisor` remains for anyone who wants a terminal-run
   service).

4. **The dev loop is an engine swap, not a parallel stack.** `scripts/dev-loop.ps1` quiesces
   self-heal (`deploy.lock` in every candidate engine directory: the repo target dir and each
   versioned install dir), stops the current engine (never relays -- they are pipes that
   reconnect), rebuilds, and starts the fresh build as THE engine on the one endpoint. Editors and
   browsers ride through via the ADR-0045/0062 reconnects. `dev-loop.ps1 -Restore` hands the
   endpoint back to the newest installed release. When no dev build is running and a relay finds
   the endpoint down, self-heal launches the sibling engine of that relay's own directory -- the
   system reverts to an available engine without ceremony.

5. **Named instances demote to a test-isolation seam.** The `Instance` layer (ADR-0044) survives
   unchanged -- the e2e harness, `lightbox fake-browser`, and CI use ephemeral named instances so
   tests never touch the real endpoint -- but no user- or developer-facing workflow installs or
   pins one. ADR-0064's explicitness principle survives in exactly this form: a client that says
   nothing gets THE stack; only test harnesses say otherwise, explicitly.

6. **Version skew is a normal condition, and the wire contract absorbs it.** With one shared
   channel, an old extension can talk to a new engine and vice versa (during the window between a
   service swap and an extension reload). Wire-protocol changes must therefore stay additive and
   tolerant (unknown fields ignored, absent fields defaulted) -- the same discipline the trained
   tool surface already follows (ADR-0034 D7). A breaking wire change requires a versioned frame,
   not a flag day.

## Consequences

- The LLM-facing tool surface is singular everywhere: `mcp__ghostlight__*`, ~20 tools, no
  duplicated near-identical namespaces to misroute a model.
- A developer's fresh build serves their real, authenticated browser and their real editors --
  which is precisely the point: dev tests the real scenario. The cost is symmetric: while a broken
  build holds the endpoint, real use is broken until `-Restore` (or the next successful loop).
  `ghostlight doctor` says who holds the endpoint.
- The ADR-0062 browser-relay reconnect and ADR-0063 deploy lock stop being dev-instance
  conveniences and become THE mechanism the daily dev loop rides on.
- ADR-0064's shipped code is partially retired (extension host selection, instance-aware
  `allowed_origins`, the dev-install workflow); its structural simplifications (no
  `Selection::Unpinned`, one endpoint per resolution, per-instance hub key) all survive and are
  what make the one-stack swap correct by construction.
- `scripts/dev-browser.ps1` is deleted (disposable profiles rejected; no dev host to point one at).

## Provenance

Owner decisions, 2026-07-12, in discussion after exercising ADR-0064 live:

- Two pinned entries spam the tool surface: "spamming the tool surface (ghostlight +
  ghostlight-dev, 20+ tools each) will make the llm tool usage unreliable at best, catastrophic at
  worst."
- The browser-family split (Chrome=dev / Edge=prod) is "a hack"; the disposable dev browser is
  useless ("if we can't test real-case scenarios in dev, how can we guarantee that it'll work in
  prod?").
- The settlement: "I don't want -dev anything. I don't want to install a dev service. I want both
  the dev extension and the live extension, when it's released, to work with whatever is locally
  available (the prod OR the dev engine). The internal layers should be able to handle it."
- A single master-key env var was considered (and already exists as `GHOSTLIGHT_INSTANCE`,
  ADR-0044); it remains the test-seam selector (Decision 5) but is NOT a dev-workflow switch --
  the workflow needs no switch at all.
