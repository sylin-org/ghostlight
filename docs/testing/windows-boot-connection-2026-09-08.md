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
The original boot failure remains unresolved pending this capture.

The submitted store ZIP is unchanged and does not contain this later normal-source logging work.
No diagnostic fork, additional installed copy, package release, or new store submission was made.
