# Changelog

All notable changes to Ghostlight are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-07-15

Closed-loop browser actions, calmer visible feedback, local-only transport hardening, and
resource-scoped command reliability.

This release establishes Ghostlight's greenfield browser boundary: browser control is local to the
interactive user's authenticated Chromium session. It does not preserve or replace the removed
browser-control web transport.

### Added

- **Closed-loop browser core (ADR-0078).** The additive tool surface now includes semantic
  `act_on`, explicit JavaScript dialog control, and exact owned-tab focus, reload, and close.
  Bounded interaction receipts and ref-linked target resolution reduce read-act-read roundtrips.
- **Resource-scoped scheduling (ADR-0080).** Browser work uses bounded fair queues per tab,
  client topology, and browser. Same-tab commands serialize, different tabs remain parallel,
  and single-surface compound operations retain a reentrant lease with a bounded yield quantum.
- **Extension surface executor.** A bounded per-surface FIFO adds command deduplication,
  acceptance and terminal acknowledgements, payload erasure, and exact unknown-outcome recovery.
- **Document-aware Presentation Broker (ADR-0081).** Page signage now uses explicit content-script
  readiness, exact document/revision acknowledgements, bounded state and event channels, on-demand
  packaged-renderer activation, navigation replay, and browser-session-only active state.
- **Linux user-session discovery (ADR-0082).** A relay launched without desktop environment
  variables securely discovers the owning `/run/user/<uid>` session, restores the user bus, and
  reaches the same local service endpoint as Chromium's native host.

### Removed

- **Browser-control web ingress (ADR-0077).** The `inbound.web` HTTP/WebSocket transport, its
  source policy, configuration keys, remote-enable route, and related scaffolding are removed.
  Browser control is local-only through the same-user OS pipe. The Console remains a separate,
  read-only loopback HTTP listener and rejects WebSocket upgrades.

### Changed

- **Least-privilege release publication.** A read-only assembly job now generates the pinned
  CycloneDX SBOM, packages the extension, creates a canonical `SHA256SUMS` manifest, and uploads
  one release bundle. The privileged publisher only downloads, verifies, attests, and releases
  those existing files.
- **Developer-first entry.** The README and install journey lead with practitioner setup,
  no-account and free-core facts, a read-only first proof, and the pre-release extension path.
- **Calmer visible feedback (ADR-0079).** Isolated denials use a short non-modal sticker. Repeated
  denials pause only the producing MCP session and offer explicit recovery controls. Narration,
  screenshot, and recording cues share a quieter visual language.
- **Persistent controlled-tab disclosure (ADR-0081 amendment).** Every Ghostlight-managed tab now
  keeps a gently breathing sky border across idle time, navigation, detachment, and extension
  worker restarts. Capture hides it only long enough to keep the returned image clean.
- **Atomic execution authority.** Configuration and policy publish as one epoch-bound snapshot;
  URL probes, authorization, dispatch, landing verification, post-processing, and audit use one
  admitted execution context.
- **Complete JavaScript release gates.** The operating-system matrix discovers every extension
  test, parses every extension JavaScript file as a whole, and exercises the npm launcher's host,
  hash, and platform-selection boundaries.

### Fixed

- Native-port and extension-worker reconnects no longer clear an uncertain tab. Recovery requires
  the exact terminal command, confirmed tab destruction, or a changed browser-process generation.
- Extension reload on an unchanged page no longer silently disables narration, denial notices, or
  action feedback. The broker reinjects the packaged renderer into managed tabs and verifies the
  current document accepted the presentation.
- Retained compound intents no longer suppress their own later extension subrequests.
- Tool completions and auxiliary asynchronous replies remain bound to the native connection that
  accepted them, even when a replacement connection reuses the same numeric request id.
- JavaScript dialogs block ref resolution, geometry reads, scroll probes, cursor movement, and
  direct fallbacks before any page-dependent preparation can stall behind the dialog.
- The Foundry demo explicitly negotiates current or legacy provenance through `tools/list`,
  validates the full service-authored boundary, and fails closed on an unnegotiated contract.

## [0.5.7] - 2026-07-13

Reliable memory-only GIF recording, bounded large-payload delivery, agent narration, a broader
installer matrix, and the cohesive Sylin Card Foundry demo.

