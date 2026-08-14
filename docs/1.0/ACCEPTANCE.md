# Ghostlight 1.0 acceptance

Acceptance is the smallest executable proof of the product language, architecture, safety, and
real journey. Each fact is protected once at its narrowest meaningful seam.

The supported operating-system matrix is Windows and Linux. Every cross-platform source,
process, package, launcher, install, upgrade, and uninstall gate runs against that matrix.

## Contract gates

1. The catalog contains exactly the 22 tools in `LANGUAGE.md`, in documented order, with no
   simultaneously advertised legacy dialect or client-selected profile. Page JavaScript is
   `browser_execute`; the unreleased `browser_evaluate` name is neither advertised nor decoded.
2. Catalog schemas are typo-closed at every object level and match every decoder requirement,
   conditional branch, bound, and default in `LANGUAGE.md`. Every declaration has field guidance,
   a shortest valid example, a truthful output schema, and standard MCP annotations.
3. The MCP edge retrieves the catalog and generically forwards every tool without per-tool code.
4. The browser connector relays every primitive without model-facing names or product defaults.
5. Incompatible service or browser bridge majors fail before work is accepted.
6. MCP and native-message framing survive split reads, coalesced reads, and disconnects.
7. Adding a product feature composed from advertised browser capabilities changes only the
   orchestrator. The MCP connector, browser connector, adapter protocol, and extension stay byte
   unchanged.
8. The browser connector does not deserialize adapter commands, receipts, events, workspace ids,
   or presentation signals. Unknown bounded adapter payloads round-trip unchanged.
9. MCP revision, service bridge, browser relay, and adapter protocol compatibility are independent.
   A change to one does not bump or reject an unrelated boundary.
10. The MCP and browser relays keep their consumer-facing streams alive across a service restart,
   reauthenticate to the new runtime endpoint, and never replay an uncertain application effect.
11. The browser engine suppresses duplicate operation ids, retains a content-free disposition
     across service-worker suspension, and reports uncertainty rather than repeating an effect when
     its prior dispatch cannot be disproved.
12. A real process test starts both relays without the service, interrupts an in-flight browser
     effect by stopping the service, restarts it, and proves the same MCP stdio and native-message
     processes renegotiate and complete new work without replaying the interrupted effect.
13. After either connector fails to find the service, it uses the one shared lifecycle seam to
     start only the exact sibling `ghostlight` executable with no application arguments and
     continues its existing reconnect loop.
14. A fresh deployment lock suppresses demand-start. A stale deployment lock cannot suppress it
     indefinitely.
15. Concurrent demand-start requests converge on one lifetime-leased service authority before
     runtime publication or desktop initialization.
16. A direct/default launch reveals the existing authenticated authority's workbench without a
    new listener or workspace. A headless authority refuses that presentation request clearly.
17. An adapter that advertises end-to-end liveness acknowledges content-free heartbeats through the
    unchanged opaque browser connector. A relay socket that stays attached without acknowledgements
    becomes unavailable, while a silent browser operation stays available when its independent
    heartbeat is acknowledged. An unanswered post-dispatch probe leaves that call unknown and makes
    the next call stop before dispatch.
18. Two browser identities stay connected at once and each answers its own requests. A second
    connection carrying an identity that is already registered replaces it and closes the replaced
    stream, so the retired connection reaches end-of-stream instead of silently discarding work.
19. A workspace binds to one browser for its life. Physical tab ids resolve only within their
    browser, and a bound browser that disconnects is waited for rather than replaced by another.
20. A crossing with no binding uses an explicit browser selection, then reported attention, then the
    sole connected browser. Two connected browsers with no evidence produce a refusal naming both
    candidates, with no dispatch and no binding. Listing a workspace's tabs succeeds with no browser
    connected.

## Executor and truth gates

1. Direct operations and sequence steps pass through the same executor and completion gate.
2. One invocation can commit only one terminal outcome.
3. Deadline or cancellation before dispatch reports no effect and is repeat-safe when the job is.
4. Disconnect or cancellation after uncertain dispatch reports unknown effect and no replay advice.
5. A partial sequence reports completed steps and no replay advice.
6. Stale tab and target handles fail before browser dispatch and suggest obtaining current handles.
7. Lower-capability-model fixtures succeed with every documented shortest call, choose the correct
   sibling among related tools, and recover from deliberately stale and ambiguous handles.
