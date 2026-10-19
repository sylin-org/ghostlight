# Alpine and the 2.0 release candidate bundle

The Alpine step of [the owner's page](linux-other-environments-2026-10-19.md), run on test-03 on
2026-10-19 by a Claude Code session at the owner's direction. Both bundles failed the way the page
says they should, and neither changed anything on the machine. Then, by the owner's direction, a
musl build of `dev` was installed in place and used through the machine's own Chromium
([below](#a-musl-build-of-dev-installed-in-place)).

## The system

- Alpine Linux 3.24.1, kernel 6.18.48-0-lts, x86-64, musl. KDE Plasma on Wayland.
- No glibc loader: `/lib64` does not exist, so `/lib64/ld-linux-x86-64.so.2` is absent.
- Browser: the machine's real Chromium, 152.0.7977.82, on its default profile in
  `~/.config/chromium`. It is Alpine's own package, unpacked under the home folder during the
  September fleet pass (no root). Its launcher, `~/.local/bin/chromium`, sets the library path the
  unpacked package needs. The bundle runs never reached it: the install stops before any browser
  step. No other browser or profile was used at any point.
- The GitHub CLI 2.102.0 was put in `~/.local/bin` from its official release (checksum checked) to
  download the bundles, so `get.sh` could also verify build provenance.
- Already present: the September 1.3.5 musl development install
  (`~/.local/bin/ghostlight` linked into `~/ghostlight-fleet/test-03/installed/1.3.5-musl-debug/`),
  running before and after both bundle runs. The in-place install below replaced it.

## The bundles

| Release run | Revision | Run result |
| --- | --- | --- |
| 39297671342 | `149c04d67b27f68f211137f6b6976d788dcca4ed` | success (the run L2-11 names) |
| 39306759480 | `68419db8cbdf6517ca7b86783e2f6971391bd660` | success, both install-from-bundle jobs included |

Each `release-candidate` artifact was downloaded with `gh run download <run id> -n
release-candidate` into its own folder under `~/Downloads`. Both manifests say version 2.0.0. The
`get.sh` served at each revision is the same file as the tree at that revision.

## The install

The page's command, at each bundle's own revision:

```sh
curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate-<run id> sh
```

Both runs printed the same three lines, and the shell exited 1:

```text
Ghostlight 2.0.0 copied from /home/test/Downloads/release-candidate-<run id>. Checksums and build provenance verified.
sh: /tmp/ghostlight-install.<random>/ghostlight-service: not found
ghostlight: installation failed
```

| Step | Expected | Result |
| --- | --- | --- |
| The install, bundle 149c04d6 | Checksums verify, then `not found` and `ghostlight: installation failed` | As expected |
| The install, bundle 68419db8 | The same | As expected |

`readelf -l` on each bundle's `ghostlight-service-x86_64-unknown-linux-gnu` names
`/lib64/ld-linux-x86-64.so.2` as its interpreter. So the `not found` is about the missing loader,
not the file. That is the 2.0 scope the owner chose (no musl images), not a regression.

## After each run

- The temporary folder `get.sh` made under `/tmp` was gone.
- No file in the home folder with Ghostlight, sylin or native messaging in its path changed after
  the run started. The service never ran, so nothing was installed or registered.
- The 1.3.5 musl install and its running process were unchanged.

Not reached from the bundle, because the install stops first: the extension, an agent, the first
task, the tray and uninstall. There was nothing to uninstall.

## A musl build of dev, installed in place

The release stays glibc-only. This part shows what the 2.0 code does on Alpine, on the machine's
real installation and browser, not what a person gets from the release.

- Source: `a7f33da7`. Its code, extension and scripts are those of `68419db8`; only the coordination
  files and this record differ.
- Build: the dev loop's build, in release mode, with the flag the September musl pass used so GTK
  and WebKitGTK link dynamically:

  ```sh
  RUSTFLAGS='-C target-feature=-crt-static' cargo build --workspace --locked --release --target-dir .target-dev-loop
  ```

  It took 4 minutes with no warnings. All three images link through `/lib/ld-musl-x86_64.so.1`.
  Alpine's WebKitGTK 4.1 and GTK 3 development packages were already installed.
- Install: the dev loop's install command, run directly because this machine has no PowerShell:
  `.target-dev-loop/release/ghostlight-service deployment install --development`, after its
  `--plan`. It did these things:
  - swapped the three images into `~/.ghostlight/bin`;
  - registered Chrome, Edge, Brave and Chromium;
  - moved Codex's entry to the installed edge;
  - removed `bin/v1.3.4` (the glibc 1.3.4 images from September, never run here) and two 1.x
    folders: an empty state folder and 60 KB of window data.
- The plan stopped nothing. The September build ran from `~/ghostlight-fleet/`, which no 1.x
  installation record names. Its process was then stopped by exact process id and start time,
  leaving one Ghostlight.
- `ghostlight install` registered the detected agents, Claude Code and OpenCode. `ghostlight
  doctor`:
  - development channel;
  - every image ready;
  - Chromium "A native Chromium package can start Ghostlight." and registered.
- The extension: the profile has Ghostlight unpacked under its pinned development id, loaded from
  `~/ghostlight-fleet/test-03/source/extension`. That worktree was moved (detached) to the same
  revision, so the 2.0 extension loaded under the same id with no change to the profile.
- Diagnostics were turned on with `ghostlight diagnostics on`.

| Step | Result |
| --- | --- |
| The tray | KDE's StatusNotifierWatcher lists Ghostlight's item |
| An agent asks with no browser open | Refused: no browser is connected. Linux's browser startup default is `manual`, so this is the design |
| The owner opens Chromium from the desktop menu | Chromium starts the installed edge for the extension. `doctor` reads Ready |
| The first task, a fresh Claude Code agent (haiku, 8 turns, 16 s) | Navigate, find, click the bell; a prompt opens; the agent accepts it. The readout says "the bell rings silently" (finding 1) |
| The window | `ghostlight open`: KWin lists a window "Ghostlight" with app id `org.sylin.ghostlight` |
| `tests/linux/installed-journey.mjs` on the owner's Chromium | All four phases pass (finding 2): registrations reinstalled with the browser running (1.6 s), browser edge crash (3.1 s), service crash with the MCP stream kept (5.7 s), an in-flight effect not replayed (6.7 s, one effect) |

## Findings

1. **Accept answers a prompt with an empty string.** The demo's prompt has a default answer,
   "hello from the desk". A person who presses OK sends the page that default.
   - Ghostlight's `browser_dialog` `accept` sends no text: `work/interaction.rs` gives text only to
     `respond`, and `extension/lib/primitives.js` then leaves `promptText` out of
     `Page.handleJavaScriptDialog`. Chromium 152 gives the page `""`, so the page reads "the bell
     rings silently".
   - Reproduced with `ghostlight call`: the click, then `status`, which shows the question, then
     `accept`, gives the same readout.
   - Nothing here is Alpine-specific.
   - The `status` result carries the question but not the default answer. Neither the click's
     sentence nor the refusal's next step points an agent at `status` before it answers, so the
     agent accepted without reading the question.
2. **The installed journey runs the browser binary directly to read its version.** A browser that
   starts only through its launcher's environment fails that probe before any phase. This
   Chromium needs its launcher's `LD_LIBRARY_PATH`.
   - With that path in the journey's environment, every phase passed.
   - The service that the journey's MCP edge started inherited the variable. No Ghostlight image
     loads a library from that folder (`ldd`), and a redeploy restarted the service clean.
   - Reading the variables from the running browser root's environment would avoid both problems.
3. **`installed/desktop-check.py` reads KWin's output from the systemd user journal.** Alpine runs
   OpenRC, so the check cannot run here. The window was checked through KWin's window runner
   instead.
4. **The browser edge was replaced once, 21 s after Chromium first connected.** The diagnostics
   show `browser_gone`, then a new edge 0.25 s later. The cause is not known; the extension may
   have been reloaded in `chrome://extensions`.

## State left on test-03

- The development build of `a7f33da7` serves from `~/.ghostlight/bin`, with diagnostics on.
- Codex, Claude Code and OpenCode name the installed edge.
- The fleet worktree is detached at `a7f33da7`; its `codex/fleet-test-03` branch is unchanged.
- The September 1.3.5 images are still in `~/ghostlight-fleet/test-03/installed/`, but nothing
  names or runs them.
- Two demo-desk tabs stay open in Chromium, because Preserve Ghostlight tabs is on.
