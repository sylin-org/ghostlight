# In-service UX source record -- 2026-10-01

Final bounded native results and selected screenshots are in the
[acceptance record](in-service-ux-acceptance-2026-10-01.md). The initial handoff and subsequent
review correction below describe their own validation points; later evidence supersedes pending rows.

Branch: `codex/in-service-outcome-ux`.
Baseline: clean `22d27bd175fded4b66a731fb6b77d9fc433b8d2c` on `codex/quiet-coexistence-trial`.
Initial source handoff: `8d1e16ff210f0b30dd944b520b54f713dd145147`.
Scope: Leo's authorized holistic local improvement trial. No push, release, deployment, or outreach.
Decision: [ADR-0187](../adr/0187-readable-workbench-outcomes-and-reconnection-controls.md).

## Before and after

| Observed baseline | Source refinement |
| --- | --- |
| Unknown script effects lost recovery facts in human history. | The typed language projection retains safe suggestions and final repeat safety; the human surface shows observation-before-unfinished-work guidance. |
| Newer refusals made earlier uncertainty look ordinary. | Historical uncertain/partial rows keep explicit recovery guidance; their original child receipts and expandable details remain intact. |
| Disconnected browsers disabled Pause. | A reachable authority enables Pause/Resume/Start session independently of browser connectivity. Lost authority disables them. Global scope and reconnect delivery are stated. |
| Deliberate human holds led with tool name, raw reason, and BLOCKED. | The human outcome leads. No-effect Pause/Stop is calm; policy refusal, failure, and uncertainty have separate language-owned tones. Exact machine status remains unchanged. |
| Show tab wording relied on a tooltip/toast. | Local authored guidance distinguishes looking from takeover and preserves exact workspace/tab targeting. |

The default hero tool heading, duplicate capability label, raw reason paragraph, and duplicate
protocol-status metadata were retired. Technical facts remain in collapsed details. There is
one existing work view and history surface, with no new destination or approval ritual.

## Preserved

The complete guardian About card, lantern pixel art, dark frame, cyan divider/diamond, circular
version medallion, typography, ability icons, flavor text, local facts, and help links remain.
The About markup, asset, and About CSS rules have no changes. Existing palette and motion remain.
The language-owned machine directives, Stop semantics, no replay on Resume, policy, exact tab
ownership, composition child receipts, privacy boundary, and storage truth remain intact.
MCP connector, browser connector, shared bridge, extension, and Tangent are untouched.

Only language constructors supply retained suggestions; completion never copies client facts
or browser exception text. Older records can omit both new optional fields. Unknown repeat safety
is displayed as not recorded, and existing unknown/partial effect evidence still requires observation.
Final composition guidance is preserved rather than replaced with a whole-operation retry.

## Initial source validation

At the initial handoff, execution was source inspection, existing-image inspection, build, or noninteractive tests.
`CARGO_TARGET_DIR=.target-quiet-trial` reuses the existing isolated build output; no binary was started.

| Gate | Status/evidence |
| --- | --- |
| `cargo fmt --check` | Passed on the final source. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed; `.tmp/in-service-ux-clippy.log`. |
| `cargo test --workspace` | Passed: 592 tests, no failures. `.tmp/in-service-ux-rust.log`. |
| `npm test --prefix extension` | Passed: 349 tests, no failures. `.tmp/in-service-ux-extension.log`. |
| `node tests/workbench-surface.mjs` | Passed: 101 assertions including new outcome/recovery/control cases. `.tmp/in-service-ux-surface.log`. |
| Changed JavaScript syntax and `git diff --check` | Passed on the final source. |
| `cargo build -p ghostlight` | Passed without startup. `.tmp/in-service-ux-build.log`. |
| Real process, native Workbench, and browser journeys | Not run: desktop/browser lane is reserved to Tangent pending parent handoff. |

Regression coverage includes successful outcomes, deliberate Pause/Stop, policy refusal, failure,
unknown/partial effects, later refusal after uncertainty, legacy missing fields, preserved child
receipts, no payload leakage, applied action before a refused postcondition, disconnected runtime
control and Start/Resume semantics, authority loss/reconnection, and exact Show tab transport.
Existing Show-tab tests still prove held/stopped state and foreign/stale ownership protection.

