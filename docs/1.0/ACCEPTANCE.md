# Ghostlight 1.0 acceptance

Acceptance is the smallest executable proof of the product language, architecture, safety, and
real journey. Each fact is protected once at its narrowest meaningful seam.

The supported operating-system matrix is Windows and Linux. Every cross-platform source,
process, package, launcher, install, upgrade, and uninstall gate runs against that matrix.

## Contract gates

1. The catalog contains exactly the 23 tools in `LANGUAGE.md`, in documented order, with no
   simultaneously advertised legacy dialect or client-selected profile. Page JavaScript is
   `browser_execute`; the unreleased `browser_evaluate` name is neither advertised nor decoded.
2. Catalog schemas are typo-closed at every object level and match every decoder requirement,
   conditional branch, bound, and default in `LANGUAGE.md`. Every declaration has field guidance
   (the field's own description, a self-describing name with its type, enum, bounds and default,
   or shared guidance in the server instructions; ADR-0198 decision 26), a shortest valid
   example, and standard MCP annotations. Under a strict JSON Schema
   check, every example and at least two ordinary calls for each tool that takes input satisfy
   the advertised input schema, match no `oneOf` more than once, and decode. Every next step that
   names a tool names only fields that tool advertises, and only values a closed field accepts.
   The serialized catalog stays under its size budget.
3. The service's MCP engine serves `tools/list` and `tools/call`, and the MCP edge forwards every
   line unchanged, with no per-tool code.
4. The browser relay carries every adapter frame as opaque bytes, without model-facing names or
   product defaults.
5. An opening that names an earlier relay revision is refused, and a first adapter frame that is
   not a protocol-4 hello hears `adapter_incompatible`, both before work is accepted.
6. MCP and native-message framing survive split reads, coalesced reads, and disconnects.
7. Adding a product feature composed from advertised browser capabilities changes only the
   orchestrator. The edge (the MCP relay and the browser relay), the relay revision, the adapter
   protocol, and the extension stay byte unchanged.
8. The browser relay does not deserialize adapter commands, receipts, events, workspace ids,
   or presentation signals. Unknown bounded adapter payloads round-trip unchanged.
9. The relay protocol, the adapter protocol, and the MCP revision, which only the service
   negotiates, are independent axes. A change to one does not bump or reject an unrelated boundary.
10. The MCP edge process survives a service restart: a call cut off by the restart is answered as
   cut off with the service's own words, and the next request resumes the session by token.
   The browser edge exits when a service accepts again, and the extension reconnects with a fresh
   hello. Lines held while the service was away are delivered once, and a cut-off call is answered,
   never resent (`tests/process-journey.mjs`; the rules are in `crates/edge/src/hold.rs` and
   `forward.rs`).
11. The browser engine suppresses duplicate operation ids, retains a content-free disposition
     across service-worker suspension, and reports uncertainty rather than repeating an effect when
     its prior dispatch cannot be disproved.
12. A real process test starts an MCP edge and a synthetic browser edge, kills the service under a
     held call, and proves the same MCP edge process answers that call as cut off and completes
     new work on a restarted service, while the browser edge exits and a new one connects with a
     fresh hello and a new service epoch (`tests/process-journey.mjs`, its reborn phase).
13. After an edge fails to find the service, it runs `bin/ghostlight-service serve --reason <word>`
     through `relay::spawn::start_service` (`agent` for the MCP relay, `browser` for the browser
     relay) and keeps connecting on its backoff schedule. The MCP relay starts it again for each
     held request; the browser relay starts it once, after the extension's hello, and never while
     waiting. `serve` without a reason is a usage
     error, and a start that finds the lock held waits for it (an `update` start up to 30 s, any
     other start up to 5 s) and then exits 0. Quit holds until an agent request, a command that needs Ghostlight, or a person starts
     it; a browser restart does not bring it back. Before it closes its streams the service says
     goodbye (`quit`, `replace`, `uninstall` or `session_end`), and each edge acts on the reason.
14. Package downloads use one permanent bin directory across releases. `ghostlight deployment
    install` is the only writer of `bin`: it refuses an older payload, leaves the same intact
    version alone, and replaces the service image while asking the running service to quit with
    the `replace` goodbye. Migration redirects owned registrations, preserves history and
    development custody, and removes only obsolete package binaries (ADR-0193).
15. Concurrent demand-start requests converge on one service. Racing `serve` starts leave exactly
    one, the holder of `run/service.lock` (`crates/relay/src/home.rs`, `SERVICE_LOCK_NAME`), and
    presence starts the shell once that service is ready (desktop workbench gate 1). The process
    journey proves the race with ten starts.
16. An explicit `ghostlight open`, and a launch with no arguments, reveal the existing
    authenticated authority's workbench without a new listener or workspace. When absent, they ask
    the shared lifecycle seam to start the sibling as `serve --reason person`, wait for an accepted
    opening, and send control `show`. Nothing serves in the foreground. No supported launch creates
    an authority that refuses presentation by design.
17. An adapter that advertises end-to-end liveness acknowledges content-free heartbeats through the
    browser relay, which passes frames on unopened. A relay socket that stays attached without acknowledgements
    becomes unavailable, while a silent browser operation stays available when its independent
    heartbeat is acknowledged. An unanswered post-dispatch probe leaves that call unknown. It
    makes new calls stop before dispatch only when it went unanswered for at least five seconds:
    a 500 ms presentation deadline with a late acknowledgement leaves the connection available.
    No single call's deadline fails another call in flight; only the heartbeat timeout and a real
    end-of-stream do. A call waiting for a silent adapter sends it one probe at once, and its
    acknowledgement makes the adapter available again. Each lost and resumed acknowledgement is
    a `heartbeat_lost` or `heartbeat_resumed` process-diagnostics record.
18. Two browser identities stay connected at once and each answers its own requests. A second
    connection carrying an identity that is already registered replaces it and closes the replaced
    stream, so the retired connection reaches end-of-stream instead of silently discarding work.