8. A screenshot view handle resolves image coordinates only while its tab, document generation,
   viewport, and zoom still match; stale views fail before pointer dispatch.
9. File paths are validated and bounded before reading, and no file bytes cross the browser bridge
   until governance and credential preflight succeed.
10. The extension alone owns recording identity, frame acceptance, deadlines, memory bounds, stop,
    retention, and erase. The orchestrator checks source authority before start and disclosure.
    Status, stop, and discard remain available without new browser authority.
11. Diagnostic reads are cursor-based and non-destructive. Enabling, filtering, sanitization, host
    authority, bounds, expiry, and eviction are enforced before model-visible results.

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
9. Audit records contain no URL, arbitrary page text, selector, target handle, form value,
   screenshot, recording frame or GIF, dialog text, console text, request URL, header, body, or
   diagnostic entry. A successful action may retain one governed, normalized, bounded target name
   in its Ghostlight-authored summary.
10. Hold, attention, end-session, and cancellation stop later effects at the runtime boundary.
11. Model-driven close dispatches only when the action capability and monotonic tab-close policy
    constraint both permit it. A denying local or managed layer cannot be expanded later.
12. A refused explicit navigation records only the normalized attempted host. Its path, query,
    fragment, request value, target description, and page text remain absent from summary,
    observation, presentation, and audit.
13. A page-authored role is narrowed to the closed Ghostlight role vocabulary before target state
    is stored. Unknown or malicious roles become `control` and cannot write an action sentence.
14. Action receipts carry the role and accessible name of the physical element actually used in
    the same browser response, with no describe round trip. Names default to preserved. A local or
    managed `preserve_target_names: false` removes them monotonically while retaining the closed
    role. Editable values never become labels, and unobservable coordinate subjects fall back to
    coordinates.

## Browser job journeys

1. With policy and the local browser setting both permitting close, open `https://example.com`
   with only `url`, read its useful text with only a tab handle, then close that exact tab. The
   result identifies the governed landing and one terminal outcome per call. Opening uses one
   physical primitive and never creates a blank tab before navigation.
2. List tabs, navigate one, inspect controls, find a target, activate it, and recover from the old
   target becoming stale after a document commit.
3. Capture a screenshot only through `browser_screenshot`, verify bounded dimensions and one
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
16. Resize a controlled tab's window within the documented bounds, observe the resulting geometry,
    and prove resize affects no unrelated window while invalidating stale view geometry.
17. Start a recording, perform ordinary browser work, save without an explicit stop, verify the
    extension's final-frame stop and one bounded GIF content block, save the result again, then
    discard it. Repeat for both browser-local destinations: a semantic target attach and a browser
    download. Prove each returns no content block and that no recording frame ever crosses out of
    the browser.
18. Prove recordings are plural and workspace-isolated; hard timeout, frozen retention,
    browser-loss, service disconnect, runtime hold, memory-limit, oversized-frame, discard, and MV3
    worker-loss paths stop or erase as ADR-0108 requires, without persistent pixel bytes. Prove
    ten byte-identical samples at 100-millisecond intervals retain one frame with a 1,000-millisecond
    visual duration rather than consuming ten frame slots. During recording, prove the perpetual
    scope glow is disabled while transient action feedback remains available, then restored on
    every terminal path and worker recovery.
19. Call `browser_diagnose {}` before tracking, reproduce console and request problems, then read
    bounded problems from both sources. Exercise console-only, network-only, all-detail, literal
    match, cursor continuation, eviction, host filtering, expiry, browser loss, and tab closure.
20. Prove diagnostics never return headers, bodies, cookies, authorization, post data, query
    strings, fragments, or another workspace's entries, and that a read cannot clear history.

## Presentation gates

1. Established controlled-scope, cursor, target, click, drag, field, key, scroll, read, find,
   navigation, screenshot, zoom, recording, signature, denial, attention, and caption treatments
   are visible for applicable 1.0 jobs. Diagnostics remain visually quiet.
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
   An adapter that advertises attention reports it when a browser window gains focus and reports
   truthfully at hello whether it already holds one. Connecting alone never claims attention.
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
   script, file path, file bytes, screenshot, recording frame, GIF, dialog text, console entry,
   network entry, or policy.

