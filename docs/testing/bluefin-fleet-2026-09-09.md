# Bluefin fleet evidence -- 2026-09-09

## Scope

Dedicated test-02, Bluefin 44.20260901 / Fedora 44 atomic, GNOME Wayland,
x86_64, glibc 2.43. Commands run on the host unless explicitly labelled Toolbox
or Flatpak. The original main checkout was clean and remains untouched.
Candidate base: a8cfd0337d83079f5fad3d5dfec846c31ba3b8fd, service 1.3.5,
source adapter 1.1.2. No adapter was loaded, so no browser compatibility claim.

Raw sanitized reports and command logs are retained on test-02 under
`/var/home/test/ghostlight-fleet-test-02/`. Its BASELINE.md records the state
before installation. RESULTS.md is the detailed index. No founder-private or
machine-local repository notes were read. The owner-authorized fleet branch push
is recorded below. No shared-branch change, PR, merge or release occurred.

## Immutable candidate results

- Public main shell installer failed on GitHub's checksum CDN redirect. The
  candidate installer downloaded and verified the public 1.3.4 siblings.
- Ordinary install refused the baseline: Firefox Flatpak only, no native
  supported browser. Host 1.3.4 execution needed no additional runtime libraries.
- A dedicated Fedora 44 Toolbox supplied build headers. Locked release build
  passed in 5m 57s. This does not prove an Ubuntu-baseline public artifact.
- 536 Rust tests and 210 extension tests passed. Formatting, release-profile
  all-target Clippy with warnings denied, installer-shell, workbench-surface,
  and the host isolated process journey passed. The latter uses a browser fixture,
  not installed Chromium or an actual MCP application's browser work.
- The documented dev loop deployed all three siblings into the test worktree.
  The host ran them. Explicit all-browser preparation, with no client changes,
  passed install/repeat/uninstall/reinstall and reproduced exact owned bytes.
  This forced preparation is not normal-browser installation acceptance.

Initial deployed SHA-256 values:

| Binary | SHA-256 |
| --- | --- |
| ghostlight | a999711a242101e8efa596f3c09e7ee86108bead4c3ed0e3ec1fb45ffe03d123 |
| ghostlight-mcp-connector | 301d579aa8b014eb05f2d2dd711273cb015de01cac6151a48766258e46f8a9b0 |
| ghostlight-browser-connector | 30f3b26e2db7224739b357612e7cab31766b830c25a316035a4ef72dc3121159 |

## Host, Toolbox and Flatpak boundaries

Native Chromium layering staged Bluefin 44.20260908 with three added packages.
The additive live apply refused package replacements. Explicit replacement then
failed with `Sender is not authorized to send message`. No permissions were
weakened and no reboot was performed; native Chromium is not active in this boot.

System Flathub Chromium 152.0.7977.82 was installed separately, commit
650cf951c98ab3be7c88d95a6591a947123c88e3791cdfd90ec144082bc47b01.
Ghostlight correctly identifies the Flatpak package and refuses normal setup.
Its shipped home-filesystem permission exposes the host connector and host
manifest. Firefox Flatpak exposes neither. Do not generalize file invisibility
across Flatpak applications.

Chromium's Flatpak can execute the public browser connector, which returns a
native-framed backend-unavailable response with closed input. The authority
cannot load `libwebkit2gtk-4.1.so.0` inside that runtime. Chromium also uses its
application-private XDG configuration root, where the host manifest is absent.
No permissions were changed and no native browser pipe was claimed from this
shell probe. The diagnostic sentence about being unable to start the connector
is broader than this observed failure; the native-browser remedy remains valid.

Toolbox and host see identical native-host manifest bytes. A successful Toolbox
build does not make a container-launched authority a host desktop acceptance test.
The first live orchestrator swap from Toolbox failed with `Text file busy`:
although PID namespaces matched, the container could not read the host authority's
executable path and PowerShell's process Path was null. Running the existing
deployment script on the host while forwarding only Cargo to Toolbox identified
the exact PID, stopped only it, and completed the swap. The failed attempt removed
its deploy lock normally. No name-based termination or permission change was used.

## Codex cold-start defect and local repair

Codex CLI 0.152.0 ran real MCP calls with ephemeral, per-invocation configuration;
the user's saved client configuration was not changed. Warm `policy_explain({})`
succeeded. Cold command/args-only setup failed to initialize within 15 seconds.
Inspection of the actual connector process confirmed that DISPLAY, WAYLAND_DISPLAY,
XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS, XDG_CURRENT_DESKTOP and XAUTHORITY were
all absent. Thirty short-lived authority PIDs appeared during that attempt.

Forwarding those six names produced one authority and a successful cold MCP call.
The [official Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)
documents `env_vars` for this handoff. The OpenAI Docs skill guided source checking;
the process observations, not documentation alone, establish this failure.

The local repair is in the existing orchestrator-owned installer. New/manual
Linux Codex setup forwards names, never captured desktop values. Existing owned
registrations missing the handoff become updatable. Migration preserves unrelated
fields, comments, custom forwarded names and explicit environment values; malformed
environment arrays remain untouched. Windows gains no Linux variables. Manual and
automatic setup use the same helper. This implements ADR-0117/0125/0127 without
changing a shared wire contract, relay, browser adapter or public support scope.