19. A workspace binds to one browser for its life. Physical tab ids resolve only within their
    browser, and a bound browser that disconnects is waited for rather than replaced by another.
    The wait lasts up to about seven seconds, capped by the call's deadline, and the same browser
    reconnecting while it installs its page runtime takes the call. A bound session never reaches
    browser startup. If the browser does not return, the refusal says it is not answering, not
    connected, or not connected while a different browser is (`browser_wrong_profile`). A session
    with no browser yet waits before browser startup only while a browser is evidently returning,
    and the evidence is read again on every poll: a registered connection still initializing or
    silent, a connection that ended less than the grace ago, or a service endpoint published less
    than 1.5 seconds ago, while relays from before a restart reconnect. A call right after a
    service start therefore attaches to the reconnecting browser instead of launching one. A
    browser closed longer ago than the grace earns no wait at all, even if it once reported
    attention, and a start with no relay stops waiting after 1.5 seconds, so browser startup keeps
    the rest of the deadline to launch a browser.
20. A crossing with no binding uses an explicit browser selection, then reported attention, then the
    sole connected browser. Two connected browsers with no evidence produce a refusal naming both
    candidates, with no dispatch and no binding. Listing a workspace's tabs succeeds with no browser
    connected.
21. Every tool result carries the lean model view of its envelope in `structuredContent`: fields at
    their default are omitted, an absent key means its default, and page data such as a script's
    `facts.value` is never rewritten. The ordinary text block carries the authored summary and safe
    next steps, plus the same lean JSON only for an MCP revision before 2025-06-18. The canonical
    envelope stays in the service. The edge has no tool-name dispatch, and image content remains
    separate.
22. A service that holds no stream of any role for 10 minutes exits (ADR-0199 Decision 12). An
    opening of any role disarms the limit and the last close arms it again, an attached shell is a
    stream and keeps the service indefinitely, and work the executor still holds re-arms the limit
    instead of leaving. The idle exit says no goodbye and writes no `state/quit`, so a browser
    start after it is not refused, it launches no shell, and it does nothing once a quit or the
    end of the session is already under way. It is recorded as the `idle_exit` diagnostics event.
    Live: on Linux over SSH a service started by `ghostlight call` exits 10 minutes after the last
    command and `ghostlight status` then says it is not running; on Windows over OpenSSH `status`
    reports `no_interactive_desktop` and the service exits 10 minutes after the last command; on
    the owner's desktop the service never idles out while the icon is shown.

## Executor and truth gates

H8 adds the isolated `tests/local-resilience-journey.mjs` lane, which runs the real service in a test
home on relay 4: the endpoint is private to its owner; silent, torn, malformed, oversized and
older-revision peers are retired while healthy clients stay served, and a later revision is
accepted; an ordinary burst completes; a repeated request id is refused and the original stays
cancellable, and work that may have acted ends uncertain and is not replayed; a full queue refuses
with no effect while controls and another workspace stay usable; queued cancellation and expiry
have no effect; human Pause and Stop from the browser stop dispatch with no effect, Stop ends
queued work with no effect and leaves the call in flight uncertain, and Resume and a new session
restore work without replay; and a client that stops reading is retired alone. The actual bundled
workbench shows waiting becoming running once.

1. Direct operations and composition steps use the same operation executor. The parent completes
   once. Each attempted child records exactly one safe receipt before parent completion under the
   same lease and snapshot. Preparation failures and unconfirmed receipts never fabricate execution.
2. One invocation can commit only one terminal outcome.
3. Deadline or cancellation before dispatch reports no effect and is repeat-safe when the job is.
   Cancellation says it was cancelled, never that the browser disconnected, and suggests nothing.
   A deadline suggests a longer `timeout_ms` only for a call that accepts that field and is below
   its 30000 ms maximum; every other call, including `browser_fill_form` at its default, is told
   to repeat the call and narrow a very large page.
4. Disconnect or cancellation after uncertain dispatch reports unknown effect and no replay advice.
   A deadline after dispatch says to check the page before repeating.
5. Flows count successful steps as completed. Mixed Continue remains non-success.
   Known partial effects stay partial; later uncertainty preserves prior known effect counts.
   Unable-to-start and never-run steps remain distinct. Result omission preserves status/effect
   metadata. Recovery respects completed work; human directives and invocation limits beat Continue.
   Safe repetition requires every step to succeed without effects and permit repetition.
6. A stale tab handle, and a target handle the service no longer knows, fail before browser
   dispatch and suggest obtaining current handles. A target handle the page finds removed fails
   inside the page call before any effect, so nothing was sent, and may offer a trusted repeat
   step when the page proved one control is the same. Otherwise it offers current handles to pick
   from.
   A call with no `tab` in a session without tabs says no tab is open yet and points to
   `browser_navigate`; with several plausible tabs it says the call must name one. A policy
   denial points to `policy_explain`.
7. Lower-capability-model fixtures succeed with every documented shortest call, choose the correct
   sibling among related tools, and recover from deliberately stale and ambiguous handles.
   The shortest `browser_read` call returns visible text from the composed page: top document, open
   shadow roots, and http(s) embedded frames in stable order under one global character ceiling.
   It includes visible form values, reports checkboxes and radios as checked or unchecked, never
   reports placeholders as values, and masks every credential-class field. It excludes hidden
   content and closed shadow roots. Explicit article mode
   returns a useful top-document article or falls back to that same composed page read.
   Inspect, find, accessible-name fallback, and text conditions walk the same composed tree but
   read page text only: no field values, checked words, credential markers, or editable content. A
   rootless document tree includes embedded frames under one 400-node ceiling. A document-scope
   inspection returns that tree to the caller, and its diff paths are positions in it.
8. A screenshot view handle resolves image coordinates only while its tab, document generation,
   viewport, and zoom still match; stale views fail before pointer or region-capture dispatch. A
   bounded region returns a magnified image and fresh view, and a second region chains from it.
