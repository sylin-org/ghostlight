# Ghostlight 1.0 acceptance

Acceptance is the smallest executable proof of the product language, architecture, safety, and
real journey. Each fact is protected once at its narrowest meaningful seam.

## Contract gates

1. Catalog schemas are typo-closed and match every decoder default in `LANGUAGE.md`.
2. The MCP edge retrieves the catalog and generically forwards every tool without per-tool code.
3. The browser connector relays every primitive without model-facing names or product defaults.
4. Incompatible service or browser bridge majors fail before work is accepted.
5. MCP and native-message framing survive split reads, coalesced reads, and disconnects.
6. Adding a product feature composed from advertised browser capabilities changes only the
   orchestrator. The MCP connector, browser connector, adapter protocol, and extension stay byte
   unchanged.
7. The browser connector does not deserialize adapter commands, receipts, events, workspace ids,
   or presentation signals. Unknown bounded adapter payloads round-trip unchanged.
8. MCP revision, service bridge, browser relay, and adapter protocol compatibility are independent.
   A change to one does not bump or reject an unrelated boundary.
9. The MCP and browser relays keep their consumer-facing streams alive across a service restart,
   reauthenticate to the new runtime endpoint, and never replay an uncertain application effect.
10. The browser engine suppresses duplicate operation ids, retains a content-free disposition
    across service-worker suspension, and reports uncertainty rather than repeating an effect when
    its prior dispatch cannot be disproved.
11. A real process test starts both relays without the service, interrupts an in-flight browser
    effect by stopping the service, restarts it, and proves the same MCP stdio and native-message
    processes renegotiate and complete new work without replaying the interrupted effect.

## Executor and truth gates

1. Direct operations and sequence steps pass through the same executor and completion gate.
2. One invocation can commit only one terminal outcome.
3. Deadline or cancellation before dispatch reports no effect and is repeat-safe when the job is.
4. Disconnect or cancellation after uncertain dispatch reports unknown effect and no replay advice.
5. A partial sequence reports completed steps and no replay advice.
6. Stale tab and target handles fail before browser dispatch and suggest obtaining current handles.
7. Lower-capability-model fixtures succeed with the documented shortest calls.
8. A screenshot view handle resolves image coordinates only while its tab, document generation,
   viewport, and zoom still match; stale views fail before pointer dispatch.
9. File paths are validated and bounded before reading, and no file bytes cross the browser bridge
   until governance and credential preflight succeed.

## Governance gates

1. With no active policy, ordinary remote HTTP(S) browser work is permitted.
2. Protected schemes, loopback, and link-local metadata remain denied without policy.
3. Request restrictions never add a host or capability.
4. Started work keeps one authority snapshot even if configuration changes.
5. Invalid configured managed authority denies all work and never falls back to unrestricted.
6. Ownership and leases are checked immediately before effects.
7. Initial, redirected, and script-caused committed landings are governed before text or readiness
   is accepted.
8. A denied landing returns truthful committed-effect and compensation facts.
9. Audit records contain no URL, page text, target name, selector, form value, screenshot, or
   dialog text.
10. Hold, attention, end-session, and cancellation stop later effects at the runtime boundary.
11. Model-driven close dispatches only when the action capability and monotonic tab-close policy
    constraint both permit it. A denying local or managed layer cannot be expanded later.

## Browser job journeys

1. With policy and the local browser setting both permitting close, open `https://example.com`
   with only `url`, read its useful text with only a tab handle, then close that exact tab. The
   result identifies the governed landing and one terminal outcome per call. Opening uses one
   physical primitive and never creates a blank tab before navigation.
2. List tabs, navigate one, inspect controls, find a target, activate it, and recover from the old
   target becoming stale after a document commit.
3. Capture a screenshot only through `browser_take_screenshot`, verify bounded dimensions and one
   MCP image content block, and verify base64 image data is absent from structured facts.
4. Fill multiple ordinary fields, but stop before dispatch and request user handoff when any
   described target is credential-class.
5. Wait for success and timeout branches of each condition family.
6. Run click plus wait directly and as a sequence; observe the same executor behavior.
7. Accept, dismiss, and supply non-secret prompt text to a visible dialog.
8. Disconnect the adapter before dispatch and after dispatch; observe no-effect and unknown-effect
   truth respectively.
9. Activate an existing tab, move backward and forward through history, and reload through the
   same landing-governance path as explicit navigation.
10. Traverse controls inside an open shadow root and perform semantic click, fill, type, hover,
    scroll-to-target, and drag journeys with actionability checked at dispatch time.
11. Capture a screenshot, click and hover through its view handle, then verify document commit,
    zoom, and viewport changes make the old view stale.
12. Scroll in four directions, reveal a target, and set zoom while returning observed viewport
    facts rather than assumed effects.
13. Upload one and several bounded local files to an ordinary file input. Reject credential-class,
    missing, directory, oversized, and changed-after-read inputs before any browser effect.
14. Run an explicitly authorized script, return a bounded serializable value, govern any committed
    landing, and never audit source or result.
15. Open a child tab from a controlled page, adopt it into the same workspace, and preserve
    ownership after moving the tab group to another window.

## Presentation gates

1. Established controlled-scope, cursor, target, click, drag, field, key, scroll, read, find,
   navigation, screenshot, zoom, signature, denial, attention, and caption treatments are visible
   for applicable 1.0 jobs.
2. Presentation frames contain only ids, closed event and activity kinds, phase, detail, and fixed
   Ghostlight-authored labels.
3. Injected presentation failure does not change permission, browser receipts, or terminal result.
4. Feedback clears or reattaches correctly across document lifecycle events.
5. A blocked close shows the fixed outcome-first policy or local-setting receipt on its visible
   controlled tab for five seconds. Reduced-motion presentation remains static and readable.
