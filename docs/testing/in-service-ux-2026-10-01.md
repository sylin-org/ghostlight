# In-service UX source handoff -- 2026-10-01

Branch: `codex/in-service-outcome-ux`.
Baseline: clean `22d27bd175fded4b66a731fb6b77d9fc433b8d2c` on `codex/quiet-coexistence-trial`.
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

## Validation

All execution so far is source inspection, existing-image inspection, build, or noninteractive tests.
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
It has not been started or deployed. The commit containing this report owns the source identity.

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

No final-interface screenshot has been captured. After the parent hands off runtime access,
inspect the actual built Workbench: success, Pause, policy refusal, unknown/partial child effects,
newer refusal with earlier uncertainty, disconnected/reconnected authority, and Show/Resume/Stop.
Verify narrow width, large text, keyboard activation/focus retention, and full About parity.
Run the existing affected native/process/browser journeys with exact binary identity and explicit
`GHOSTLIGHT_BIN_DIR`; preserve installation, data, and registrations. Independent review is owned
by the parent. This source handoff does not claim installed or visual acceptance.

Exact-record targeting from history search is explicitly deferred. It does not naturally belong
to this outcome/control-readiness change and would add another interaction contract.