9. File paths are validated and bounded before reading, and no file bytes cross the adapter protocol
   until governance and file-target preflight succeed.
10. The extension alone owns recording identity, frame acceptance, deadlines, memory bounds, stop,
    retention, and erase. The orchestrator checks source authority before start and disclosure.
    Status, stop, and discard remain available without new browser authority.
11. Diagnostic reads are cursor-based and non-destructive. Enabling, filtering, sanitization, host
    authority, bounds, expiry, and eviction are enforced before model-visible results.

## Governance gates

1. With no active policy, local and remote HTTP(S) browser work is permitted, including localhost,
   its subdomains, loopback, link-local metadata, and IPv4-embedded IPv6 destinations.
2. Non-HTTP(S) schemes remain denied. Local destinations follow authored host and capability
   rules and policy-defined never-touch hosts.
3. RAWX is independent: each operation uses its complete set, and composition steps are admitted
   separately. Empty wrapper or local-window operations do not invent authority. Current audit has
   one parent composition record plus safe incremental child receipts. Grouped history and policy
   previews use the child evidence without double-counting the aggregate (ADR-0156).
4. Strict schema-4 grants use exact, suffix, universal, tie-deny, ordered-first-admission, per-grant
   deny, and layer-intersection rules from ADR-0121. Schema 4 has no observe mode, channel
   switches, user-policy switch or frame options. A schema-3 file fails closed with the migrate
   sentence, `ghostlight policy migrate <file>` moves a file only where its meaning is kept and
   keeps a `.schema3.bak` copy, Ghostlight's own user file moves at load by the same rule, and
   administrator files are never rewritten.
5. Tool calls cannot author host or capability policy. Obsolete request fields fail before effects.
   Started work keeps one immutable configured authority snapshot even if configuration changes.
6. A configured invalid cold source fails closed. An invalid replacement retains last valid for
   future snapshots and never falls back to unrestricted.
7. Ownership and leases are checked immediately before effects.
8. Initial, redirected, and script-caused committed landings are governed before text or readiness
   is accepted.
9. A denied landing returns truthful committed-effect and compensation facts.
10. Audit records contain no URL, arbitrary page text, selector, target handle, form value,
   screenshot, recording frame or GIF, dialog text, console text, request URL, header, body, or
   diagnostic entry. A successful action may retain one governed, normalized, bounded target name
   in its Ghostlight-authored summary.
   Failed flows also exclude earlier successful child results. Arbitrary exceptions cannot enter
   retained summary text. Synthetic input, path, script-result, and exception sentinels remain
   visible in permitted client results where applicable and absent from serialized audit.
   Historical records remain readable without re-emitting their arbitrary failure-facts payloads;
   the original files remain unchanged.
11. Audit carries the complete RAWX set, authority, tier, grant, rule, deterministic denial
    id, and managed sequence without policy payloads.
12. Hold, end-session, and cancellation stop later effects at the runtime boundary.
13. Repeated enforced denials never create an automatic session hold. A later permitted request
    succeeds without review/resume, in the same workspace and in other workspaces.
14. Model-driven close dispatches only when the action capability and monotonic tab-close policy
    constraint both permit it. A denying local or managed layer cannot be expanded later.
15. A refused explicit navigation records only the normalized attempted host. Its path, query,
    fragment, request value, target description, and page text remain absent from summary,
    observation, presentation, and audit.
16. A page-authored role is narrowed to the closed Ghostlight role vocabulary before target state
    is stored. Unknown or malicious roles become `control` and cannot write an action sentence.
17. Action receipts carry the role and accessible name of the physical element actually used in
    the same browser response, with no describe round trip. Names default to preserved. A local or
    managed `privacy.preserve_target_names: false` removes them monotonically while retaining the closed
    role. Editable values never become labels, and unobservable coordinate subjects fall back to
    coordinates.
18. No managed bootstrap performs no policy network work. Signed file and HTTPS bundles share one
    verification path. Ed25519 is required; configured ML-DSA-65 makes both legs mandatory.
19. Verified cache recovers offline. Bad signatures, malformed bytes, rollbacks, reused sequence,
    and unreachable updates keep last-known-good. Configured cold start without valid source or
    cache fails closed.
20. The Policy Passport shows organization, verification, sequence, freshness, source class,
    timing, rationale, and contacts without source address, credentials, keys, or rules.
21. Authority changes update policy-aware catalog discovery and emit one MCP tool-list change;
    all-open remains the exact canonical 23-tool catalog.
22. A hand edit to a policy file takes effect within about a second, a workbench edit at once, and
    an administrator's HTTPS policy fetch never runs inside a tool call.

## Browser job journeys

1. With policy and the local browser setting both permitting close, open `https://example.com`
   with only `url`, read its useful text with only a tab handle, then close that exact tab. The
   result identifies the governed landing and one terminal outcome per call. Opening uses one
   physical primitive and never creates a blank tab before navigation.
2. List tabs, navigate one, inspect controls, find a target, activate it, and recover from the old
   target becoming stale after a document commit. An anchor link or a `history.pushState` route in
   the same origin keeps the target valid, still checks the new address against landing
   governance, and expires views; an address in another origin stales the target.
3. Capture a screenshot only through `browser_screenshot`, verify bounded dimensions and one
   MCP image content block, and verify base64 image data is absent from structured facts. Capture
   and magnify one bounded view region, then repeat against the returned view.
