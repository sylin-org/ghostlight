# Ghostlight 1.0 intent

## Promise

Ghostlight lets an MCP client state a browser job simply. Ghostlight performs the internal
browser steps, applies one authority snapshot, and returns one truthful terminal result.

The browser is the user's visible, existing, authenticated Chromium browser. Ghostlight does
not create a hidden browsing world or ask a model to reproduce browser orchestration.

## User jobs

Ghostlight supports these distinct jobs:

1. See the tabs Ghostlight controls.
2. Bring an exact controlled tab and its window into view.
3. Open a URL in a selected or newly controlled tab.
4. Navigate an exact controlled tab to a URL, through history, or by reloading it.
5. Close an exact controlled tab.
6. Read useful bounded text from a page or target.
7. Inspect page structure and actionable controls, including open shadow trees.
8. Find semantic targets and receive short-lived opaque handles.
9. Capture a viewport, page, or target screenshot and receive a short-lived view handle.
10. Activate a semantic target or a point in a current captured view.
11. Scroll the page or reveal a semantic target.
12. Set the visible tab zoom.
13. Hover a semantic target or a point in a current captured view.
14. Fill a group of ordinary form controls without entering credentials.
15. Type ordinary text through browser input events without entering credentials.
16. Send an explicit keyboard action.
17. Drag one semantic target to another.
18. Upload explicitly named, bounded local files to an ordinary file control.
19. Run an explicit bounded page script and return its serializable result.
20. Wait for an explicit observable condition.
21. Run a short, fully specified sequence on one controlled tab.
22. Resolve a visible browser dialog.

These jobs are separate when their intent, required inputs, safety, or result facts differ.
Internal browser commands are combined when a person experiences them as one job.

## Delight

- The shortest common call contains only the user's intent.
- Ghostlight safely chooses a workspace and tab when there is one obvious choice.
- Navigation waits briefly for useful readiness and reports the committed landing.
- Results say what happened, what is ready, and whether replay is safe.
- Failures include at most two contextual recovery actions written by Ghostlight.
- Stale tab and target errors explain how to obtain current handles.
- Lower-capability models can succeed without knowing Chromium, CDP, transport, policy,
  settlement, or process details.
- The toolbar always explains whether Ghostlight is connected, active, paused, waiting for the
  user, or ended.
- Ghostlight-created tabs remain visibly grouped by workspace. A child tab opened by a controlled
  page joins the same workspace when ownership is unambiguous.
- Visual feedback follows the active document without leaking page content or becoming required
  for successful work.
- The tray opens a calm workbench that answers what is running, what happened, whether Ghostlight
  is healthy, which controls apply, and where it is connected, without turning setup into a CLI
  archaeology exercise.
- Once its connector registration and browser extension are installed, Ghostlight needs no
  separate startup ritual. Launching an MCP client or Chromium demand-starts the local service
  when it is absent.
- Launching Ghostlight directly shows the workbench or focuses the workbench owned by the running
  authority. Adapter demand-start keeps the workbench hidden but leaves the tray available.
- Three destinations and one global search reach every workbench surface and user-visible record.
  The monitor carries the current action in full and the recent ones beneath it, so a glance is
  enough.

## Desktop workbench experience

The workbench is the orchestrator's local-human presentation surface. It is part of the same
installed product and process, while remaining a separate presentation and application context.
Headless operation is an explicit service-only mode, not the ordinary installed experience.

- Monitor is where the work becomes visible. The current action stands in full with its elapsed
  time running. When the next one starts, that action settles and drops into a newest-first queue
  beneath it, so a person can watch a session unfold and scroll back through what already happened.
  While nothing is running, the last completed action stays on screen rather than an empty panel.
  Plural MCP sessions and browser instances appear beside it.
- The record beneath the current action is bounded, local, newest first, and payload-free.
- MCP integrations checks, connects, and disconnects Ghostlight's owned registration for explicitly
  supported MCP clients. It never overwrites a foreign entry or exposes a generic file editor or
  command runner.
- Status explains service, browser, and authority state, tests native notifications, and applies
  the explicit end-session intent through the same governance owner used by browser and MCP flows.
- Pause, resume, and start-session apply from the persistent header beside the connection state
  and match the tray, so taking the wheel has one affordance rather than several.
- Important blocked and attention-required facts may produce quiet, deduplicated OS notifications.
  Routine successful work does not.
- The orchestrator publishes sequenced changes, so the surface shows work as it happens instead of
  sampling for it. A surface that misses a change resynchronizes from a snapshot rather than
  guessing.
- Closing the window returns it to the tray without ending browser service. A failed or reloaded
  WebView is disposable and reconstructs from the orchestrator.
- The original Ghostlight artwork, motion character, and reduced-motion behavior remain product
  identity across extension and desktop surfaces. The desktop carries Ghostlight's published teal
  accent and night-garden ground, while the in-page renderer keeps the sky signal users are already
  trained on. The amendment to ADR-0102 records why the two differ.

