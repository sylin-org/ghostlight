# Windows boot connection investigation, 2026-09-08

## Observed failure and restart experiment

After boot, the normal `target/release` authority (PID 11280, service 1.3.4) responded to doctor
and had no connected browser. Chrome was running and the source 1.1.2 extension was enabled.
All four HKCU native registrations were present; Chrome's 32-bit and 64-bit views agreed, HKLM
had no competing Chrome host, and no Chrome policy blocked native messaging. The manifest had
valid UTF-8 without a BOM, the expected host path and both allowed extension identities, and
read access for the current user. There was no browser-connector process.

The user's extension options showed `Not installed here`. Reloading the extension did not fix
it. The added built-in connection log captured two worker lifetimes, successful local state
initialization, native-port attempts, and actual reconnect alarms firing approximately every
three seconds. Each failed with `Specified native messaging host not found.` Logging reported
zero persistence failures. It was not a sleeping worker or an absent retry.

The owner requested a complete Chrome shutdown and relaunch. All Chrome processes exited
gracefully. Launching the same ordinary profile with native-host logging enabled produced native
connector PID 10288 at 18:47:38 local time and Ready within seconds. Authority PID 11280 retained
its original start identity. No binary replacement or native-registration repair preceded that
recovery. Because the relaunch also enabled Chrome logging, this is not a clean comparison of
logging flags; it establishes recovery after that complete relaunch, not a browser-cache cause.

Evidence remains under `.tmp/chrome-native-restart/`: selected doctor before/after snapshots,
process identities, pre-deployment binary hashes, the failed-state extension export, and Chrome's
local log. The extension export retains exact bounded native errors, not page contents.
The initial logger labeled a hello as sent even if the port closed during the awaited focus
check; current source records `native_hello_skipped` when `send` returns false and timestamps
events when observed rather than when their persistence queue drains.

## Normal deployed diagnostics

The extension's existing Developer diagnostics preference enables a bounded persistent connection
log. The options page can save it to Downloads while disconnected. The normal process diagnostics
marker enables all three sibling loggers across restarts, under `target/release/logs`.
The new process context and error events are documented in ADR-0145's amendment.

The standard dev loop deployed the three 1.3.5 siblings into the same installation. Doctor then
reported Ready, the native host reconnected, and the installed logs contained process context,
native hello size, the actual transient OS 10061 connection refusal during replacement, service
port publication, and successful adapter attachment. Only one authority remained running.

Validation: format, warnings-denied Clippy, all workspace Rust tests, 209 extension tests,
JavaScript syntax checks, the real process journey, and the real MV3 frame/editor journey
(68 checks) passed. Logs are `.tmp/startup-diagnostics-*.log`. The separate test processes ended
before the owner's explicit instruction to keep future live testing on the installed instance.
No further parallel desktop acceptance instance is authorized in this live-debugging session.

## Reboot continuation

The owner rejected the prepared OS observer and extra shortcut. The one-time RunOnce value,
desktop Chrome diagnostic shortcut, helper file, helper directories, and helper source were
removed. No such startup task remains. Built-in diagnostics stay enabled on the installed product.

Reboot normally and leave any failed Chrome process intact. Save connection diagnostics from the
extension options; inspect that export alongside the normal process logs. Do not require another
Chrome restart as the solution, and do not turn the successful relaunch into a root-cause claim.
At this point the original boot failure remained unresolved pending that capture.

## Reproduced with deployed logging

The owner rebooted at 19:05:32 EDT and reproduced the failure. Installed authority PID 18812
was running 1.3.5 and serving MCP before ordinary Chrome PID 16464 started at 19:06:24.
There was no browser-connector process or new native-connector log. The exported diagnostics
contained 74 events after boot, including 11 failed native attempts across two worker lifetimes,
zero successful handshakes, and zero persistence failures. Every disconnect reported
`Specified native messaging host not found.` Initialization completed and retry alarms fired.