### Added
- **Agent narration (ADR-0072).** The additive `narrate` tool shows one temporary, responsive
  workflow ribbon in the controlled tab. It is domainless RAWX none, audited through the normal
  chokepoint, usable in scripts and batches, capture-hidden, memory-only, and visually distinct
  from authoritative governance notifications.
- **More first-class MCP clients.** Codex, Windsurf, Zed, OpenCode, and Crush join Claude,
  Cursor, and VS Code. Codex TOML is merged losslessly; strict JSON is updated idempotently;
  commented JSONC is preserved and receives copyable manual configuration.
- **Sylin Card Foundry demo.** `ghostlight demo` now tells one simulated foil-card QA story with
  3D card inspection, defect review, drag disposition, structured form completion, screenshot and
  GIF evidence, and a real off-domain policy denial. The public stage is
  `sylin.org/ghostlight/demo/foundry/`.
- **Bidirectional install handoff.** A successful first install opens the stable browser-extension
  walkthrough once, while dry-run, CI, failed, repeated, and `--no-open` paths stay quiet.

### Changed
- **Recording is ephemeral and lease-bound (ADR-0073).** Capture is session, surface, and
  generation-owned; memory-only; byte-bounded; transactionally started and finalized; refreshed
  by recordable tool activity; and capped by independent 30-second idle and 120-second hard
  deadlines. Export auto-finalizes, and every session/policy/panic/retention failure path erases
  captured bytes.
- **GIF encoding is bounded.** Frames are encoded in two passes and processed one at a time rather
  than retaining all decoded frames at once.
- **One tab group per client (ADR-0066).** Repeated tab creation reuses the session's managed group
  instead of producing a new group for every tab.
- Public documentation and install URLs now consistently use `sylin.org`; Chrome Web Store privacy
  and permission disclosures were tightened against the implemented data flows.

### Fixed
- **Large browser-bound payload delivery (ADR-0074).** Screenshot and GIF uploads use negotiated,
  SHA-256-verified, memory-only chunks, avoiding Chromium's native-messaging size disconnect. Old
  extensions fail fast before an oversized write, and uncertain delivery is reported as
  `outcome_unknown` with `retry_safe: false`.
- Debug observability no longer persists MCP or tool payload bodies that may contain captured page
  data.
- Restored the intended governance badge bleed after the responsive narration pass.

## [0.5.6] - 2026-07-12

Session-policy governance and a scripted demo, root-fixed browser identity and reconnect
resilience, the one-stack dev model, an on-screen governance ribbon, the field-splash effect
with a codified visual language, and the SAPS security-hardening pass. (Includes everything
listed under 0.5.5 below, which was prepared but never published.)

### Changed
- **One stack: the engine is whoever holds the endpoint (ADR-0065).** There is one native host
  (`org.sylin.ghostlight`, allowing both the Web Store and the unpacked dev extension ids), one
  service endpoint, and one `ghostlight` MCP entry -- no dev install, no `-dev` host, no second
  tool namespace. The dev loop becomes an engine swap (`scripts/dev-loop.ps1`: deploy-lock
  quiesce, stop the engine, rebuild, start the fresh build; `-Restore` hands the endpoint back to
  the installed release), ridden through transparently by the relay reconnects (ADR-0045/0062).
  Amends ADR-0064: its dev-stack-as-workflow half (extension identity host selection,
  instance-aware `allowed_origins`, the installed dev instance) is retired; its structural
  simplifications survive. Named instances remain only as a test-isolation seam;
  `scripts/dev-browser.ps1` is removed.
- **The pre-release two-stack development design was retired (ADR-0065).** ADR-0064's explicit dev
  isolation replaced the earlier auto-shadow during development, but the final v0.5.6 release
  removed the dev host, dev install, and duplicate MCP entry in favor of the one-stack engine swap
  above. Named instances remain only as a test-isolation seam.
- **Safe-by-default posture.** The audit flight recorder now defaults ON in every preset,
  all-open included (a session always leaves a trail), and `inbound.web.enabled` now defaults
  OFF in every preset (driving the browser over a local TCP port is opt-in; the pipe path remains
  the default). `ghostlight doctor` shouts `UNGOVERNED (all-open)` and prints an audit-health line.
