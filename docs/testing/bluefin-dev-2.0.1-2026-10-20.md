# Bluefin re-test of the 2.0.1 dialog and history fixes

Re-test of the 2026-10-19 finding ([the bundle note](bluefin-2.0-bundle-2026-10-19.md)) on dev's
tip, run on test-02 on 2026-10-20 by a Claude Code session at the coordinator's direction, on the
machine's real installation and the owner's layered Chromium. Both fixes hold: a prompt's status
now says what its field holds, and history past either end refuses with the new sentence. One
machine fact worth keeping: restarting Chromium did not reload the unpacked extension's service
worker; only clearing the profile's Service Worker cache did.

## The system and the deploy

- The machine of the bundle note: Bluefin 44.20261013, kernel 7.2.5-200.fc44, GNOME on Wayland,
  SELinux enforcing, layered Chromium 155.0.8059.39 on the owner's profile. The installation was
  the public 2.0.0 ([that note](bluefin-public-2.0.0-2026-10-20.md)).
- dev at f0c5161e (one coordination commit past b04df585; same code), built in the Fedora 44
  Toolbox (`koi-dev`, the host lacks the WebKitGTK 4.1 development packages) with
  `cargo build --workspace --locked --release` into `.target-dev-loop`, then deployed on the host
  with `deployment install --development`, per DEV-LOOP's Bluefin rule. The plan listed all three
  executables; the install replaced them, restarted the service, and `status` reads
  `Channel: development`. The browser relay and this session's MCP edge reconnected on their own.
- The unpacked extension Chromium has loaded is the Downloads copy of the 2.0.0 archive
  (`~/Downloads/ghostlight-eba78013.../extension`), not the repo tree. dev changes only four
  `extension/lib` files and the manifest is identical, so the session copied those four files into
  the loaded folder before reloading.

## Finding: a Chromium restart does not reload the unpacked worker

After the file copy, a graceful Chromium restart served the old service worker: the prompt's
status had no field text and `browser_history` past the end timed out the old way. The profile's
`Service Worker/ScriptCache` still carried its 2026-10-19 date through two full restarts, and a
manifest version bump (2.0.0.1) changed nothing either (it was reverted). What worked: with the
browser closed, delete `~/.config/chromium/Default/Service Worker/`, then start it; the cache was
rebuilt and the new worker ran. So on Chromium 155, "reload the unpacked extension explicitly"
(DEV-LOOP) is load-bearing: a browser restart is not a reload. The reload button in
`chrome://extensions` remains the ordinary route when a person is at the keyboard.

The accidental mixed-version run is its own data point. With the 2.0.1 service and the 2.0.0
worker, a fresh agent (haiku, Ghostlight tools only, the first task as the owner's page words it)
no longer stalled to ask -- the language fix alone ends the 2026-10-19 defect -- but with no field
text in the status it answered the prompt with its own "Hello", and the page read
`the bell says: "Hello"`. The audit of that run also shows one `invalid_request` browser_dialog
call before the accept.

## The fixes, on the full dev stack

- The first task, exactly as worded ("Open sylin.org/ghostlight/demo/desk and ring the bell."),
  with a fresh `claude -p` haiku agent and only Ghostlight tools: navigate, find, click, status,
  accept, read -- six calls, no failures. The agent reported it accepted the default unchanged and
  the page read `the bell says: "hello from the desk"`. The status it acted on, reproduced by this
  session directly: `A prompt dialog is currently visible: "The bell asks: what should it say?".
  Its field holds "hello from the desk".`, with the next step saying accept answers with the text
  in its field, as OK does.
- History, in one tab whose two entries came from `browser_navigate` alone: `back` returned to the
  desk page and `forward` to the demo index, each landing on the exact entry; past the ends the
  refusals read `This tab has no earlier page in its history, so nothing was sent.` and the same
  with "no later page".

## GNOME window and tray on Tao 0.36.0

- The shell started with the service after the deploy (`ghostlight-desktop --tray`, the new
  binary). Its tray item is registered with GNOME's AppIndicator watcher
  (`tray_icon_tray_app_ghostlight`), Status `Active`, a 32x32 RGBA icon, and a menu of Open
  Ghostlight, Pause agents, Resume agents (disabled while not paused) and Quit Ghostlight. The
  Open item fired over dbusmenu opened and focused the window; `ghostlight open` did the same, and
  the WebKitGTK processes appeared under the shell.
- Not verified: the look of the window's titlebar controls. GNOME denies screenshots and window
  introspection to this session, so the visual check of the decorations under Tao 0.36.0 needs a
  person's glance.
- SELinux: no AVC record today names Ghostlight (none at all today).

## Side effects on the machine

- Chromium was restarted three times (graceful TERM, relaunched from its desktop entry). Open tabs
  were not restored automatically; they remain in history. `Service Worker` caches for web origins
  were cleared once and re-register on the next visit.
- The loaded extension folder in Downloads now carries dev's four `extension/lib` files over the
  2.0.0 archive; its manifest is back at 2.0.0 and matches the tree. Preserve Ghostlight tabs kept
  the demo tabs open, as the owner set.