## Desktop workbench gates

1. Normal `ghostlight` startup creates the tray and backgrounds the workbench (minimized on Windows,
   hidden on Linux), or restores and focuses the existing workbench. `--headless` starts no desktop
   runtime. Closing destroys only the workbench window, tray Open rebuilds an absent workbench, and
   only explicit quit stops the process. Native minimize does not close or hide the window.
2. Home presents plural session, operation, and browser counts plus current work and system health
   at a glance. Activity, history, checkup, configuration, and installations remain separate
   focused destinations behind one compact rail.
3. Global search covers destinations, sessions, operations, browser instances, terminal history,
   diagnostics, configuration, and supported harnesses. Search input is bounded at the adapter.
4. Reloading the disposable WebView reconstructs its state from `WorkbenchFacade`; it owns no
   product state, runtime token, authority, or durable history.
5. Abnormal Linux WebKit renderer termination discards only the failed workbench. The authority and
   tray remain live, and the next explicit Open creates one replacement without an automatic retry
   loop.
6. Blocked and attention-required operations request at most one content-free native notification
   per invocation. Notification failure cannot change governance, audit, or completion truth.
7. Workbench runtime controls use the existing governance facade and publish the resulting state
   through the existing browser port. The desktop adapter cannot dispatch a browser primitive.
8. MCP integrations explicitly check, connect, and disconnect Codex, Claude Code, Claude Desktop,
   Cursor, Visual Studio Code, Windsurf, Zed, OpenCode, and Crush registrations. Mutations are
   serialized, idempotent, backed up, preserve unrelated entries, and touch only an entry whose
   command identifies Ghostlight's connector.
9. JSONC and Codex TOML comments, trailing commas, formatting, and unrelated values survive
   install and uninstall. Malformed configuration, unreadable files, and foreign `ghostlight`
   entries are left untouched with an actionable result.
10. Harness paths follow the effective Windows or Linux environment. Codex honors `CODEX_HOME`
   before its home-directory fallback. An exact pre-1.0 `ghostlight-relay --role agent` entry in
   the owned versioned install root is updatable; a different command, role, or root remains
   foreign and is reported rather than silently skipped.
11. The WebView loads bundled assets under a restrictive CSP and has no shell, arbitrary file,
   remote-navigation, or network capability. File mutation terminates in the explicit harness
   application service, outside the UI event loop.
12. The desktop executable, tray, bundle, and workbench use the original Ghostlight icon bytes,
    established palette and spring curve, and a static readable reduced-motion treatment.
13. Recoverable Tauri setup or event-loop failure leaves the orchestrator service alive in
    headless mode. The MCP connector, browser connector, shared bridge, and extension have empty
    diffs for the complete workbench feature.
14. Clear view removes completed actions from the current Monitor surface, preserves running work,
    issues no orchestrator mutation, and leaves the durable audit unchanged. A later action appears
    normally, and a fresh desktop process may reconstruct the cleared history from audit.

## Integration and release-readiness gates

1. `cargo fmt --check` passes.
2. Clippy passes with warnings denied for the workspace.
3. Focused unit, contract, and integration tests pass.
4. The full Rust workspace and extension test suites pass in isolated build output.
5. Real MCP stdio, service IPC, browser relay, native messaging, and visible Chromium complete the
   open, read, close journey.
6. The repo-built stack starts, the MCP client sees the exact 22-tool catalog, and the directly loaded
   unpacked extension reports a compatible adapter connection.
7. Every defect found in visible-browser use has a focused regression proof and is reverified.
8. The unpacked extension renders the correct toolbar icon, opens a usable popup, opens settings,
   toggles hold through the established native host, and keeps its id across reload.
9. The complete documented catalog, extension manifest, popup/options states, and all browser-job
   journeys are audited against `INTENT.md`; there are no undocumented placeholders or accepted
   journeys lacking implementation.
10. The bundled workbench HTML, CSS, and JavaScript render against a representative plural-state
    fixture, every destination is keyboard reachable, and frontend scripts pass syntax checks.