- **README/registry re-lead:** the front door leads with the durable axis (governance + audit +
  governed composition) and the Continuity Promise; pricing is discoverable-when-sought.
- Softened the security/support SLAs to explicit best-effort targets (solo project).

### Added
- **On-screen denial notifications (SAPS PRES-HIGH-01).** When governance refuses an action, the
  browser shows a persistent top ribbon -- a neutral near-black band with a severity-colored icon
  medallion (error/warn/info/debug) -- explaining the denial in plain language, on the page where
  it happened. It persists until the next genuine page action or an explicit close click;
  read-only actions (screenshot, read, zoom, wait) deliberately never dismiss it.
- **Session policy overlays and the policy composition model (ADR-0060).** A client can declare a
  tighten-only policy at `initialize` (`_meta.ghostlightSessionPolicy`, a schema-3 manifest):
  grants intersect, sacred domains union, the stricter mode wins -- the AWS-session-policy shape,
  escalation-safe by construction. An overlay tightens even an all-open service.
- **`ghostlight demo`:** a scripted, watchable tour of the public demo stage driven through the
  real MCP surface under a session overlay, ending with a governance refusal on screen. Exits
  non-zero on any failure, so it doubles as an end-to-end smoke test.
- **Multi-browser sessions (ADR-0058/0061).** Every attached browser gets its own identity-keyed
  session and tab-id slot, with focus-chain routing for slot-less calls; `ghostlight doctor` lists
  every connected browser. Browser identity is a persistent UUID the extension itself mints and
  stores (stable across relay reconnects AND service-worker restarts), replacing the relay's
  guessed parent pid -- the root fix for tab ids mis-routing under pid 0.
- **Browser-relay restart resilience (ADR-0062).** A service restart or upgrade no longer drops
  Chrome's native-messaging port: the browser relay reconnects and replays the extension's
  identity frame, keeping the same session and slot with no extension reload.
- **Deploy-quiesce lock (ADR-0063):** a `deploy.lock` beside the service executable holds off
  relay self-heal while a binary is being replaced, so a swap never races a relaunch of the old
  image (stale locks self-expire after 30 minutes).
- **Developer instrumentation (ADR-0059):** `lightbox fake-browser` (an offline wire-protocol
  driver -- no Chrome needed, with a deliberately billion-scale canned tab id so encoding bugs
  surface offline), structured native-host attach/reject diagnostics in the debug state ring, and
  the one-command `scripts/dev-loop.ps1`.
- **Field splash effect + a codified visual language.** Every form write (`form_input`,
  `form_fill`, `file_upload`, `upload_image`) now draws a sky-blue splash hugging the field it
  just set -- the click ripple's language applied to the field's own rectangle -- so a watcher
  sees exactly which field the agent touched. The whole effect vocabulary (foundations,
  invariants, the effect dictionary, and the add-an-effect checklist) is now codified in
  `docs/design/visual-language.md` as the normative reference.
