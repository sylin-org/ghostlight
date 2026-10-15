# Break 2.0: shell milestone 1 ledger

Milestone 1 (M1) of the desktop shell track carries the 2026-10-15 amendment of
[ADR-0199](../../adr/0199-one-service-thin-shells.md): the Ghostlight web apps over loopback.
Slices m1-01 to m1-12 replace the private scheme and the page bridge with the existing workbench
served on a loopback port, with access decided in context, one live stream and a shared Controls
sheet. Tauri stays inside the service for the whole milestone and its window keeps working. The
milestone ends with the coordinator's live demo with the owner, in three parts: the workbench in
the Ghostlight window, the same URL watching in a Chrome tab, and watching turned off with the
About page still showing in full. The rest of the shell track is planned after that demo.

Gate, from the repo root on `dev`, in Git Bash, with
`CARGO_TARGET_DIR=E:/repo/github/sylin-org/ghostlight/.target-lane-shell-m1`:

- `cargo fmt --check -p ghostlight-service`, plus `-p ghostlight-desktop` when `crates/desktop`
  changed and `-p ghostlight` when `crates/edge` changed (fix only with `cargo fmt -p <package>`,
  never a bare `cargo fmt`).
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
- When anything under `crates/orchestrator/ui`, `tests/workbench-*.mjs` or
  `tests/workbench-preview-server.mjs` changed: `node --test tests/workbench-transport.test.mjs`
  (from m1-07), `node --test tests/workbench-controls.test.mjs` (from m1-08) and
  `node tests/workbench-surface.mjs`.
- `node --test "crates/desktop/guards/*.test.mjs"` when `crates/desktop`, any `Cargo.toml` or
  `Cargo.lock` changed.
- `node --test "tests/guards/*.test.mjs"` when `crates/edge`, any `Cargo.toml` or `Cargo.lock`
  changed.
- `node --check` on every changed `.js` and `.mjs` file.
- `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`.

Never pass `--locked` to cargo build, clippy or test.

Rules:

1. The commit protocol is in [BOOTSTRAP.md](BOOTSTRAP.md).
2. One slice is one commit. Stage and commit only your own paths.
3. Never run a Ghostlight binary, a journey, an install, a carrier or `scripts/dev-loop.ps1`.
   The coordinator runs every live check; record each under Owed live checks.
4. Follow the tree when it differs from a spec, keep the spec's intent and record the difference
   under Notes. When the intent cannot be kept, record a blocker and stop.

## RESUME HERE

Next: m1-05-live-stream.

## Landed

| Slice | What | Commit |
|---|---|---|
| m1-01-adr-amendment | The owner-approved amendment of 2026-10-15 is appended to ADR-0199 (the Ghostlight web apps over loopback), with its README row, this ledger, and the pointers from the shell-spike ledger and the task index. Docs only. | see `git log` |
| m1-02-harness-evidence | Harness evidence names a foreign program and its argument count, never its arguments; a configuration that cannot be parsed carries no evidence (owner answers Q3 and Q8). | see `git log` |
| m1-03-own-origin | Agents can never open or read the workbench's own loopback origin: `GovernanceFacade::protect_workbench_port` shares the port with every snapshot, and the landing decision refuses a loopback address on that port with `ProtectedHost` whatever the policy says. The authority id does not depend on the port. | see `git log` |
| m1-04-loopback-server | The service serves the workbench on 127.0.0.1 with access decided per route in three contexts (Control, Watch, Closed), each part projected for its audience, guards on Host, Origin and framing, and the bearer made at each start. `protect_workbench_port` runs between binding and answering. The Tauri window is unchanged. | see `git log` |

## Route table

Server: `S/workbench/web/routes.rs`. Pages answer every context (a wrong bearer is 401 even there).
Control carries the bearer, Watch has none while watching is on, Closed has none while it is off.
A watcher on a route marked "window" gets 401 `needs_window`; Closed gets 403 `watching_off` on
every route but the pages and `/api/context`.

| Path | Method | Control | Watch | Closed | Watch limit | Replaces |
|---|---|---|---|---|---|---|
| `/` and each file under `ui/` | GET, HEAD | page | page | page | none | (the private scheme) |
| `/api/context` | GET | full | projected | product only | none | (new; Tauri command in m1-07) |
| `/api/snapshot?parts=` | GET | all parts | projected | 403 | none | `workbench_snapshot` |
| `/api/policy` | GET | full, with etag | projected, with etag | 403 | none | `workbench_policy` |
| `/api/search` | POST | yes | yes | 403 | 5 per second | `workbench_search` |
| `/api/policy/preview` | POST | yes | yes | 403 | 1 in flight, 10 per minute | `preview_user_policy` |
| `/api/integrations/rescan` | POST | scans | cached roster, one scan per 5 s | 403 | coalesced | `refresh_harnesses` |
| `/api/window` | POST | yes | yes | 403 | 1 per 5 s | (new; `ghostlight open`) |
| `/api/runtime` | POST | any intent | `hold` only | 403 | 6 per minute | `apply_runtime_intent` |
| `/api/policy/user` | PUT | with `If-Match` | window | 403 | none | `apply_user_policy` |
| `/api/policy/user` | DELETE | with `If-Match` | window | 403 | none | `remove_user_policy` |
| `/api/controls/diagnostics` | PUT | set | window | 403 | none | `toggle_diagnostics` |
| `/api/integrations/setup-all` | POST | yes | window | 403 | none | `setup_detected_harnesses` |
| `/api/integrations/act` | POST | with `expected_state` | window | 403 | none | `manage_harness` |
| `/api/notifications/test` | POST | yes | window | 403 | none | `test_notification` |