## Extension experience

The unpacked extension is a complete local product surface, not an invisible relay fixture.

- Its manifest preserves the established `Ghostlight in Browser` name, description, icon set,
  toolbar title, options page, and take-the-wheel keyboard command. Its permissions remain limited
  to mechanisms used by 1.0.
- Its installation identity persists across service-worker suspension, browser restart, and
  extension reload. The pinned development key preserves the established unpacked extension id.
- The toolbar popup preserves the established take-the-wheel control, separate panic control,
  human-attention choices, connection indicator, local-only note, caption toggle, and wording.
- The options page preserves the established dark sky-blue presentation, effects-on and
  captions-off defaults, connection status, diagnostic toggle, governance explanation, and a
  default-on local choice to preserve controlled tabs. It owns only adapter-local presentation,
  diagnostics, and physical safety preferences. Authority and policy never move into the
  extension.
- Controlled tabs use one browser-wide blue exact-title group named
  `Ghostlight - <client label>`. New work reuses that group wherever the user placed it. When no
  Ghostlight group exists, the adapter creates a dedicated normal browser window instead of
  inserting work into the user's active window. Moves between windows preserve ownership, and
  unambiguous child tabs are adopted by the parent workspace.
- Presentation preserves Ghostlight's luminous sky-blue visual language: controlled-tab border,
  phantom cursor, target and field treatments, click ripple, drag trail, scroll cue, read scan,
  action signatures, screenshot frame, denial, attention, and optional captions. It survives
  document replacement and clears terminal state. The contract includes the established
  `#38bdf8` sky accent, `#eaf6ff` ink, `#0c0f14` governance chrome, layered omnidirectional glow,
  spring entrance curve, ease-out exits, and reduced-motion variants. Timing is identity too:
  the cursor glides for 150 ms, scope breathes over four seconds, a click ripple lasts 620 ms,
  the read scan lasts 1450 ms, the navigation pill lasts 1600 ms, and a guardrail receipt remains
  readable for five seconds. Reduced motion removes the receipt animation without hiding its text.
- A blocked close is described as a protective outcome, not a generic failure. Policy and the
  browser-local preserve-tabs setting receive distinct fixed Ghostlight-authored copy. The relevant
  tab remains visible evidence, manual closure remains available, and background notices wait for
  that tab to become visible rather than stealing focus.
- Established Ghostlight artwork is preserved byte-for-byte in the repository. Clean-room rules
  apply to implementation, not product identity.

## Safety and truth

- No active policy permits browser capability, subject to the protected-host deny ceiling.
- Started work uses one immutable authority snapshot. Request restrictions only tighten it.
- Ownership, leases, runtime controls, and landing authority are checked at their final boundary.
- Model-driven tab closure occurs only when the immutable service authority and the browser's
  local preserve-tabs choice both permit it. Neither gate can expand the other, and local human
  tab closure remains available.
- Every committed landing is governed before its content or readiness is accepted.
- Managed authority that is present but invalid fails closed.
- Credential-class fields require visible user handoff. Ghostlight never types secrets.
- Local file upload reads only paths explicitly supplied for that invocation, enforces count and
  byte bounds before browser dispatch, and never records paths, names, or contents in audit.
- Script execution is explicit `execute` authority. Its source and result are bounded and never
  enter audit or presentation.
- Screenshot coordinates are accepted only with a current opaque view handle. A document commit,
  changed viewport, changed zoom, or superseding screenshot makes unsafe coordinate state stale.
- Cancellation, disconnects, holds, attention, partial effects, and uncertain effects are
  terminal truths, not hidden retries or fabricated success.
- Unknown, partial, committed, or otherwise unsafe effects never recommend replay.
- Audit records contain identifiers and decisions, never URLs, page content, field values,
  screenshots, selectors, or dialog text.
- Presentation is content-free. Presentation failure cannot grant authority or change a result.
- Ghostlight never phones home.

## Exclusions

- No hidden or headless browser.
- No credential entry, secret storage, content inspection, or DLP.
- No Firefox support.
- No vendor-, model-, or client-specific language.
- No remote multi-tenant service.
- No generic workflow engine, actor framework, event bus, CQRS split, or event store.
- No telemetry, activation service, update check, or network call unrelated to the requested
  visible-browser work.

## Product terms

- **Invocation:** one model-requested unit of work with one terminal outcome.
- **Workspace:** one admitted MCP session and its controlled Chromium tabs.
- **Tab handle:** an opaque stable handle for a controlled tab.
- **Target handle:** an opaque handle tied to one tab document generation.
- **View handle:** an opaque handle tied to one tab document generation and captured viewport
  transform.
- **Authority snapshot:** the immutable effective permission for one invocation.
- **Landing:** the final committed document observed after navigation.
- **Hold:** a state that prevents further effects until runtime authority permits them.
