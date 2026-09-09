# Alpine native-musl fleet acceptance -- test-03

Starting source: `a8cfd0337d83079f5fad3d5dfec846c31ba3b8fd` (service 1.3.5,
source adapter 1.1.2). Local branch: `codex/fleet-test-03`.
This record is native-musl feasibility and installed development acceptance,
not a declaration of Alpine support or a release attestation. The initial
`main` checkout at `48ef29e` was left unchanged.

## Machine and delivery boundary

- Alpine Linux 3.24.1, x86_64, musl 1.2.6-r2, kernel 6.18.48-0-lts.
- Host PID 1 is init; OpenRC, real KDE Plasma 6.6.6 Wayland session.
  The initial tool container boundary was identified before host acceptance.
- No glibc loader or compatibility layer was installed.
- Root package mutation is blocked: sudo and doas require authentication.
- Chromium 152.0.7977.82 came from signature-verified Alpine Chromium,
  chromium-common and crc32c APKs, extracted into a user-owned directory.
  This is a native musl executable and a real desktop browser, not an APK
  database installation. It uses its ordinary default profile, normal sandbox,
  Wayland, and a user Applications entry. The source extension is explicitly
  loaded unpacked; store-extension acceptance is not claimed.

The published `npx -y ghostlight@1.3.4 install` downloaded and verified all three
siblings, then failed with spawn ENOENT. The files exist, but their ELF interpreter
is `/lib64/ld-linux-x86-64.so.2`, which is absent. This is an unsupported ABI,
with an unhelpful consumer error. No published install success is claimed.

The default native musl build reached the authority link and failed because
static GTK/GLib/intl libraries are absent. A native dynamic build passed:

```sh
RUSTFLAGS='-C target-feature=-crt-static' cargo build --workspace --locked \
  --target x86_64-unknown-linux-musl --target-dir <isolated-target>
```

Rust is 1.95.0, host `x86_64-unknown-linux-musl`. The authority uses
`/lib/ld-musl-x86_64.so.1`. These are unoptimized, unstripped debug binaries.
All three siblings live in one explicit installed user directory. Deliberate
`ghostlight install` created the actual native registration, command,
Applications entry and Codex registration. Connector demand-start worked on OpenRC
without a system service. No machine-local notes or private project material were read.

## Results before reboot

| Boundary | Result | Evidence file or directory |
| --- | --- | --- |
| Candidate native build, fmt, Clippy, Rust | PASS; 536 Rust tests before fix | `native-*`, `candidate-format.log` |
| Candidate extension, workbench, npm, policy grammar | PASS; 210 extension tests | `candidate-*.log` |
| Installed full browser journey | PASS; 23 tools, 15 check groups | `installed-live-journey.json` |
| Native workbench Open/minimize/Open/close/Open/concurrent Open | PASS; exact KWin window/PID evidence | `desktop-lifecycle.json` |
| Human navigation, overlapping wait, child tabs, moved tabs | PASS; no passive audit/hold, no child grouping/debugger adoption, no regrouping; next agent read governed | `human-boundary.json` |
| Real popup pause/resume/stop/new-session handlers | PASS | `human-boundary.json` |
| Installed registration and connector/authority/worker recovery | PASS on rerun; original failure retained | `recovery-diagnostic/`, `recovery-fixed/` |
| Interrupted browser effect | PASS; effect count 1, outcome unavailable, no replay | recovery results above |
| Full normal-browser exit and Applications relaunch | PASS; stable adapter identity, unchanged authority | `browser-restart.json` |
| Repeated real uninstall/reinstall | PASS; audit byte preservation, owned manifest restored exactly, unrelated sentinel retained | `install-lifecycle.json` |
| Two real Chromium profiles and workspace isolation | PASS; absent profile does not switch into survivor | `plural-profiles.json` |
| No-tray private D-Bus session with actual KDE windows | PASS for close/Open; supplemental context | `no-tray-result.json` |
| Useful WebView accessibility tree | NOT ESTABLISHED; application/frame visible only | `no-tray-result.json` |
| Signed managed authority in isolated native browser | PASS; seven checks including fail-closed, cache and rollback | `managed-policy.json` |
| Actual Codex application browser call | BLOCKED by client's enforced approval policy | `actual-codex-*` |
| Actual OpenCode 1.18.30 with opencode/big-pickle | FAIL before schema fix; PASS browser read/fill/click/read after fix | `opencode-*-events.jsonl`, `opencode-dom-proof.json` |
| Actual MCP Inspector | Catalog passed; call FAIL before schema fix; PASS afterward | `inspector-*.json` |
| Native portable debug prototype | PASS member-byte verification; no APK install or provenance | `native-package.json` |
| Reboot | Pending at this checkpoint | `reboot-*.json` when completed |