4. Fill multiple fields. If any described target is credential-class and
   `user_authorized_credentials` is absent or false, return guidance before any input dispatch
   and leave subsequent permitted work available. With explicit user authorization acknowledged
   as true, grouped fill and targeted/focused typing enter credentials under ordinary policy and
   human controls. Verify actual retained password input without retaining its value in evidence.
   The physical fill carries the remaining invocation budget, uses one all-field preflight and one
   terminal batch verification, and leaves enough time for a decisive receipt. The 30-second
   default must support ordinary multi-field work without multiplying a fixed stability delay by
   field count.
   A controlled ordinary input and textarea must receive native editing evidence and retain their
   values after a later framework-style model render. The fixture first proves that prototype
   setters plus generic synthetic events are rejected by the same render. Neither path submits.
   On a page that resets drafts after focus/visibility refresh, retained controlled tabs emulate
   focus between calls. Fill without submission, leave the physical tab inactive for at least
   45 seconds, return, click a field, and verify every value. Separately verify normal focus is
   restored on pause, stop, disconnect, and release, and unowned tabs are unaffected. Do not use
   emulated-focus retention as evidence of retention after handoff (ADR-0168).
5. Wait for success and timeout branches of each condition family.
6. Run click plus wait directly and as a flow with omitted step IDs; observe the same executor behavior.
7. Accept, dismiss, and supply non-secret prompt text to a visible dialog.
8. Disconnect the adapter before dispatch and after dispatch; observe no-effect and unknown-effect
   truth respectively.
9. Activate an existing tab, move backward and forward through history, and reload through the
   same landing-governance path as explicit navigation.
10. Traverse controls inside an open shadow root and perform semantic click, fill, type, hover,
    scroll-to-target, and drag journeys with actionability checked at dispatch time.
    Repeat target hover, drag, capture, point-subject, and coordinate-drop geometry through an
    iframe hosted inside an open shadow root.
11. Capture a screenshot, click and hover through its view handle, magnify a bounded region and
    chain another region from the result, then verify document commit, zoom, and viewport changes
    make the old view stale.
12. Scroll in four directions, reveal a target, and set zoom while returning observed viewport
    facts rather than assumed effects.
13. Upload one and several bounded local files to a file input, including one with credential-like
    metadata. Explicit recording save to that control works too. Reject missing, directory,
    oversized, changed-after-read, and non-file targets before any browser effect.
14. Run an explicitly authorized script, return a bounded serializable value, govern any committed
    landing, and never audit source or result.
    - A runtime exception containing `Illegal return statement` and a runtime-thrown `SyntaxError`
      each follow one synthetic effect; neither repeats it or reports a known no-effect failure.
    - Supported bare returns, top-level await, repeated REPL declarations, and resource
      declarations retain their useful results. Invalid syntax sends no page evaluation.
    - `tests/script-browser-journey.mjs` verifies the shipped extension's evaluation in real Chromium
      and its receipts through the real browser edge, service, and MCP edge in a test home. Its
      native-port discovery is a loopback test bridge (`tests/lib/browser.mjs`); it does not
      substitute for installed-extension/native-host acceptance.
15. Manually navigate a controlled tab to a prohibited destination. No activity, audit record,
    policy response, or tab hold is produced. The next agent request checks the current URL and
    refuses prohibited work. Opening a child tab from a controlled page does not adopt, group,
    or attach a debugger to it. Moving tabs between windows does not trigger regrouping.
    Cached agent targets still expire on document changes (ADR-0164).
16. Resize a controlled tab's window within the documented bounds, observe the resulting geometry,
    and prove resize affects no unrelated window while invalidating stale view geometry.
17. Start a recording, perform ordinary browser work, save without an explicit stop, verify the
    extension's final-frame stop and one bounded GIF content block, save the result again, then
    discard it. Repeat for both browser-local destinations: a semantic target attach and a browser
    download. Prove each returns no content block and that no recording frame ever crosses out of
    the browser.
18. Prove a session holds one recording that no call names, that a second start is refused until
    the first is saved or discarded, that a start after a save replaces the saved recording, and that sessions are isolated from each other; hard timeout, frozen retention,
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
21. With `Auto-open browser on request` off and no adapter connected, call a browser tool and
    verify no browser starts. The failed result tells the model to ask the user to open one of
    every eligible installed browser it names with the Ghostlight extension installed, carries
    the same names in structured facts, and does not repeat the instruction in `next_steps`.
22. With several installed browsers and no adapter connected, no browser refusal names a choice
    or says Ghostlight declined to choose: plural evidence returns the ask in either posture,
    stale same-tree registrations are repaired silently before the ask, and nothing usable
    answers with one named choice-free remedy instead (ADR-0149).
23. A registration owned by another Ghostlight installation is never adopted by recovery: the
    call fails with `native_host_owned_elsewhere`, the machine's registration is byte-identical
    afterward, `doctor` names the owning directory and marks it removed when that installation
    no longer exists, and a deliberate `ghostlight install` from the adopting installation is
    what takes ownership (ADR-0149 amendment).
24. A session cannot list, switch to, or act on another session's workspace. A tab or target
    handle from another session is refused before any browser work and offers no switch. The
    refusal names the owner only by its client label, with a next step to the caller's own
    `browser_tabs` list. An owner with an empty label gets the generic refusal, so no refusal
    carries a workspace id. The caller's own tab list never shows the other session's tab, and a
    retired `browser_workspace` call, direct or inside `browser_flow`, fails without dispatch
    (ADR-0196).
25. Default reuse never takes a person's tab. With a person-opened tab on the same host, a plain
    `browser_navigate` asks the browser only for a fresh tab with `reuse` set to `never`, and the
    person's tab is not regrouped, navigated, or attached. A tab Ghostlight opened and a released
    session of the same client left open is reused by the next open on its host in the same
    browser, after a live check that it still shows its release address, and the summary says it
    was reused. A released tab becomes reusable only once its release close is refused by the
    preserve-tabs setting. Before that answer, or after a close that worked, the next open creates
    a fresh tab. A page commit, a close, or a browser disconnect since the release removes it from
    reuse. A tab released by a `ghostlight call` session is not reused by a Claude Code session,
    which gets a fresh tab in its own `Ghostlight - Claude Code` group, while the next
    `ghostlight call` session still reuses it. Chrome's tab 5 and Edge's tab 5 are different tabs,
    and a workspace refusal found after the browser opened a tab reports an applied effect
    (ADR-0196 Decision 9).