- **`ghostlight demo` pacing controls:** `--setup-pause` (default 10s, a hold right after the
  demo tab opens so you can resize/position the window) and `--section-pause` (default 5s
  between the tour's sections), alongside the existing per-step `--pause`.
- **`ghostlight doctor` confirms the extension link without `--debug`** via an in-band
  control-plane `status` request over the ADAPTER/CONTROL endpoint (activates the reserved
  `control` role). Renders a real connected/disconnected verdict.
- **`navigate` auto-creates the Ghostlight tab group** when none exists, instead of failing cold.
- Supply-chain gate: `deny.toml` + a blocking `cargo-deny` CI job; pinned toolchain
  (`rust-toolchain.toml`).
- Bounded audit file (single-generation rotation at 50 MiB).
- `MAINTENANCE.md` (continuity / bus-factor) and `CODE_OF_CONDUCT.md`.

### Fixed
- **Management-plane write path:** the `enable-remote` action requires an `X-Ghostlight-Intent`
  consent header (CSRF hard-stop); the management router enforces a loopback `Host` header; a
  present-but-unparseable `Origin` is no longer treated as the loopback peer.
- **Owner-only pipe DACL fails closed** on Windows instead of falling back to the default DACL.
- **One-liner installers verify integrity** (SHA-256 + best-effort `gh attestation verify`) before
  trusting a downloaded binary.
- **Group-gated the tab-URL probe** so it cannot enumerate the user's non-Ghostlight tabs.
- **`tabs_create` prose now carries the composite tab id**, matching `structuredContent`, instead
  of leaking the raw native id a follow-up call could mis-route on.
- Retired the perpetually-hanging `e2e-smoke` CI job; corrected stale docstrings and the governance
  license path.

## [0.5.5] - 2026-07-11 (prepared but not published; first shipped as part of 0.5.6)

Central policy distribution (ADR-0055): an organization can now host, sign, and roll out a
governance policy from one place instead of hand-copying a manifest file to every machine. Plus a
dev-only integration harness for the whole governance/managed stack (ADR-0056), and the Open Trust
Center -- a public, evidence-linked set of procurement and security-review documents.

### Added
- **managed:// central policy distribution** (ADR-0055): a signed, versioned policy bundle an
  organization publishes once and every device polls. `ghostlight policy sign` produces the bundle
  (the same Ed25519 + ML-DSA-65 composite signature as the license engine); a device bootstraps from
  a local file or a network fetch (`--no-default-features` opts out of the network path entirely),
  keeps a last-known-good cache that is never allowed to fail open, and re-polls live with no
  restart. `ghostlight doctor` and `explain` surface the active policy's provenance -- source,
  freshness, signer -- from a single `ManagedStatus` sidecar; every audited tool call now carries a
  `policy_seq` so the audit trail can prove which policy version was in effect at decision time; a
  denial under a managed policy names the organization's contact line.
- **Lightbox** (ADR-0056): a dev-only integration harness that exercises the real governance/managed
  code path -- no mocks -- against injected time and a local fake-org endpoint, covering passport
  freshness and multi-instance policy propagation. Not part of any shipped release artifact.
- **The Open Trust Center** ([docs/trust/](docs/trust/)): public, evidence-linked procurement
  documentation -- an FAQ, security overview, data-flow diagrams, sub-processor and supply-chain
  disclosures, continuity commitments, a CAIQ-shaped controls questionnaire, the support policy, and
  MSA/DPA drafts on Common Paper standard forms. Every non-obvious claim links the ADR, test, or
  attestation that backs it; nothing here is gated behind a sales call.

## [0.5.4] - 2026-07-10

### Fixed
- Windows binaries no longer require the Microsoft Visual C++ Redistributable. The MSVC build now
  links the C runtime statically, so `ghostlight.exe` and `ghostlight-relay.exe` start on a clean
  Windows machine with nothing extra to install. Previously they failed with "VCRUNTIME140.dll was
  not found" wherever the redistributable was absent -- a latent issue in every Windows build since
  v0.1.0, surfaced by the winget clean-room validation. This is a generic Rust-on-MSVC behavior
  (Rust's own `std` links the VC runtime dynamically by default), not a dependency of any crate, and
  the static-linking fix covers every Windows channel at once (npm, scoop, winget, direct download).
  Linux and macOS are unchanged.

## [0.5.3] - 2026-07-10

The offline licensing engine (ADR-0028), plus how-to guides. Licensing is purely observational --
it never gates behavior, never phones home, and is dormant unless an org-deployed governance policy
is operating -- so this release changes nothing about how Ghostlight runs.

### Added
- Offline license verification: `ghostlight license status` / `install` (a file, an armored
  `-----BEGIN GHOSTLIGHT LICENSE-----` block, or stdin), plus a read-only `License:` section in
  `ghostlight doctor`. A license is verified locally with a composite Ed25519 + ML-DSA-65 (FIPS 204,
  post-quantum) signature; there is no activation server and no network call in the license path.
- Founder-only offline authoring (`license sign` / `pubkey`, gated behind the `license-admin`
  build feature; never in a release build).
- Guides: [docs/guides/licensing.md](docs/guides/licensing.md) (installing/refreshing a key) and
  [docs/business/issuing-licenses.md](docs/business/issuing-licenses.md) (the founder's signing
  runbook).

### Notes
- The audit `license` marker appears only while governance is operationally in effect via an
  org-deployed policy; the all-open audit stream is byte-identical to before.
- Production key generations are embedded by the founder offline; until then a public evaluation
  key allows frictionless self-signed evaluation.

## [0.5.2] - 2026-07-10

npm launcher supply-chain hardening (prompted by a Socket.dev supply-chain finding on the 0.5.1
package). No changes to the ghostlight binaries themselves.

### Security
- The npm launcher now verifies every downloaded binary before it is made executable or run. Each
  release binary's sha256 is pinned in a `checksums.json` that ships inside the npm package (an
  immutable, independently hosted manifest), and the launcher aborts on any mismatch -- a tampered
  release asset, a corrupted transfer, or a hijacked redirect can no longer lead to running an
  unverified binary.
- Downloads and their redirects are constrained to GitHub hosts (`github.com` and
  `*.githubusercontent.com`), so a rogue `Location` header cannot redirect the fetch off-platform.
- The release pipeline (`scripts/release.ps1`) generates the pinned manifest from the published
  `.sha256` assets and refuses to publish a launcher whose manifest is missing or incomplete.

## [0.5.1] - 2026-07-10

Windows first-run fix, found live while validating the Cline marketplace submission (issue #17).

### Fixed
- Windows auto-start actually works without elevation now (ADR-0054). The installer previously
  registered a Task Scheduler logon task, but Windows denies creating logon-trigger tasks from a
  non-admin shell -- so every unelevated install silently registered nothing, and a fresh user's
  first tool call failed until they ran `ghostlight service` by hand. The installer now writes the
  per-user registry Run key (always writable, visible in Task Manager's Startup page) and starts
  the service once, detached; the relay's self-heal likewise spawns the sibling service executable
  detached (null stdio, its own process group) instead of asking a scheduled task that may not
  exist. Old scheduled tasks from elevated installs are cleaned up on install and uninstall.
  macOS (launchd) and Linux (systemd --user) are unchanged.

## [0.5.0] - 2026-07-09

The tool-surface release. Four new tools, harvested from the official Claude-in-Chrome v1.0.80 (now
the sole reference), grow the advertised surface from 17 to 21; the 13 trained tools are re-baselined
against that reference; and the role executables collapse from three to two.

### Added
- `file_upload` tool: upload base64-encoded file bytes to a located file `<input>` on the page via
  an in-page DataTransfer. It never reads the host filesystem; the caller supplies the bytes.
  (ADR-0050)
- `browser_batch` tool: a trained front door that runs a sequence of browser actions in one call over
  the same engine as `script`, returning each step's result with images preserved. (ADR-0050)
- `upload_image` tool: upload a previously captured screenshot (by image id) to a file `<input>`, or
  drag-drop it onto a page element at a coordinate. (ADR-0050)
- `gif_creator` tool: record a browser-automation session and export it as an animated GIF, either
  downloaded or drag-dropped onto a page element. Capture is change-driven (a frame only when the
  page visually changes, with real per-frame timing); frames carry visual overlays (click cues,
  action labels, a progress bar, and a Ghostlight watermark) and use an adaptive NeuQuant color
  palette for faithful screenshots. The encoding pipeline runs in the binary; the extension only
  relays captured frames. (ADR-0050, ADR-0052, ADR-0053)

### Changed
- The extension is thinner: it contains only what must touch a Chrome API (the thin-extension rule,
  ADR-0053); pure computation and durable state -- including the whole GIF pipeline -- live in the
  binary, where fixes ship with a release instead of a web-store review.
- The 13 trained tool schemas are re-baselined against the official Claude-in-Chrome v1.0.80. Only
  three description strings changed for accuracy (`form_input`, `get_page_text`, `read_page`); no
  trained tool name, parameter, or enum value changed. (ADR-0050)
- The two thin pass-through adapters (`ghostlight-adapter-agent`, `ghostlight-adapter-browser`) are
  merged into a single `ghostlight-relay` binary, role-selected at launch (`--role agent`; the
  browser role auto-detected from the Chrome extension origin, since a native-messaging manifest
  gives Chrome a bare path). Releases now ship two executables (`ghostlight` + `ghostlight-relay`)
  instead of three. (ADR-0051)

## [0.4.1] - 2026-07-09

A distribution-plumbing patch: it makes Ghostlight publishable to the official MCP Registry and
fills out the package-manager manifests. No runtime behavior changes.

### Added
- npm `mcpName` marker (`org.sylin/ghostlight`) in the launcher package, the ownership proof the
  MCP Registry requires to link the `org.sylin/ghostlight` server entry to its npm package.

### Changed
- `server.json` targets the current registry schema (`2025-12-11`) and tracks this release.
- The winget, scoop, and homebrew manifests carry the real per-artifact sha256 sums for this
  release instead of placeholders.

## [0.4.0] - 2026-07-09

The multi-instance, resilience, and conformance release. The single binary splits into three role
executables; named instances run fully isolated stacks; a live `dev` build shadows the release
install for unpinned clients; the adapter rides through a service restart, rebuild, or crash with no
client reload; and an MCP protocol-conformance pass brings the handshake up to the current spec.

### Added
- Named instances (ADR-0044): `--instance <name>` runs a fully isolated Ghostlight stack (a `dev`
  alongside the default deploy) with its own identity, directories, host registration, and
  supervisor; `--keep-warm` keeps a service up between actions instead of idle-exiting.
- The development override (ADR-0048): an MCP client or browser registered WITHOUT an explicit
  instance now resolves at connect time, preferring a live `dev` instance and falling back to
  the default -- run a dev service and every unpinned client routes to it; stop it and they
  return to the release install on their next connect.
- One browser surface: the native-host manifest always allows both the Web Store and the pinned
  unpacked-dev extension ids, so `ghostlight install` needs no --extension-id and one
  registration serves a store install and a dev checkout at once.
- `ghostlight doctor` reports whether a live dev instance is currently shadowing the default for
  unpinned clients.
- MCP `protocolVersion` negotiation (ADR-0049): `initialize` echoes the client's requested revision
  when supported and offers the latest (`2025-11-25`) otherwise, instead of a hardcoded
  `2024-11-05`.
- `--no-supervisor` install flag (ADR-0046): skip registering the OS auto-start service (the dev
  loop runs the service in a terminal instead), documented in docs/DEV-LOOP.md.

### Changed
- Three role executables (ADR-0046): the single binary is now `ghostlight` (the CLI + persistent
  service) plus two thin pass-throughs, `ghostlight-adapter-agent` (the MCP-client side) and
  `ghostlight-adapter-browser` (the Chrome native-messaging side). A service rebuild no longer
  relinks the adapters, and `install` places all three side by side.
- Resilient reconnecting adapter (ADR-0045): a service restart, rebuild, upgrade, or crash no
  longer forces an MCP-client reload -- the adapter reconnects (a patient window, up to 120s) and
  replays the captured MCP handshake, so the client rides through transparently.
- Session identity is stable across reconnects: the agent adapter re-presents one guid per
  process, so tab ownership and the session's Chrome tab group survive a service restart
  (ADR-0047 D2).
- New tabs are born directly in the calling session's tab group (no more about:blank bootstrap
  litter), and tabs_context_mcp reports that session's group (ADR-0047 D3).
- Tab groups are titled by the MCP client's name (for example, the ghost glyph followed by
  "Claude Code"), deduped across sessions, instead of a truncated session id (ADR-0047 D4).
- A tab owned by a session that is no longer connected can be adopted by a live session, and
  dead group-map entries are pruned on service-worker restart (ADR-0047 D5).
- `--instance dev install` is now thin (ADR-0048 D6): it registers only the pinned
  `ghostlight-dev` MCP-client entries; browser traffic rides the unified default host.
- The extension always connects to the `org.sylin.ghostlight` host; the installType-based
  dev-host selection is superseded by adapter-side resolution (ADR-0048 D5).
- MCP conformance (ADR-0049): `initialize` advertises `tools.listChanged` (the server does emit it
  on manifest hot-reload); a malformed JSON-RPC frame gets an addressable `-32700` instead of a
  silent drop; and a JSON-RPC batch (array frame, removed from MCP in 2025-06-18) is rejected with
  a message pointing at the `script` tool.

### Fixed
- Tab tools no longer refuse tabs that sit in a per-session Ghostlight group: the extension's
  gate now recognizes every Ghostlight-managed group (ADR-0047 D1; the e2e F4 desync).
- A service-side read error in the agent adapter reconnects instead of exiting, so an abrupt
  service death never forces an MCP-client reload (ADR-0047 D6).
- The anti-squat hub-key is now per-user, not per-instance, so an unpinned adapter (default
  identity) can verify a live `dev` service's proof -- the development override no longer fails the
  "not the one this user installed" refusal (ADR-0048 amendment).

## [0.3.0] - 2026-07-07

The composition batch (ADR-0035 through ADR-0038): sequential multi-step scripting, semantic form
filling, page-state awareness, and structured results -- the tools that collapse multi-round-trip
browser workflows into a single call. Plus the distribution push: one-line installers, the npm
launcher, the landing/install pages, and the extension's first-run walkthrough tab.

### Distribution (2026-07-07 session)

- One-line installers `scripts/get.sh` / `scripts/get.ps1` (download latest release, run the
  idempotent `ghostlight install`). Release assets now include RAW per-target binaries
  (version-less names) so `releases/latest/download/...` works with no API parsing.
- `ghostlight` npm launcher (`npx -y ghostlight`): fetches the version-matched binary on first
  run; stderr-only chatter so MCP stdio stays clean.
- cargo-binstall metadata; winget/scoop/Homebrew manifest templates under `packaging/`.
- Landing + install pages under `site/` (GitHub Pages); the extension opens the install
  walkthrough on first install (reason "install" only, no state, no tracking).
- README quick-install block with Cursor / VS Code one-click deeplinks and the npx snippet;
  `server.json` for the official MCP registry.

### Fixed (distribution session)

- Cross-platform test-profile compile errors (`proc.rs` cfg-gated `Stdio` import,
  `supervisor.rs` cfg-gated test helper) that failed the macOS/Linux CI gate.
- Console index truncation on hosted Windows runners: the management web server now performs a
  bounded lingering close (drain to client EOF) after `flush` + `shutdown`.
- The quarantined `e2e-smoke` CI job is capped at 15 minutes (it previously hung to the
  6-hour runner ceiling on every push).
- CI now runs `tests/extension/grouping.test.js` (it existed but was not in the test line).

### Added

- **`script` tool** ([ADR-0035](docs/adr/0035-script-tool.md)): run up to 20 tool calls sequentially
  in one request. Steps execute in order through the same governance chokepoint every individual call
  enters; each step is independently authorized, audited, and post-processed. Step arguments may
  reference a prior step's structured result (`$prev.field`, `$N.field`); a `dry_run: true` flag
  returns per-step governance verdicts (`would_allow` / `would_deny`) without dispatching, so the
  model sees the pre-flight map before committing. A `budget_ms` argument bounds the whole call.
- **`form_fill` tool** ([ADR-0036](docs/adr/0036-form-fill-tool.md)): fill a form by field labels in
  one call. Matches keys against label, placeholder, name, and aria-label with specificity-ordered
  tiering; ambiguous keys are returned unmatched with candidates instead of guessed. Optional
  `submit: true` clicks the form's own submit control after filling.
- **`wait_for` tool** ([ADR-0037](docs/adr/0037-page-state-awareness.md)): wait until a page condition
  holds and the page has settled. An adaptive settle detector (mutation-rate decay, floored at 3)
  gates on the page's own pace; returns elapsed_ms, settle diagnostics, and the matched element's
  ref for direct chaining.
- **Structured results** ([ADR-0038](docs/adr/0038-structured-results.md)): tools with a declared
  result vocabulary (`find`, `tabs_context_mcp`, `tabs_create_mcp`, `navigate`, `wait_for`,
  `script`, `form_fill`) carry a `structuredContent` field alongside text and advertise an
  `outputSchema`. This is the substrate `script`'s references resolve against.
- **Consequence digests** ([ADR-0037](docs/adr/0037-page-state-awareness.md) Decision 2): every mutating
  action's confirmation gains an `observation:` block reporting what changed (URL, title, DOM
  mutations, focus movement, alerts, dialogs).