6. A denial for a background tab never changes focus. One bounded notice per tab survives service
   worker suspension, marks the toolbar, renders when that tab becomes visible, and expires.

## Extension product gates

1. The manifest has the established `Ghostlight in Browser` identity, key and id, byte-identical
   16, 32, 48, and 128 pixel icons, toolbar title, popup, options page, take-the-wheel command and
   description, and only permissions used by tested responsibilities.
2. The same opaque adapter installation id is sent after service-worker suspension, browser
   restart, extension reload, and native reconnect.
3. The adapter hello advertises versioned physical capabilities and a restart-local epoch. Service
   behavior is gated by those capabilities rather than the extension version string.
4. The popup distinguishes disconnected, connected-active, held, attention-required, and ended
   states without reading page content.
5. Popup pause, resume, end-session, and start-new-session actions become authoritative service
   runtime state before the next physical effect. Ordinary resume cannot revive an ended session.
   The keyboard command uses the same intent path.
6. Options preserve the established dark sky-blue UI and persist effects-on, captions-off,
   diagnostics-off, and preserve-tabs-on defaults under local keys. Preserve tabs is a final
   physical interlock: it can refuse model-driven close but cannot edit, import, expand, or
   override authority. Human browser closure remains available.
7. Each exact `Ghostlight - <client label>` title has one canonical blue group across normal
   browser windows. A new tab is created directly in that group's window. With no Ghostlight group,
   the first URL opens in a dedicated normal window; no tab is inserted into the user's active
   window and no blank tab is exposed. Concurrent opens cannot create duplicate groups. Child tabs
   follow their unambiguous opener, and closing or moving tabs does not transfer ownership.
8. The established content-free visual language renders visibly, stays pointer-transparent except
   for attention controls, remains hidden from screenshots, and remounts after navigation. Its
   palette is sky `#38bdf8`, ink `#eaf6ff`, and governance ground `#0c0f14`; its spring curve is
   `cubic-bezier(.22,1,.36,1)`. The 150 ms cursor, four-second scope breath, 620 ms ripple, 700 ms
   field splash, 1450 ms read scan, 1600 ms navigation pill, 1500 ms capture frame, and 1150 ms
   zoom frame are protected product contracts, including their reduced-motion alternatives.
9. The popup and options pages have useful empty, disconnected, error, and incompatible-version
   states and are keyboard accessible.
10. No extension storage key contains URL, title, page text, target name, locator, form value,
   script, file path, file bytes, screenshot, dialog text, or policy.

## Desktop workbench gates

1. Normal `ghostlight` startup starts the orchestrator before the Tauri shell; `--headless` starts
   no desktop runtime. Closing the window hides it, and only explicit quit stops the process.
2. Home presents plural session, operation, and browser counts plus current work and system health
   at a glance. Activity, history, checkup, configuration, and installations remain separate
   focused destinations behind one compact rail.
3. Global search covers destinations, sessions, operations, browser instances, terminal history,
   diagnostics, configuration, and supported harnesses. Search input is bounded at the adapter.
4. Reloading the disposable WebView reconstructs its state from `WorkbenchFacade`; it owns no
   product state, runtime token, authority, or durable history.
5. Blocked and attention-required operations request at most one content-free native notification
   per invocation. Notification failure cannot change governance, audit, or completion truth.
6. Workbench runtime controls use the existing governance facade and publish the resulting state
   through the existing browser port. The desktop adapter cannot dispatch a browser primitive.
7. Installations explicitly check, install, and uninstall Codex, Claude Code, Claude Desktop,
   Cursor, Visual Studio Code, Windsurf, Zed, OpenCode, and Crush registrations. Mutations are
   serialized, idempotent, backed up, preserve unrelated entries, and touch only an entry whose
   command identifies Ghostlight's connector.
8. JSONC and Codex TOML comments, trailing commas, formatting, and unrelated values survive
   install and uninstall. Malformed configuration, unreadable files, and foreign `ghostlight`
   entries are left untouched with an actionable result.
9. The WebView loads bundled assets under a restrictive CSP and has no shell, arbitrary file,
   remote-navigation, or network capability. File mutation terminates in the explicit harness
   application service, outside the UI event loop.
10. The desktop executable, tray, bundle, and workbench use the original Ghostlight icon bytes,
    established palette and spring curve, and a static readable reduced-motion treatment.
11. Recoverable Tauri setup or event-loop failure leaves the orchestrator service alive in
    headless mode. The MCP connector, browser connector, shared bridge, and extension have empty
    diffs for the complete workbench feature.

## Integration and release-readiness gates

1. `cargo fmt --check` passes.
2. Clippy passes with warnings denied for the workspace.
3. Focused unit, contract, and integration tests pass.
4. The full Rust workspace and extension test suites pass in isolated build output.
5. Real MCP stdio, service IPC, browser relay, native messaging, and visible Chromium complete the
   open, read, close journey.
6. The repo-built stack starts, the MCP client sees the exact catalog, and the directly loaded
   unpacked extension reports a compatible adapter connection.
7. Every defect found in visible-browser use has a focused regression proof and is reverified.
8. The unpacked extension renders the correct toolbar icon, opens a usable popup, opens settings,
   toggles hold through the established native host, and keeps its id across reload.
9. The complete documented catalog, extension manifest, popup/options states, and all browser-job
   journeys are audited against `INTENT.md`; there are no undocumented placeholders or accepted
   journeys lacking implementation.
10. The bundled workbench HTML, CSS, and JavaScript render against a representative plural-state
    fixture, every destination is keyboard reachable, and frontend scripts pass syntax checks.