## Presentation gates

1. Established controlled-scope, cursor, target, click, drag, field, key, scroll, read, find,
   navigation, screenshot, zoom, recording, signature, denial, and caption treatments
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
3. The adapter's first frame is the protocol-4 hello. It carries the persistent browser id, the
   browser's product name, platform and engine version, the adapter version, and whether the
   browser already holds a focused window. The service answers with `hello_accepted`: its epoch,
   version, control state and the page assets with an id. Service behavior is gated by the
   protocol, not by the extension version string, and a service that cannot answer a protocol-4
   hello is called outdated. An adapter reports attention when a browser window gains focus and
   reports truthfully at hello whether it already holds one. Connecting alone never claims
   attention. Every primitive the adapter answers has a production handler and golden coverage, and
   an unknown primitive is answered `unsupported`. When the first connection finds the service
   absent, the extension opens its own bundled setup page once; the page loads nothing from the
   network.
4. The page assets arrive from the service in `hello_accepted` and are installed only after the
   adapter has checked them against their SHA-256 id. They run in the isolated world `ghostlight`
   of every controlled page session, the root page and each out-of-process child, and assets that
   fail the check are refused. Every already-loaded and future permitted frame, including an
   out-of-process cross-origin iframe, has the exact assets before work reaches it. No file in the
   extension package holds page code.
5. The popup distinguishes disconnected, connected-active, held, and ended
   states without reading page content.
6. Popup pause, resume, end-session, and start-new-session actions become authoritative service
   runtime state before the next physical effect. Ordinary resume cannot revive an ended session.
   The keyboard command uses the same intent path.
7. Options use the Ghostlight kit, the sylin.org night ground and the #5eead4 accent, and persist
   effects-on, captions-off, diagnostics-off, and preserve-tabs-on defaults under local keys.
   Preserve tabs is a final physical interlock: it can refuse model-driven close but cannot edit,
   import, expand, or override authority. Human browser closure remains available.
8. Each exact `Ghostlight - <client label>` title has one canonical blue group across normal
   browser windows. A new tab is created directly in that group's window. With no Ghostlight group,
   the first URL opens in a dedicated normal window; no tab is inserted into the user's active
   window and no blank tab is exposed. Concurrent opens cannot create duplicate groups. An opener
   does not grant ownership: child tabs and popups are not automatically adopted, grouped, or
   attached to a debugger. Manually moving tabs causes no regrouping; closing or moving tabs does
   not transfer ownership (ADR-0164).
9. The established content-free visual language renders visibly, stays pointer-transparent with
   no focusable controls, remains hidden from screenshots, and remounts after navigation. A missing
   stylesheet cannot expose buttons or create an input-intercepting layer. Runtime-state
   transitions create no page dialog. Its
   palette is sky `#38bdf8`, ink `#eaf6ff`, and governance ground `#0c0f14`; its spring curve is
   `cubic-bezier(.22,1,.36,1)`. The 150 ms cursor, four-second scope breath, 620 ms ripple, 700 ms
   field splash, 1450 ms read scan, 1600 ms navigation pill, 1500 ms capture frame, and 1150 ms
   zoom frame are protected product contracts, including their reduced-motion alternatives.
10. The popup and options pages have useful empty, disconnected, error, and incompatible-version
   states and are keyboard accessible.
11. No extension storage key contains URL, title, page text, target name, locator, form value,
   script, file path, file bytes, screenshot, recording frame, GIF, dialog text, console entry,
   network entry, or policy.
12. Developer diagnostics form tracing stays off by default, observes only already-controlled
    top documents, stops on disable/release or after ten minutes, and examines at most 100
    ordinary controls. Empty-state changes, control replacement, reset/focus/visibility events,
    and explicit Ghostlight fill/type/clear boundaries produce only closed structural metadata.
    Input activity is coalesced without per-key counts. Credential values are never read;
    unique values, attribute names, event data, and key sentinels never enter the 400-record
    persisted/exported ring. Storage or observer failure cannot change an action's return or
    throw, focus, form values, or submission count. Passive traces never become service work.
13. Ordinary text input and textarea fill/clear use one native browser edit on the named control.
    If that edit is unavailable, the operation refuses with the existing draft intact and without
    synthetic input/change events or submission.

## Desktop workbench gates

1. Whatever starts the service, the shell appears whenever a display is reachable: the service
   starts `ghostlight-desktop --tray` 2 s after it is ready, shows only the icon, and starts a shell
   with no argument when the window is asked for. An explicit `ghostlight open` or a watcher's
   Continue opens and focuses the window. Windows restores its existing view; Linux coalesces Open
   requests and reconstructs its disposable view after the old window's destroyed event. The
   Applications entry and `ghostlight open` remain available when the desktop has no tray. Closing
   destroys only the workbench window, the tray's Open rebuilds it, and only explicit quit stops the
   service. Native minimize does not close or hide the window.
   Concurrent Open calls during startup and after close produce one responsive native workbench
   for the exact service. Windows acceptance counts actual shell windows, including hidden
   windows; a single service PID or a mocked activation result is insufficient evidence.
   Open tolerates delayed native startup within its bounded wait; an expired wait permits retry
   without telling the person to stop a healthy authority. Ghostlight never delays a sign-out or
   shutdown, and starts nothing once the session is ending. Every process (the service, the edge
   in every role and every one-shot command) owns the session listener: on Windows a hidden
   top-level window, on Linux SIGTERM and SIGHUP. `WM_QUERYENDSESSION` never vetoes the end, and
   the final signal ends the exact process within two seconds even when Tao has already destroyed
   its event target. A console-control fallback never suppresses the default process exit. The
   service says `session_end` goodbye on every stream first. While the session is ending, no
   service, browser or page is started by any process that heard the end. On Windows that is every
   process, one born after the broadcast included, because each also asks the system whether the
   session is shutting down. On Linux only a process that received the signal refuses.