Built authority: `.target-quiet-trial/debug/ghostlight.exe`, SHA-256
`0bf3c223505667aaebd6e6bf62694982957ab86e6c9be2ab5b77d94a3a2ca1c3`.
It was not started or deployed. Initial handoff `8d1e16ff` owns this artifact's source identity;
the rebuilt review correction below supersedes this hash.

The first compile attempts exposed missing fields/imports in new fixtures; those were corrected.
Strict Clippy caught a moved import; it now lives only in tests. A new disconnected-control test
exposed the existing empty-browser publication confirmation bug; the facade now uses connection
evidence as well as publication success. These failed attempts remain separate from final passes.

## Visual evidence and remaining lane

Inspected baseline images, all under `.tmp/`:

- `quiet-native-installed-2026-10-01T01-59-12-995Z-35616-ux-ordinary-filled.png`
- `quiet-native-installed-2026-10-01T02-08-13-229Z-51000-ux-uncertain-effect.png`
- `quiet-native-installed-2026-10-01T02-08-13-229Z-51000-ux-disconnected.png`
- `quiet-native-installed-2026-10-01T02-08-13-229Z-51000-ux-paused-show-tab.png`
- `quiet-native-installed-2026-10-01T02-08-13-229Z-51000-ux-about-card.png`

At this source handoff, no final-interface screenshot had been captured. The owed runtime lane was:
inspect the actual built Workbench: success, Pause, policy refusal, unknown/partial child effects,
newer refusal with earlier uncertainty, disconnected/reconnected authority, and Show/Resume/Stop.
Verify narrow width, large text, keyboard activation/focus retention, and full About parity.
Run the existing affected native/process/browser journeys with exact binary identity and explicit
`GHOSTLIGHT_BIN_DIR`; preserve installation, data, and registrations. Independent review is owned
by the parent. This source handoff does not claim installed or visual acceptance.

Exact-record targeting from history search is explicitly deferred. It does not naturally belong
to this outcome/control-readiness change and would add another interaction contract.

## Independent review correction

Review of clean `8d1e16ff` found one blocker. A topology sample could see the last adapter, then
that adapter could disconnect before publication. Successful publication to no remaining writer
is not proof that any browser received the state. Conditional confirmation could omit the
reconnect explanation despite that disconnect.

The correction takes the reviewer's simpler option: human confirmation states only the applied
authority effect. Pause/Stop/Attention describe their continued authority after browser reconnection;
Resume/Start describe new requests without replay. The confirmation accepts no browser-publication
argument. Existing best-effort metadata remains compatible and is explicitly not delivery evidence.
No acknowledgement or browser mechanism is added.

The deterministic regression loses the last adapter inside publication, after the facade's topology
sample, without threads, sleeps, or runtime startup. It proves that the sampled publication flag
can stay true despite no browser, while the human message remains truthful and the authority stays
Held. A connected publication produces the same message. Browser calls panic to catch any replay.
The facade now accepts its existing `BrowserPort` interface instead of a concrete relay type, so
this fixture executes the actual application method. Production still supplies the same relay.

Validation passed: the focused race regression, `cargo fmt --check`, strict workspace Clippy,
593 workspace Rust tests, 349 extension tests, and 101 executable Workbench assertions.
Logs are `.tmp/in-service-ux-race-{focused,clippy,rust,extension,surface}.log`.
The corrected authority builds without startup at `.target-quiet-trial/debug/ghostlight.exe`,
SHA-256 `10e1b8012bcedbf26046c2078c1bbd6a142779a5de896d659a8e79bc75b4c514`.
The first fixture compile revealed the concrete relay dependency; switching to the existing port
resolved that compile error. The final aggregate passes include the actual application race test.

At the review-correction handoff, desktop/browser, final visual, and installed journeys were pending.
About, layout, assets, other UX behavior, and optional search deferral are unchanged.

## Native observation correction

The actual background-attention refusal remained red because its capability decision was permitted.
The human projection now also examines closed retained refusal facts for browser-attention protection
and legacy request restrictions. A refusal is amber even when that separate permission check passed.
Unknown and partial effects keep their higher-priority caution presentation. Machine allowed/reason,
status, effect, and the authored sentence remain unchanged. A regression exercises both typed facts
with a permitted capability decision. Final native/source evidence is in the linked acceptance record.