- **`read_page` diff mode** ([ADR-0037](docs/adr/0037-page-state-awareness.md) Decision 3): the optional
  `diff: true` argument returns only changes since the previous read on that tab. Stale-ref errors
  now name the re-render and the fix.
- **`engine.script.budget_ms` config key**: total wall-clock budget for one `script` call (default
  120000ms, range 1000..480000).

### Changed

- `dry_run` is a pipeline-level parameter on `run_tool_call`, not a script-internal evaluator: it
  runs the real governance decision (registry, schema, sacred, authorize) and returns the verdict at
  the dispatch boundary without dispatching. It is honored by every tool at the pipeline layer but
  advertised only on `script`'s inputSchema (the 13 trained schemas are byte-pinned).
- [ADR-0035](docs/adr/0035-script-tool.md) Decision 9 (an `idempotency_key` cache on `script` /
  `form_fill`) was not taken in v1; it is superseded by [ADR-0040](docs/adr/0040-pipeline-idempotency-gate.md)
  (Proposed), which relocates retry-safety to a pipeline-level gate covering every tool call.

## [0.2.0] -- 2026-07-05

The Ghostlight Hub release. The single-session model is replaced by a persistent
background service that owns the one browser link and multiplexes every client through a
single governance chokepoint, plus a local Console for seeing what the service is doing.