2. Home presents plural session, operation, and browser counts plus current work and system health
   at a glance. The persistent lamp band states whether Ghostlight is all-open, has one or more
   policies applied, or has a configured policy issue, and links to Status for detail. Activity,
   history, checkup, configuration, and installations remain separate focused destinations behind
   one compact rail.
3. Global search covers destinations, sessions, operations, browser instances, terminal history,
   diagnostics, configuration, and supported harnesses. Search input is bounded at the adapter.
4. Reloading the disposable WebView reconstructs its state from `WorkbenchFacade`; it owns no
   product state, runtime token, authority, or durable history.
5. Abnormal Linux WebKit renderer termination discards only the failed workbench. The service and
   tray remain live, and the next explicit Open creates one replacement without an automatic retry
   loop.
6. Blocked operations request at most one content-free native notification
   per invocation. Notification failure cannot change governance, audit, or completion truth.
7. Workbench runtime controls use the existing governance facade and publish the resulting state
   through the existing browser port. The desktop adapter cannot dispatch a browser primitive.
8. MCP integrations explicitly check, connect, and disconnect Codex, Claude Code, Claude Desktop,
   Cursor, Visual Studio Code, historical Windsurf, current Devin, Zed, OpenCode, and Crush
   registrations. Mutations are
   serialized, idempotent, backed up, preserve unrelated entries, and touch only an entry whose
   command is Ghostlight's edge (`ghostlight mcp`). One `Set up everything` action adds missing
   registrations and updates Ghostlight-owned registrations for every detected target while
   skipping current and not-detected targets. One target's environmental failure does not prevent
   independent targets from being attempted, and foreign or malformed entries remain untouched by
   every automatic path. A parseable foreign entry offers one confirmed per-target Fix that
   replaces only Ghostlight's key after creating a byte-exact backup and re-checking eligibility.
   Windsurf and Devin share one product card but retain their own executable evidence and config
   path, so neither generation can make Ghostlight write the other generation's file.
9. JSONC and Codex TOML comments, trailing commas, formatting, and unrelated values survive
   install, uninstall, and Fix. Malformed and unreadable configurations are left untouched with an
   actionable result. Foreign `ghostlight` entries remain untouched unless the person confirms the
   dedicated Fix action; repeat or stale Fix refuses without changing bytes.
10. Harness paths follow the effective Windows or Linux environment. Codex honors `CODEX_HOME`
   before its home-directory fallback. An exact pre-1.0 `ghostlight-relay --role agent` entry in
   the owned legacy install root is updatable; a different command, role, or root remains
   foreign and is reported rather than silently skipped.
11. The WebView loads only the service's loopback workbench, exactly `http://127.0.0.1:<port>/`
   carrying the window's bearer, under the service's restrictive CSP, and the shell's own state
   screens, which are handed nothing. Any other navigation is refused, and a link the page wants
   opened goes to the system opener through the service. The WebView has no shell or arbitrary
   file capability. File mutation terminates in the explicit harness application service, outside
   the UI event loop.
12. The desktop executable, tray, and workbench use the original Ghostlight icon bytes,
    established palette and spring curve, and a static readable reduced-motion treatment. The
    tray's state dot is drawn at run time over the original icon bytes, in the band's colours;
    the ready icon is the original bytes.
13. A broken window never stops agent work. A shell that cannot start, a window that cannot be
    built, a missing display, web view runtime or tray, and a shell that vanishes leave the service
    serving; `status` and `doctor` say why there is no window, and presence relaunches a vanished
    shell at most three times in two minutes. The edge, the relay, and the frozen extension have
    empty diffs for the complete workbench feature.
14. Clear view removes completed actions from the current At a glance surface, preserves running work,
    issues no orchestrator mutation, and leaves the durable audit unchanged. A later action appears
    normally, and a fresh desktop process may reconstruct the cleared history from audit.
15. A Linux user install owns one XDG Applications entry and one byte-identical icon. The entry
    runs `bin/ghostlight-desktop` from the one fixed `bin` directory, which has no version path;
    repeat install is a no-op, foreign state is preserved, and repeat uninstall is a no-op.
    On Windows, uninstall removes the Start-menu shortcut and the per-user Uninstall key, each only
    when it is this home's (the shortcut starts a program in `bin`, or carries the AppUserModelID
    when no target can be read from it; the key's command or location points into the home),
    leaves and names any other, including another home's shortcut, and a repeat
    uninstall is a no-op.
16. Install and doctor distinguish native, Snap, Flatpak, multiple-sandbox, missing, and unchecked
    browser package forms from native-host registration. Default Linux setup selects only detected
    native browsers, while `--all-browsers` remains explicit pre-registration.
17. When an authority is already running, text and JSON `doctor` read its exact orchestrator-owned
    readiness projection and use the same word and detail as At a glance. Inspection does not
    demand-start, reveal the workbench, admit a channel, open a session, or write audit. An absent
    or lost authority never causes `doctor` to fabricate live readiness.
18. When configured authority fails closed, readiness reads Policy not valid in the attention tone
    and points to the Policy page, never Ready. The header, the Status authority card, and the
    Policy tab read the one validity governance enforces. A failed reload that keeps the last valid
    policy in force stays Ready, keeps its Policy situation, and shows a Status warning.
19. Pause, Resume, End session, and Start session are offered and applied while the authority is
    reachable and no browser is connected. A workbench that loses the authority disables the header
    control until it reaches the authority again, even though its last snapshot offered control.
    Each confirmation states only the resulting authority state, identically whether control
    publication to adapters succeeded, failed, or reached none.