The original cold-start failure, successful explicit forwarding, and subsequent
generated-configuration trial are separate logs; do not overwrite the failure
with the workaround. The repaired tree passed 540 Rust tests (455 orchestrator
library), 210 extension tests, release-profile Clippy with warnings denied,
formatting, JavaScript syntax, and workbench-surface checks. After the host-owned
orchestrator-only swap, the product-generated manual fragment cold-started exactly
one authority through Codex and completed policy_explain with status succeeded and
effect none. The saved user client configuration remained unchanged. Automatic
writer/manual-fragment equivalence and migration were exercised by regression tests.

Repaired deployed orchestrator SHA-256:
b0e6faf96c489c8aff5f7b426f34a76f3d32c37b5a6bd265345c775765ddb101.
Both connector hashes remain the initial values above. The repaired isolated
process journey passed again. This is a local repair on
codex/fleet-test-02, not a new coordinated or published fleet candidate.

## Independent package and real-Chromium checks

Default-profile workspace Clippy (all targets, warnings denied) and all 540 Rust
tests also passed in an isolated target directory before the repair commit.

The portable-packager regression passed. Two independent invocations packaging
the repaired installed siblings produced identical archives, SHA-256
774822bd204147c2a2ab0dafabcd2c6e9814198c49e752ca394532dd1c63ecfe.
The six expected payloads have stable modes, owner/group and timestamps. Extracted
authority bytes match the repaired deployed hash and execute on the host as 1.3.5.
The installed Applications file passes desktop-file-validate. This is a local
Fedora-built package, not an Ubuntu-baseline or public-release provenance claim.

The repository's script-browser journey passed all 23 cases against official
Chrome for Testing 153.0.8010.36 on the host, with Chromium's sandbox enabled.
The downloaded archive's locally computed SHA-256 is
167a098c4fdec156b58a9f678c90a84f9072d789f9c6e7b35496a6987b8b7ef8.
This exercises the shipped evaluator, real CDP and process boundaries, including
script failure behavior and audit exclusion of private script/results. It uses
headless Chromium and a test native adapter, not the installed MV3 extension or
the user's normal desktop browser. It does not close the blocked lanes below.

The portable startup driver also passed: a missing Wayland display exits 1 after
15.076 seconds; a subsequent valid Open exits 0 and starts its own authority.
This is CLI recovery evidence, not a visible-window assertion. The portable
ownership lifecycle passed using a disposable bubblewrap home and separate XDG
directories: install preserves a foreign client entry; uninstall preserves a
foreign manifest, command, client entry and audit; reinstall restores owned
surfaces without losing audit. This uses fixture Claude configuration, not an
actual Claude client session. Despite the driver's no-tray directory name, this
test used the ordinary session bus and makes no no-tray UI claim. Extra fixture
authorities were stopped by exact executable identity; files remain for review.

## Blocked and not run

Native computer APIs are explicitly disabled in the provided tool surface. Only
the Codex in-app browser is exposed. Independently desktop-launched ordinary
browser acceptance, extension installation/reload, observed page effects,
human-browsing isolation (including overlapping agent work), native pause/stop,
visible GNOME workbench close/minimize/reopen, and actual browser reconnect are
blocked, not passed. Older child-adoption expectations are superseded by ADR-0164.

GNOME Applications entry bytes were checked; menu visibility was not. Three-client,
store-adapter, public-package upgrade/rollback, reboot, and new RPM packaging lanes
were not run. The staged native browser still needs activation after this session.
The coordinator task could not be resolved from this host; evidence is retained
locally for retrieval rather than repeated delivery attempts.

## Coordinator retrieval checkpoint

Credential intake is COMPLETE, not awaiting delivery. Machine bluefin (test-02)
has verified secure credentials for GitHub account lbotinelly with push permission
to sylin-org/ghostlight. The repository-local helper is libsecret / Secret Service,
with repository-path matching enabled. Commit identity is Leo Botinelly
<leonardo.botinelly@gmail.com>. No RSA recipient key was needed or created, so no
public key or fingerprint exists to send. No token or private key is in this report.

The signed-off repair commit is 4af4988522a17f7b733c9f34b0d400fa789ced95, pushed
without force to codex/fleet-test-02 and independently read back from the remote.
The repair's transfer patch is retained locally as
0001-fix-install-forward-Linux-desktop-context-to-Codex-M.patch, SHA-256
5aba3f3089c8b874219be777a111106c8f908b42833b33bad900ebfe3b034e75.
Fetch that branch or the repair commit; credential transfer is unnecessary.

The coordinator reports that remote task reads return items:[] even after final
responses. That report-delivery limitation remains unresolved. This sanitized
Git report is the alternate supported retrieval route, not a repair of task
delivery. Local BASELINE.md, RESULTS.md, CHECKPOINT.md, AUTH-STATE.md and raw
reports remain preserved. No acceptance tests were rerun for this checkpoint.

Last verified machine state: one installed repaired authority, PID 60429,
service 1.3.5, diagnostics enabled, browser Not connected. Original main checkout
remains clean at 48ef29ece1ff0f1633daba62a03932a666f00ca5. Native Chromium remains
staged and inactive. No GDM automatic login was configured; a request for someone
to unlock GNOME and reopen Codex after reboot is unanswered. No reboot recovery
is claimed. The campaign is incomplete for the blocked/not-run lanes above;
credentials are not their blocker. Do not rerun passing checks merely to keep the
task active. Resume desktop lanes when a permitted control/login path is available.