Synthetic boundary drivers exercise the actual installed MCP connector, authority,
registered native host, extension and visible browser. They are not counted as
real AI-client acceptance. External browser manipulation uses the browser APIs;
this is deterministic human-boundary coverage, not a physical human participant.
Managed-policy and no-tray cases use explicitly isolated contexts; neither proves
the normal user's managed deployment or a different desktop shell.

OpenCode initially sent `new_tab` as the string `True` three times. These inputs
were refused before effects. It recovered by omitting that optional field and
completed the requested task. Independent DOM inspection found the synthetic
`OPENCODE-MUSL-PASS` draft and count `1`. Do not label this a first-attempt model pass.

## Local fixes

### Output schema must include the actual storage receipt

Both MCP Inspector and OpenCode rejected otherwise successful actual installed
results with `data must NOT have additional properties`. The closed output schema
omitted `history_storage`, which `InvocationResult` always serializes.

The fix adds the existing `Storage` enum to the orchestrator-owned common output
schema and required fields. The active language contract now documents it.
It keeps `additionalProperties: false`; no client-specific workaround, connector
change, extension change, or permission change was introduced. The Rust regression
compares actual serialized result fields with every advertised output schema and
covers both saved and unconfirmed storage. Both real clients accept the deployed fix.

Validation: formatting, warnings-denied native-musl Clippy, all 537 Rust tests,
210 extension tests, eight driver fault tests, JavaScript syntax and workbench
surface pass. The authority alone was deployed; connector and adapter bytes remain
unchanged. PowerShell 7.6.6's official native-musl archive was digest-verified.
A local copy of the existing dev-loop script adapts its repository/install roots
and explicit musl target path for this campaign; deployment lock, exact-image
process selection, bounded copy and normal startup are retained.

### Recovery reporter must preserve a protocol failure

The first installed recovery run passed registration and native-host crash phases,
then its reporter threw a TypeError in the authority-restart phase. JavaScript's
regex test coerced undefined to the matching string `undefined`, then the true
branch dereferenced absent facts. The underlying transient reply was lost; its
cause remains unresolved and must not be claimed fixed by the reporter correction.

Commit `98b7e93` adds an explicit string guard and records only a numeric protocol
error code. Injected missing-result and protocol-error regressions exercise the
actual reporter without starting processes. Private error text is not retained
in the report. The normal five-phase recovery journey passes after the fix.
The prior diagnostic rerun also passed; neither pass erases the initial failure.

## Artifact identities and evidence custody

Baseline candidate binary SHA-256:

| Binary | SHA-256 |
| --- | --- |
| authority | `03114683d1f422d066477aa8ddff56deafe308a31a59dacaacc12f5b0a1ea4e6` |
| browser connector | `4a148d448f08f68bfe4c7f44dc2074409efc7577b8dd84e81f8fcb3a70bfc2ed` |
| MCP connector | `fe77f4cfcc30e7aad28898d9e1b7dac13ed23861e07470a93d2a8e6bcb55177d` |

The fixed authority is
`f05fbeac3eff53d2e72229ea3b8a834f9352ff3d4e051a365986a9a38a369510`.
The two connectors retain their baseline hashes. Source adapter hashes are in
`source-adapter-sha256.json`. The baseline native debug archive is
`84098468fbe8aa93e1f52ac2e7e97d6ec80f098e0f6927385cf68d9193547c7c`.
A fixed-source archive must name its resulting commit separately.

Machine evidence root: `/home/test/ghostlight-fleet/test-03/`. Logs and reports are
under `evidence/`; recovery outputs are in their named sibling directories.
Only sanitized reports belong in transfer bundles. Runtime documents, browser
profiles, generated signing seeds, and Git credentials must never be bundled.

Failed fixture attempts are retained: stale APK mirror downloads, missing native
browser library path, numeric rather than string wait input, and managed-context
home masking of prerequisites. The managed rerun used exact copied source assets
and native browser bytes outside that masked home. It did not modify host policy.

Remaining lane limits: root APK/package-manager lifecycle, a second Chromium brand,
store adapter, useful WebView accessibility, Codex's enforced approval block,
three distinct real AI applications, and reliable MCP-led cold-start. Published GNU binaries
cannot establish Alpine upgrades/downgrades. The native debug prototype is not a
new supported release channel. Broader fleet and release approval remain separate.

## Fixed artifact and reboot checkpoint

The schema repair is commit `578e8038bc10552d5477d76ef69d71a1c8299752`.
Both signed-off fixes are pushed only to `codex/fleet-test-03`. No shared branch,
release, store or package publication was changed.

