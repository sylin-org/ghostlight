# ChatGPT browser extension package study

Date: 2026-09-12. Status: clean-room research input; no product decision.

## Question

The owner observed that OpenAI's embedded Codex browser control retained edits on a React form
where Ghostlight's prototype setter plus generic synthetic events produced an unsaved indicator
and was later reconciled to empty. The owner then requested inspection of the downloadable ChatGPT
browser extension for mature browser-control practices.

This record separates package facts, observable behavior, and inference. No package source or
derived code is copied into Ghostlight. ADR-0050 still defines the official Claude-in-Chrome
extension as the sole implementation reference. Promoting another package to implementation
authority requires a new decision.

## Provenance

The official OpenAI documentation identifies a ChatGPT browser extension used by ChatGPT Work and
Codex for signed-in Chrome control:

- `https://learn.chatgpt.com/docs/chrome-extension`
- `https://chromewebstore.google.com/detail/chatgpt/hehggadaopoacecdllhhajmbjkdcmajg`

The Chrome Web Store page identified publisher OpenAI, extension id
`hehggadaopoacecdllhhajmbjkdcmajg`, and version `1.26.901.11451`. The exact CRX3 downloaded from
Google's extension update service was 21,502,271 bytes with SHA-256
`688d5c0a8141c9bee394d3738d4a177b448713c2fa9c29b5af1815bfddae46e1`. It contained 1,635 files
after extraction. The CRX and extracted proprietary package remain in an operating-system
temporary analysis directory outside the repository and are not project artifacts.

The manifest declares one MV3 service worker, a ChatGPT-only website content script, a main-world
media-permission script, a side panel, all-URL host access, native messaging, debugger, tabs,
tab-groups, web-navigation, downloads, history, sessions, storage, notifications, bookmarks,
favicons, context menus, alarms, and declarative request-header permissions. These are package
facts, not Ghostlight permission recommendations.

## Form input boundary

The package declares closed commands for element-index `set_value`, free `type_text`, and a
Playwright-style locator fill carrying a selector, value, and replacement boolean. Its local API
maps both `fill` and `type` to that locator-fill command and invalidates cached locator state after
completion.

The package contains no implementation of ordinary value assignment, `Input.insertText`,
`Input.dispatchKeyEvent`, or a framework setter bridge. The extension sends the locator command to
the native OpenAI runtime. Separately, the native runtime can send arbitrary CDP commands through
the extension's debugger relay. The exact ordinary-fill algorithm therefore cannot be harvested
from the extension package.

The live affected form supplies behavior evidence: OpenAI's embedded browser control retained the
same class of form edit, while Ghostlight's synthetic setter path did not. That result agrees with
Ghostlight's independent keyboard A/B test and supports the native-edit correction. It does not
prove which OpenAI host-side CDP or Playwright sequence produced the retained value.

## Practices worth evaluating

These package techniques are mature enough to test against Ghostlight's existing contracts. They
are candidates for evaluation, not automatic imports.

1. **Session, turn, and tab leases.** Tabs carry session id, turn id, agent-versus-user origin,
   active-versus-handoff state, an extension instance id, optional viewport state, and an explicit
   terminal mark. Every browser mutation checks current ownership. Tab replacement, navigation,
   close, debugger detach, turn completion, handoff, and stale recovery converge on the same lease
   records.
2. **Serialized lifecycle changes.** One lifecycle queue orders claims, releases, handoffs,
   viewport changes, and turn transitions. Storage mutation also uses one queue and restores its
   prior in-memory state if persistence or derived rule synchronization fails.
3. **Bounded debugger commands.** CDP dispatch requires an attached, owned tab. Commands have a
   finite timeout, and a timeout detaches the tab unless the caller explicitly preserves the
   debugger for that operation. Detach events and child-target attachment have dedicated cleanup.
4. **Correlated native requests.** Native requests use JSON-RPC ids, one pending-request map,
   optional per-request deadlines, exact response settlement, and rejection of every pending
   request on disconnect or forced reconnect.
5. **Reconnect that survives MV3 suspension.** A short in-memory timer and a Chrome alarm both
   schedule the same bounded reconnect attempt. An exact missing-host error can select a known
   fallback host; unrelated failures do not silently rotate identity.
6. **Idempotent document adapters.** Each content-script instance announces its identity and
   aborts older copies. Timers, animation frames, listeners, navigation watchers, and remount logic
   share one abort scope. Page-show and DOM replacement repair the content-free overlay.
7. **Physical state restoration.** Tab favicons, managed groups, viewport overrides, request-header
   rules, attached debuggers, and injected document state each have explicit release or restore
   paths rather than depending on process exit.
8. **Closed input schemas.** Browser actions are discriminated command variants with bounded
   integer indexes and enumerated buttons, directions, and selection modes. Browser, tab, selector,
   value, replacement, and timeout fields remain explicit at the boundary.

## Practices that do not transfer directly

The package can add an `x-browser-agent` request header through session DNR rules, uses OpenAI
network and feature-flag endpoints, blocks frames owned by other browser extensions, and requests
permissions for product features outside Ghostlight's scope. Ghostlight's no-phone-home rule,
governance model, visible presentation, and all-open-first posture remain authoritative. Any
equivalent needs its own concrete user promise and threat-model evidence.

The package also combines side-chat product UI with the physical bridge. Ghostlight's stable relay,
orchestrator-owned language, and policy-free adapter are intentional boundaries and should not be
collapsed to imitate package layout.

## Next evaluation slice

A repeatable behavior matrix should compare OpenAI browser control and Ghostlight on ordinary
input, textarea, number, select, checkbox, contenteditable, shadow DOM, iframe, focus mutation,
selection replacement, undo, composition, delayed framework render, navigation, disconnect, and
partial batch failure. Capture event trust and order, DOM/model retention, submission count, and
effect certainty. Do not retain form contents or authenticated page data.

Capturing the exact host-side fill mechanism would require an approved black-box CDP command trace
or a separately authorized study of the native runtime. The downloadable extension alone cannot
answer it.