20. The service links no UI toolkit and starts with no display. `cargo tree` for
    `ghostlight-service` on the Windows and Linux targets, normal and build edges, holds no
    `tauri*`, `tao`, `wry`, `gtk`, `gdk`, `webkit2gtk`, `javascriptcore`, `soup3`,
    `libappindicator`, `tray-icon`, `muda` or `webview2-com`, and none of `tokio`, `reqwest`,
    `hyper` or `socket2` (`tests/guards/connectors.test.mjs`). On Windows Task Manager reads
    "Ghostlight service" for the service, "Ghostlight" for the window and "Ghostlight connection"
    for each edge. Every start Ghostlight itself makes (the edge, Chrome, agents, presence, the
    shell and open) shows no console window. On Windows 11 24H2 and later no direct start of the
    service image shows one either. Before 24H2 two starts may: the Uninstall entry in Settings >
    Apps prints its outcome in a console, and ADR-0199 Decision 4 accepts a brief console flash
    on Windows 10. On Linux with
    `DISPLAY` and `WAYLAND_DISPLAY` both unset the service starts, publishes readiness and
    `status` says `no_display`.
21. A service-first install on a computer with a supported browser and no extension prints the
    version, the location, the agents and one Next line, and shows the Ghostlight window on the
    browser step of its setup view. `--dry-run`, `--no-open`, CI and an app container show no
    window, and an install after setup is complete prints no Next line and shows nothing.
22. An extension-first install opens the bundled setup page once, and only when Ghostlight is
    absent at the extension's first connection. The page loads nothing from the network. Where
    Ghostlight is present the extension opens no page, and its Open Ghostlight shows the window.
23. The browser step lists the installed browsers and acts on them: Add starts that browser at
    the store listing, Open starts it in the profile that holds the extension, and Get Google
    Chrome appears only when no supported browser is installed. While setup is incomplete the
    service reads each browser's `Local State` and `Extensions` folder; the read writes, logs and
    sends nothing. A watcher in a browser tab sees the ordinary workbench.
24. The first MCP request that ran in a browser and succeeded completes setup: the view shows
    Ready and goes to At a glance, `state/setup.json` exists, the next start opens on the
    workbench and the setup view retires. A `ghostlight call` does not complete it, and a home
    upgraded from 1.x opens on the workbench. Until then, while no browser profile holds the extension, an agent's
    browser call that finds no browser connected says in its answer where the extension comes from,
    and opens no window.
25. With reduced motion, the setup page, the popup and the window's setup view show every state
    with the same words at the same moments and play no animation, and each resting look reads
    without it.

## Integration and release-readiness gates

1. `cargo fmt --check` passes.
2. Clippy passes with warnings denied for the workspace.
3. Focused unit, contract, and integration tests pass.
4. The full Rust workspace and extension test suites pass in isolated build output.
5. Real MCP stdio, service IPC, browser relay, native messaging, and visible Chromium complete the
   open, read, close journey.
6. The repo-built stack starts, the MCP client sees the exact 23-tool catalog, and the directly loaded
   unpacked extension reports a compatible adapter connection.
7. Every defect found in visible-browser use has a focused regression proof and is reverified.
8. The unpacked extension renders the correct toolbar icon, opens a usable popup, opens settings,
   toggles hold through the established native host, and keeps its id across reload.
9. The complete documented catalog, extension manifest, popup/options states, and all browser-job
   journeys are audited against `INTENT.md`; there are no undocumented placeholders or accepted
   journeys lacking implementation.
10. The bundled workbench HTML, CSS, and JavaScript render against a representative plural-state
    fixture, every destination is keyboard reachable, and frontend scripts pass syntax checks.
11. Release Linux binaries are built on Ubuntu 22.04.

## Grouped history evidence (ADR-0156)

1. Equivalent direct and composed work retains equivalent decisions, requirements, effects,
   measurements, and safe summaries. Caller labels, results, and arbitrary errors stay out.
2. Actual policy evaluation records all evaluated layers and distinguishes all-open, granted,
   and refused work. Permission traces are bounded and disclose omission.
3. The workbench keeps the parent active as child receipts arrive; restored incomplete groups
   state that completion was not recorded. Missing evidence never becomes a no-effect claim.
4. Groups start collapsed. Opening brings the first problem into view; updates preserve expansion,
   scroll, and keyboard focus. No routine child notifications. History retains at most 500 whole
   groups, and policy previews count actual recorded child operations.
5. `tests/process-journey.mjs` checks real JSONL during parent execution and after completion.
   `tests/workbench-surface.mjs` checks projection updates and escaping. The isolated Chromium
   `tests/workbench-history-browser.mjs` checks the bundled UI at 1280 and 720 pixels using synthetic
   events. These do not establish installed shell/MV3 deployment or H5 timing guarantees.
6. Ordinary action names toggle inline hero details with mouse, Space, and Enter. Receipt updates
   and newer actions retain open panels and keyboard focus. Expanded content fits its row at both
   widths, with no horizontal overflow. Pause precedes At a glance visually and in keyboard order.

## Explicit control and credential-input evidence (ADR-0185)

1. A call that sends `restrict_hosts` or `restrict_capabilities` fails before dispatch under every
   policy. Configured enforcing policy still denies direct and composed work. Policy denials never create an automatic
   hold.
2. After any number of matching or mixed denials, the same session can perform permitted work
   without a human review or resume. Other sessions remain independent. Flow stop/continue applies
   normally to ordinary refusals.
3. Credential input without acknowledgement returns guidance and dispatches no input, including
   an ordinary field before a credential field in grouped fill. Subsequent ordinary input works
   immediately. A first-call or later `user_authorized_credentials: true` reaches password input
   for grouped fill and targeted/focused typing. It cannot bypass configured policy or Pause/Stop.
4. Control arriving after preparation or during writer queuing prevents the next command. Queued
   cancellation/deadline also sends no command and leaves no pending receipt. Applied and uncertain
   receipts keep their facts when controls change after dispatch; postcondition refusal preserves
   the previously acknowledged action. Its summary names that action before saying why the check
   did not confirm it, and never says nothing happened.