The fixed-source native archive is
`ghostlight-v1.3.5-x86_64-unknown-linux-musl-debug-prototype-578e8038.tar.gz`,
94,062,113 bytes, SHA-256
`f3fb5476064c0b48342e51211ec8a824b94da57a1cad0a1b90f243a1c024fea8`.
All seven members verify against the staged bytes, including the two license
files and build metadata. Its extracted binaries pass version and repeated
install/uninstall under isolated XDG roots without starting an authority.

Logind reports that this user can reboot. Pre-reboot boot ID is
`f741e580-6fd6-4b27-8473-abc193846675`. A task heartbeat is prepared to resume
post-boot verification. The readable SDDM configuration has no Autologin section;
after reboot, an interactive login may be required. This checkpoint does not
claim reboot recovery. Primary AT-SPI traversal also did not establish a useful
WebView tree; preserve that limitation alongside the supplemental no-tray result.


## Post-reboot result: browser recovery passes, MCP cold-start fails

The owner completed reboot. The new boot ID is
`6ccc22b2-bfdd-4555-a4b7-25e8bfdc6646`, different from the checkpoint above.
The actual KDE login session was present. Before intervention, Ghostlight was not
Ready and MCP-led demand-start repeatedly created authorities that did not stay
running. One process snapshot contained 54 authority processes.

Launching the normal Chromium Applications entry let the registered native host
start one installed authority. The adapter retained
`browser_fec1a57a95d64265a452c5569c428714`; all three binary hashes were unchanged.
A fresh installed MCP browser read returned `ALPINE-MUSL-135`, and explicit Open
produced the native Ghostlight workbench. This is a browser-led recovery pass,
not an overall cold-start pass. See `reboot-result.json` and `reboot-proof.log`.

A controlled repeat closed Chromium normally and stopped only the exact installed
authority through the deployment-lock dev loop. Actual MCP Inspector then called
`policy_explain` through the installed connector without first starting a browser.
It failed with `Connection timed out after 15000 ms`. Logs show repeated authority
launches and transient runtime publication. See `reboot-mcp-cold-prepare.log`,
`reboot-mcp-cold-stderr.log`, and `reboot-diagnostics-final.txt`.

The observed MCP connector lacked DISPLAY, WAYLAND_DISPLAY, XDG_RUNTIME_DIR,
DBUS_SESSION_BUS_ADDRESS and GDK_BACKEND. The working browser-led authority had
them. The installed MCP SDK's `client/stdio.js` defaults on Linux to HOME,
LOGNAME, PATH, SHELL, TERM and USER; its spawn merges only those with explicit
server environment. Thus this independent client reproduces the failure without
requiring a Codex tool call or changing its approval policy.

An exact installed-authority launch with desktop variables absent and an isolated
runtime discovery path confirmed `Failed to initialize GTK`, exit code 1 after
15.41 seconds, and runtime cleanup. This is supplemental diagnosis, not installed
acceptance. See `reboot-sanitized-desktop-result.json`.

The owning path is shared bridge lifecycle demand-start plus orchestrator desktop
initialization. The bridge inherits the caller environment and discards child
stderr. The authority publishes its service runtime before GTK initialization;
failed desktop startup releases the service lease, then the activation fallback
waits up to 15 seconds. Connector retries can create many such waiting processes.
ADR-0127 correctly prevents an invisible surviving authority. This record does
not change that decision or add a login supervisor, headless mode, session-address
hardcoding, or private parent-environment scraping. A reliable desktop-session
launch mechanism for sanitized MCP clients remains an unresolved platform defect.

Chromium was relaunched normally after the failed cold-start test; the installed
service is Ready again. Process diagnostics were turned off after evidence capture.
The final doctor also reports the user command entry absent; the direct installed
binary, Applications entry and native registration are available. The timing of
that command-entry loss is not established by this reboot check. The reboot
heartbeat is paused. No product code or installed binary was changed post-reboot.


## Post-reboot local Git authentication verification

Verified from local Git in this worktree, not through a connector account:
`git credential fill` used the repository-scoped helper backed by KDE Secret
Service. GitHub authenticated that credential as `lbotinelly`, and the repository
API confirmed push permission for `sylin-org/ghostlight`. The local commit identity
is `Leo Botinelly <leonardo.botinelly@gmail.com>`. An authenticated
`git push --dry-run origin HEAD:refs/heads/codex/fleet-test-03` exited 0.
`credential.useHttpPath` is true. No new credential intake key is needed.