Both Windows registry views still held the same REG_SZ manifest path. The manifest's timestamp
and SHA-256 were unchanged. Chrome, the authority, Explorer, and the diagnostic shell all had the
same user SID and medium-integrity, non-elevated, non-AppContainer process tokens. No Chrome
native-messaging policy was found in the inspected HKCU/HKLM policy keys. These observations
exclude simple registration loss, a stopped authority, and a stalled extension retry loop; they
do not prove what Chrome could actually open.

The raw export, selected process logs, identity snapshot, manifest hash, and event counts are
retained in `.tmp/boot-failure-20260908/`. WPR refused capture in the ordinary shell because
profiling requires administrator rights. The owner authorized elevation; the built-in recorder
then captured the same running Chrome into that repository evidence directory. Recording ended
normally. No startup task or extra product installation was used.

## Root cause and live recovery

The trace showed successful Chrome registry opens followed by manifest file opens returning
`0xC000003A` (`STATUS_OBJECT_PATH_NOT_FOUND`). The failed path was the ordinary per-user
`AppData/Local/Ghostlight/NativeMessagingHosts/org.sylin.ghostlight.json`. No connector executable
was reached. Registry values and file permissions were not the failing boundary.

Opening the supposedly existing manifest from this task and asking Windows for the final path
of its file handle exposed the discrepancy: the physical file was below
`AppData/Local/Packages/<Codex package>/LocalCache/Local/Ghostlight/NativeMessagingHosts/`.
The task's filesystem view redirected the logical AppData name. Its installer and doctor saw
the redirected file; Chrome launched independently after boot saw the missing ordinary path.
An elevated child retained that redirected view, as did attempted desktop-policy and shell
launch probes. Elevation and a process reporting no package identity were not sufficient tests
of filesystem visibility. The earlier logging-enabled Chrome launch inherited a view where the
logical path worked, explaining why that relaunch concealed the installation defect.

After verifying the existing manifest's host name and exact installed connector path, changing
only Chrome's registration to the physical manifest path restored Ready at 19:19:40 EDT.
Chrome retained PID 16464 and authority retained PID 18812. The real installed browser connector
PID 4176 received the native hello and connected to the authority. Neither browser reload nor
authority restart was required for recovery. The manifest and executable were not copied.

The source fix resolves the manifest's physical path after writing and publishes that path in
the registry. Inspection now marks a logical redirected registration Updatable instead of
Current. The existing owned-repair/install seam corrects it. ADR-0115 records this decision and
the lifetime limitation of caller-owned package storage. Format, warnings-denied Clippy, all
workspace Rust tests, and all 209 extension tests passed. This session's acceptance evidence is
the running installed stack and real Chrome/Windows trace, not a second desktop installation.

The normal dev loop then replaced only the installed orchestrator and applied native registration.
All four browser keys name the existing physical manifest. Chrome PID 16464 and native connector
PID 4176 survived the authority swap and reconnected to the new single authority PID 11164.
A regression check against the deployed executable briefly restored Chrome's original logical
registry value: `native-host check` reported Updatable, and `native-host install` reported
changed=true and restored Current. Ready remained true.

## Reboot acceptance completed

The owner rebooted again at 19:25:14 EDT and confirmed automatic connection, with a screenshot
showing Ready, two sessions, and one browser. A read-only check from this task independently
confirmed Ready on service 1.3.5. Exactly one installed authority was running (PID 7164, started
19:26:03), with the installed native connector (PID 23476, started 19:26:15), both under the same
`target/release` directory. This confirms the registration fix survives reboot on this machine.
No repair, extension reload, or browser relaunch was performed by this task after that reboot.
Built-in diagnostics remain enabled. The boot-connection investigation is closed; broader release
acceptance and the unchanged pending store submission retain their separately documented scope.

Selected raw trace events, decoded registry/file statuses, the registration repair, and the
successful native-connector log remain beside the ETL. Relevant source references:
[Chromium lookup](https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/extensions/api/messaging/launch_context.cc),
[Microsoft file-operation correlation](https://learn.microsoft.com/en-us/windows/win32/etw/fileio-opend),
and [packaged desktop filesystem behavior](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes).

The submitted store ZIP is unchanged and does not contain this later normal-source logging work.
No diagnostic fork, additional installed copy, package release, or new store submission was made.