### Added

- **The Ghostlight Hub (ADR-0030).** A persistent, standalone `ghostlight service` now
  owns the browser link and the client endpoint for its whole life. Every MCP client runs
  as a thin adapter that connects to it, so any number of clients (Claude Code, Cursor,
  and others) can be connected at once, each multiplexed as its own session through the
  single governance chokepoint. This repeals the previous one-session-at-a-time limit
  (ADR-0004). The service is kept warm by a per-user OS supervisor (Windows Task
  Scheduler, macOS launchd, Linux systemd --user), self-heal-started on first use if it
  is down, and shuts down only after an idle-grace window with no live sessions and no
  browser link.
- **The Console (ADR-0030 Decision 9).** A local, loopback-pinned web page served by the
  service at its web-API address. It shows live sessions (with truncated session ids), a
  provenance-aware view of the layered configuration (value, source layer, and lock state
  per key), and a single "enable remote connections" control. It is never a manifest
  editor and never a remote control plane.
- **Local web API.** An opt-in TCP JSON-RPC endpoint that acts as a second session source
  alongside the stdio adapters, gated by the new `channels.webapi.from` policy key
  (loopback-only by default).
- **Per-session browser tab groups.** Each session's tabs are grouped in the browser so
  concurrent sessions stay visually distinct.