The helper is `/home/test/.local/libexec/git-credential-ghostlight-wallet`.
Only the sanitized verification is retained in `auth-post-reboot.json`; no token,
private key, credential output or plaintext credential material was printed or
added to the report. The previously authorized local Git setup survives reboot.
No combined-candidate rebuild or release action was needed for this check.

The post-reboot documentation commit passes formatting, warnings-denied native
dynamic-musl Clippy, the Rust workspace tests and all 210 extension tests.
No extension JavaScript changed. Installed binaries remain the prior fixed
artifact; this report does not imply a new combined-candidate build.


## Procured cold-start repair and installed proof

The owner authorized procuring a fix after the failed reboot checkpoint. The
integrated fleet already contained the owning installer repair, commits
`4af4988522a17f7b733c9f34b0d400fa789ced95` and
`f51130d08c0c9ec8c94cd06aa8a2220ed2e0f350`. Their complete current
`crates/orchestrator/src/install/mod.rs` was brought into this Alpine lane from
integration `0ff1d6c5f4b0cce6befc98091c9114546f3d7e49`. No other fleet runtime
changes were imported. This is a targeted backport, not combined-candidate acceptance.

The repair belongs to Ghostlight's existing Codex registration writer. It adds
DISPLAY, WAYLAND_DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS,
XDG_CURRENT_DESKTOP and XAUTHORITY to Linux `env_vars`, preserving custom fields
and explicit settings. Missing forwarding makes an owned registration updatable;
malformed forwarding members remain untouched. Manual and automatic setup share
the same helper. Windows receives no Linux fields. These are names resolved by
Codex when launching, not saved session addresses. The
[official MCP documentation](https://learn.chatgpt.com/docs/extend/mcp) documents
this forwarding contract. ADR-0117, ADR-0125 and ADR-0127 remain unchanged.

Native build and the orchestrator-only development-loop deployment passed.
The installed authority SHA-256 is now
`04896d7547c79f74ca701ba8fe938a477fd48bb29e3eaa01a1257ca987cd0284`.
Both connector hashes and the source adapter are unchanged. The actual installed
`ghostlight install --client codex --browser chromium --no-open` updated the
user's saved registration. `codex mcp get ghostlight --json` confirms the six
names and exact installed connector command. Repeating installation preserved
that same effective registration. The missing user command entry is restored.

The original trigger now works on the normal desktop:

- With Chromium closed, the deployment-lock dev loop stopped authority 23517
  without starting a replacement. The existing real Codex app's connector 24426
  automatically started authority 26727, with all six desktop variables present.
  The app picked up its saved registration without an application restart.
- A fresh actual Codex CLI 0.153.4 invocation using the saved configuration
  completed exactly one `policy_explain` call with status succeeded and effect
  none. It connected to that already recovered authority, so the CLI result is
  not separately labeled cold-start. No approval settings were overridden.
- Actual MCP Inspector then started from a verified absent authority and absent
  browser connector. Existing exact-path MCP connectors were briefly SIGSTOP'ed
  to prevent them winning the cold-start race, then resumed in a finally block.
  Inspector forwarded the same six current variable values through its `-e`
  options, started authority 27991 through connector 27986, and completed
  `policy_explain` in 1.17 seconds with status succeeded and effect none.
- Normal Applications Chromium reconnected to that same MCP-started authority.
  Its adapter identity remained unchanged, a fresh installed browser read returned
  `ALPINE-MUSL-135`, and native Open produced one active, unminimized workbench.

The first configured Inspector attempt placed options before the target command;
this version's parser treated it as a catalog invocation and reported no servers.
It never tested Ghostlight. Its logs are retained as
`cold-start-fix-inspector-argument-order-*`; the corrected invocation is separate.

Evidence: `cold-start-fix-registration.json`, `cold-start-fix-codex-prepare.log`,
`cold-start-fix-codex-processes.json`, `cold-start-fix-codex-call-summary.json`,
`cold-start-fix-inspector-summary.json`, and `cold-start-fix-live-result.json`.
Formatting, warnings-denied native-musl Clippy, all 542 Rust tests and 210 extension
tests pass. No extension JavaScript changed. Installer regressions cover migration,
manual output, explicit/custom settings, malformed members and idempotency.

The installed Codex startup defect is resolved. A deliberately unconfigured MCP
SDK/Inspector launch still omits desktop context; such callers must forward it.
No claim is made that an arbitrary sanitized caller can discover a desktop or
that the service runs without one. Failed-start retry amplification outside a
correctly configured desktop launch is not changed by this repair. The earlier
unconfigured failure evidence remains valid. No second physical reboot was run
for this repair; cold-start, active-client recovery and browser rejoin were tested.
The service is Ready with its native workbench. Prior prototype archives retain
their recorded older bytes; no public release or support declaration was changed.