5. Human Pause and Stop still prevent the next browser effect. Resume
   permits new requests without replay or permission changes. Page feedback contains no buttons,
   focusable elements, or input-intercepting layers, even when its stylesheet is unavailable.
6. Rust application/relay tests and the fresh-build process journey cover timing and isolation with
   fake/synthetic browser adapters. The isolated Chromium history journey exercises bundled UI.
   These do not establish installed shell/MV3, physical input timing, or Linux runtime evidence.

## Document boundary evidence (ADR-0158)

1. Admit each supported document for actual RAWX requirements before extracting content. A denied
   child contributes no text, labels, values, or target handles. Frame-id reuse cannot revive a
   stale document-bound target. Known denied batch targets refuse before the first effect.
2. Document handling is fixed: permitted documents are read, excluded ones are left out and
   reported, and a notice appears when an exclusion affects the work. Unrelated permitted targets
   remain usable.
3. Distinguish unavailable content, policy exclusions, and size limits; negatives describe only
   inspected content. Durable coverage excludes observed origins and payloads; human host details
   are volatile. Preserve limitations across physical preparation and action receipts.
4. Full, viewport, target, and magnified screenshots share one binary policy allowance. Full-open
   and permitted captures return complete images without document inventory or masks. A disabled
   screenshot returns no image and sends no capture command. Visible embedded pixels do not grant
   semantic or coordinate-action authority. Organization refusal cannot be relaxed locally.
   Restricted recordings stop
   at document-set changes, and their prior frames remain subject to current source authority on
   every export destination. Source-bound overflow is explicit rather than silent eviction.
5. Scripts refuse when document exclusions or unavailable evidence prevent bounded admission.
   All-open scripts keep browser behavior without an inferred sandbox or website network filter.
6. `tests/frame-browser-journey.mjs` passes its Chrome/MV3 checks on Windows using independently
   checked Sylin iframe demo snapshots and form content on distinct local hosts, with the real service,
   browser edge and MCP edge in a test home. The page cannot see Ghostlight's runtime; only the
   extension's isolated worlds hold it.
   Native-port discovery alone is a test pipe; installed registration and other platforms are
   separate lanes. See the H6 verification record for source hashes, artifacts, and limitations.

## Audit health evidence (ADR-0159)

1. A failed append or sync leaves the actual outcome, effect, and repeat safety intact. Results
   and human history distinguish confirmed storage from a volatile receipt. A saved parent does
   not upgrade children whose storage was unconfirmed.
2. Keep working is the default. Either policy layer can require audit; a lower layer or a grant
   cannot relax the requirement. Known failure refuses browser work before startup
   recovery and at the final dispatch callback, including later children under Continue.
3. Storage recovery is bounded and serialized with writes. It reopens the repaired destination,
   synchronizes a content-free gap marker, and permits new requests without replay or backfill.
4. Cold storage failure keeps the service, workbench, diagnostics, and human controls reachable.
   Damaged and oversized history entries do not erase readable neighbors or grow line buffers
   without bound. Unreadable historical portions and surviving gap counts remain explicit.
5. At a glance shows one persistent health indication; Status reports health; Policy authors the
   closed choice and respects organization floors. Saved and unconfirmed steps remain distinct
   through updates and recovery. Audit refusal does not emit repeated native notifications.
6. Rust sink/executor/history tests, the real-process journey with a synthetic browser adapter,
   and the bundled workbench in isolated Chromium own these checks. Installed native transport,
   active shell deployment, Linux runtime, and storage power-loss behavior remain separate lanes.

## Connection reporting (ADR-0161)

- Two different connections sharing one workspace, and the actual MCP edge, each receive their
  own service-issued connection identity and channel. No executable or signature is observed.
- Active, queued, composed, preparation-failed, cancelled, capacity-refused, and invalid work keep
  their original connection evidence. A later connection or released workspace cannot rewrite it.
- Each reconnect gets new evidence. Session details retain plural active connections and remove
  only the closed one. Action details remain associated with their original connection.
- Raw reported application names remain in bounded live human details only. Serialized audit and
  enabled service and edge diagnostics exclude unique claim sentinels. Durable history retains
  bounded evidence after restart; history written before connection identities existed says so.
- Human details show the reported application, the connection and its channel, with one sentence
  that says what the connection does and does not establish. Details are quiet by default, escape
  claims, preserve expansion/focus on refresh, and fit a narrow workbench.

This foundation adds no executable observation, signature/hash verifier or identity admission.

## Consolidated tool surface (ADR-0162)

### Bounded preparation (ADR-0195)

All thirteen ordinary page tools accept `visual_settle`, default true. Each call
spends at most 1000 ms on optional preparation and proceeds when the page stays busy.
Opt-out spends no preparation budget. Targeted operations tolerate unrelated frame
churn while required document replacement, excluded input and human controls stop work.
Composite waits preserve satisfied primary conditions and separately report visual
settlement. Explicit visual and layout waits retain their requested conditions.
Verify actual effects and retained values in the ordinary installed browser, including
animation and auxiliary-frame navigation. This does not prove the original incident's
exact routing trigger. Older adapter compatibility is verified independently.

### Consolidated inputs

1. No schema exposes request host/capability restrictions, flow dry-run, or browser_sequence.
   Their presence is rejected without browser effects, including obsolete fields in later children.
2. Short and named flows use ordinary tool arguments. IDs default deterministically, duplicates
   and explicit empty IDs fail, and result references retain their original semantics.
3. A flow's tab defaults its tab-scoped children, explicit child tabs win, and tab-independent
   work receives no invented tab argument. Plural controlled tabs remain supported.
4. Capability matrices use real configured grants. Draft writes, credentials, frames, screenshots,
   scripts, recordings, controls, history, and all-open browser work retain their existing coverage.
5. An audit line written before audit v2, carrying the retired fields, still reads and shows in
   history, and the retired fields are never written back (`audit/record_tests.rs`). Cached
   clients receive catalog-refresh guidance without automatic replay.