- **Cross-session isolation and admission control.** Binary-authoritative tab ownership so
  one session cannot drive another's tabs; adapter-minted session ids bound to the
  connecting client's OS credential; per-client session and mint quotas (never a single
  global cap); and an anti-squat proof on the client endpoint.
- **Reconnect grace and an honest bounded queue (ADR-0030 Decision 3).** A bounded
  reconnect window over transient extension drops, per-client rate limiting, and
  oversize-reply chunking so one session's large payload cannot head-of-line-block
  another's small one.
- **Extension polish.** Official mascot icons, a per-action visual-feedback vocabulary
  (click ripples, drag trail, type shimmer), and an options page plus popup toggle for
  those preferences and action captions.
- **Installer auto-start.** `ghostlight install` now registers and starts the OS
  supervisor so the service is always ready.

### Changed

- Renamed the browser extension to "Ghostlight in Browser" and recorded its Chrome Web
  Store listing.
- Reorganized the internals into a `src/hub` composition root (HubCore / ServiceContext)
  with transport-generic session serving, so the same governance path serves both the
  stdio adapters and the web API.

### Fixed

- **Lifecycle hardening (ADR-0029).** Cross-platform process-liveness primitives, a
  parent-death watchdog so an orphaned session self-terminates when its editor exits, a
  liveness-aware `doctor` with a `--fix` reaper and a startup orphan sweep, and a single
  shutdown coordinator. Idempotent extension library modules so a re-injected content
  script cannot double-register.

## [0.1.0] -- 2026-07-04

First tagged release: the unconstrained browser-automation engine (all-open) with the
governance overlay available as an opt-in capability manifest. Shipped as four platform
binaries (Windows x86_64, macOS Intel and Apple Silicon, Linux x86_64) plus the extension
zip, with SHA-256 checksums and signed build-provenance attestations.

[0.2.0]: https://github.com/sylin-org/ghostlight/releases/tag/v0.2.0
[0.1.0]: https://github.com/sylin-org/ghostlight/releases/tag/v0.1.0