Not routed, by design: `quit_ghostlight`, `copy_harness_text` (m1-07 copies in the page),
`reveal_diagnostics`, `locate_harness`, `open_harness_download`, `open_destination` (m1-06) and the
event stream (m1-05).

Bodies are JSON objects with exactly the named fields. Answers keep today's Tauri values: search
answers the array of hits, a preview answers the preview, an intent answers the intent result, a
rescan answers the projected roster, a client act answers the projected `HarnessActionResult`.
Errors are `{ "error": { "code", "message" } }`; what a facade call refuses with is 422
`refused` and its text.

## Owed live checks

- m1-04, Windows and Linux: the About card's two orbs, whose colors are inline style attributes the
  new content policy blocks, look the same in the Tauri window and in a browser tab on the new
  address. Today the page in a tab shows "Not connected" until m1-07; open the About page in a tab
  on the logged address (`RUNTIME_PUBLISHED` carries `workbench=<url>`) and compare it with the
  window.
- m1-04, Windows: a service stopped and started again at once takes the same port (the unit test
  covers TIME_WAIT on loopback; confirm with a real restart).

## Blockers

None.

## Notes

- Owner approval: the owner approved the amendment text on 2026-10-15, as written.
- Precondition 1: `seams.md` has Landed rows for f08a-edge, f08b-console-attach and
  w01-windows-user-path, and its RESUME HERE names none of them as next.
- Precondition 2: no tracked file under `crates/`, `Cargo.toml` or `Cargo.lock` was modified or
  staged; `install/user_path.rs` and `win-peer/src/settings_change.rs` are tracked.
- Precondition 3: the orchestrator package is `ghostlight-service`; `relay::home` has
  `SERVICE_EXECUTABLE_NAME` and `EDGE_EXECUTABLE_NAME`; `install/deployment.rs` has `service_swap`,
  `edge_swap` and `placed_in_bin`; `intake/listener.rs` `judge` refuses `Role::Desktop` and
  `service/mod.rs` refuses it again; `desktop/mod.rs` still has its `tauri::generate_handler!`
  list. No name differs from the spec.

- m1-02: the foreign entry's `program` is the file name of `command` when `command` names an existing file; otherwise the command is cut at its first whitespace before a file name is taken, and every later word counts as an argument. A configuration that cannot be parsed names the line of the parse error (taken from the parser, never any file text) in its detail and carries no evidence (owner answer Q3); `HarnessError::Malformed` carries only that line.

- m1-04: the server is `S/workbench/web` (`mod.rs`, `http.rs`, `routes.rs`, `assets.rs`,
  `limits.rs`, `watching.rs`, `tests.rs`); the parts and their projections are
  `S/workbench/parts.rs` (tests in `parts/tests.rs`); the contexts and acts are
  `S/workbench/context.rs`; the shared input bounds moved to `S/workbench/input.rs`; the words are
  `S/language/workbench.rs`. The Tauri commands call the moved validators and the two copy
  confirmations, so the Tauri window says what it said.
- m1-04: the bind is two steps (`PendingWorkbench::bind`, then `serve`) so `ServiceHost::publish_ready`
  can call `protect_workbench_port` between binding and answering. `WorkbenchServer::start` does both
  for callers with no rule to put between them. The `Intake` holds the running server in a
  `OnceLock` (`workbench_server`); the desktop role (m1-09) and `serve_control` (m1-11) read the
  address and the bearer from it.
- m1-04: the server builds `/api/context` through `WorkbenchFacade::context_document`, as the spec says.
  The facade keeps the first `attach_web` it is given, so a facade is served by one server.
- m1-04: scope-down, as the spec allows: `POST /api/runtime` has no `If-Match`. Hold, resume, start
  and end are sets that converge, and the window's view of the runtime is pushed within a second (m1-05).
- m1-04: search reads only fields a watcher is shown (operations, sessions, browsers, history
  status and reason, diagnostics cards, client names and details, the runtime word), so no hit needs
  filtering for a watcher. A test pins that an excluded host appears in no hit.
- m1-04: ceilings. The policy's ceiling lines that name the person's never-touch sites are found by
  the hosts in the `content.security.sacred_domains` settings of the layers; a watcher gets one
  count line (`never_touch_count`) in their place. `effective.rs` is not this lane's, so its line
  format is not touched.
- m1-04: the policy etag hashes the user layer's document as the authority reports it, which is the
  document re-serialized by the policy parser, parsed again without its top-level `version` and
  written with sorted keys. A change of formatting or version keeps the tag.
- m1-04: `GovernanceFacade::owning_user_policy` (test-only) became `pub(crate)` so the server tests can
  give a facade a user policy file of their own; `HarnessRegistry::isolated` and
  `WorkbenchFacade::with_harnesses` (test-only) keep the tests off this user's client configuration.
- m1-04: the default port of a home is 41000 plus the FNV-1a hash of the lower-cased home text
  modulo 1000, and 41738 when that is 41737. Windows' std binds without `SO_REUSEADDR`; the restart
  test passes there, so TIME_WAIT on the server's port does not block binding.
- m1-04: docs/trust/data-flows.md says `ghostlight status` shows the port, as the spec's sentence
  does; `status` shows it from m1-11.
