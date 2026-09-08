# Linux acceptance tests

Linux-only installed-browser, desktop, and package-consumer drivers live here. Run them
explicitly on a dedicated acceptance installation. These are separate from the ordinary
`node tests/hardening-suite.mjs` gates. They can install/uninstall test registrations,
change test policy, open windows, and stop test processes.

The drivers were moved from `.tmp/linux-local/` and `.tmp/linux-installed/` on 2026-09-08.
Reports, browser profiles, keys, package archives, root filesystems, and generated context
files remain ignored. The dated [local acceptance record](../../docs/testing/linux-local-acceptance-2026-09-08.md)
and [integration record](../../docs/testing/linux-integration-2026-09-08.md) retain the original
results and limitations. Relocating the drivers does not rerun that acceptance.

## Environment configuration

Run from the repository root. Copy `environment.example.sh` to an ignored file for each
environment, edit its exports, and source it before invoking the runner:

```bash
mkdir -p .tmp
cp tests/linux/environment.example.sh .tmp/linux-kde.env
# Edit .tmp/linux-kde.env for this acceptance installation.
source .tmp/linux-kde.env
bash tests/linux/run.sh --print-env
bash tests/linux/run.sh acceptance/start.mjs
bash tests/linux/run.sh acceptance/governance.mjs
```

The environment file is ordinary shell code that you review and source yourself. It is
never loaded automatically. The runner accepts relative overrides, resolves paths from
the repository root, and exports the same values to Node, Python, and shell drivers.
Run the moved helpers through this runner, rather than invoking them without configuration.

| Variable | Default or purpose |
| --- | --- |
| `GHOSTLIGHT_LINUX_AREA` | `.tmp/linux-local`; dedicated local fixture and report root |
| `GHOSTLIGHT_LINUX_INSTALLED_AREA` | `.tmp/linux-installed`; installed desktop/worker fixture and new `recovery/` reports |
| `GHOSTLIGHT_LINUX_VERSION` | Workspace version in `Cargo.toml`; expected candidate version |
| `GHOSTLIGHT_LINUX_PREVIOUS_VERSION` | `1.3.4`; recorded public npm and upgrade predecessor |
| `GHOSTLIGHT_LINUX_BIN_DIR` | `$AREA/portable-a/ghostlight-v$VERSION-x86_64-unknown-linux-gnu`; extracted siblings for setup |
| `GHOSTLIGHT_LINUX_PORTABLE_ARCHIVE` | `$AREA/portable-fixed-a/ghostlight-v$VERSION-x86_64-unknown-linux-gnu.tar.gz` |
| `GHOSTLIGHT_LINUX_CHROMIUM` | `/usr/lib/chromium/chromium`; native Chromium executable |
| `GHOSTLIGHT_LINUX_BRAVE` | `$AREA/brave/opt/brave.com/brave/brave`; native Brave executable |
| `GHOSTLIGHT_LINUX_EXTENSION` | Repository `extension/`; unchanged unpacked development adapter |
| `GHOSTLIGHT_LINUX_GUEST_AREA` | `$AREA/guest`; disposable guest roots and build tree |
| `GHOSTLIGHT_LINUX_DEB` | `$GUEST_AREA/build/target/x86_64-unknown-linux-gnu/release/bundle/deb/Ghostlight_$VERSION_amd64.deb` |
| `GHOSTLIGHT_LINUX_PREVIOUS_DEB` | `$GUEST_AREA/build/public/ghostlight-v$PREVIOUS_VERSION-x86_64-unknown-linux-gnu.deb` |
| `GHOSTLIGHT_LINUX_POWERSHELL_DIR` | `.tmp/linux-tools/powershell-7.6.5`; extracted guest build tool |
| `GHOSTLIGHT_LINUX_RUST_TOOLCHAIN_DIR` | Active `rustup which rustc` toolchain; used only by the build guest |
| `GHOSTLIGHT_LINUX_CARGO_HOME` | Existing `CARGO_HOME` or `$HOME/.cargo`; guest dependency-cache source |

`$AREA`, `$VERSION`, and `$GUEST_AREA` above abbreviate the corresponding prefixed variables.
Use dedicated writable fixture directories. Installer drivers mount a disposable directory
over the invoking user's home inside bubblewrap, with separate XDG directories. Do not point
the area at a personal home or an ordinary installation. No machine notes or credentials are
required. Managed-policy signing keys are generated inside the ignored fixture.

Prerequisites: Linux, Bash, Node 22+, Python 3.11+, native Chromium, bubblewrap, and a usable
desktop session. Desktop drivers additionally need KDE KWin, `qdbus6`, the user journal, and
PyGObject; accessibility probes need AT-SPI. Guest drivers need user namespaces, subordinate
uid/gid mappings, `unshare`, and prepared distro roots. These prerequisites are not installed
by the runner.

## Drivers and prepared fixtures

| Driver, relative to this directory | Coverage and prerequisites |
| --- | --- |
| `installed-journey.mjs` | Opt-in real registration/native-pipe/authority/worker recovery. Requires explicit CLI arguments below. |
| `installed/desktop-check.py` | Exact-PID visible KWin Open/minimize/close/concurrent Open checks. Requires siblings under `$INSTALLED_AREA/bin` and their dedicated XDG environment. |
| `installed/worker.mjs` | Inspect the development adapter worker using `$INSTALLED_AREA/context/config/chromium/DevToolsActivePort`; optional argument is a JavaScript expression. |
| `acceptance/start.mjs` | Install extracted siblings into dedicated XDG state, launch Chromium, and write `$AREA/context.json` and `browser.json`. |
| `acceptance/governance.mjs` | Installed RAWX, frame coverage/masking, popup human controls, and audit privacy; requires `start.mjs`. |
| `acceptance/plural.mjs` | Chromium, Brave, second profile, draft retention, reload, and pinned-browser recovery; requires the shared context plus an already connected Brave and `$AREA/brave.json` containing its exact `pid`. Enable the source adapter in both browsers and the second ordinary Chromium profile first. |
| `acceptance/cold.mjs` | Extension-first install with awake/stopped worker; creates fresh contexts from the portable archive. |
| `acceptance/managed/run.mjs` | Signed managed-policy admission/cache/restart/rollback in a disposable `/etc/ghostlight` mount; creates fresh contexts from the portable archive. |
| `acceptance/startup-check.py` | Missing-display failure followed by valid Open; extracts the portable archive under `$AREA/startup`. |
| `acceptance/no-tray/check.py` | Start `$AREA/no-tray/bin/ghostlight` on a private session bus; write its context and keep the bus alive until `$AREA/no-tray/stop` exists. Extract siblings there first and create its XDG directories. |
| `acceptance/no-tray/desktop-check.py` | KWin lifecycle, exact descendant renderer failure, and Applications launch using the private-bus context. |
| `acceptance/no-tray/desktop_atspi_driver.py` | Diagnostic accessibility tree probe; lack of a usable tree is not a passing accessibility result. |
| `acceptance/portable-lifecycle.py` | Ownership-safe install/remove/reinstall using the no-tray context and disposable home. |
| `acceptance/npm/native.mjs` | Real public npm launcher and registered native browser journey. First install `ghostlight@$PREVIOUS_VERSION` with npm prefix `$AREA/npm/prefix`; it downloads the published binaries into `$AREA/npm/cache`. |
| `acceptance/cleanup.py` | Quit authorities inside `$AREA` through their tray; exact no-tray fallback only. Close dedicated browsers first; unrelated authority/connector identities must remain unchanged. |
| `acceptance/guest/get-rootfs.py` | Download and digest-check official `debian` or `ubuntu` image layers; arguments are image and tag. Writes only under `$GUEST_AREA`. |
| `acceptance/guest/enter.sh` | Enter a prepared build root at `$GUEST_AREA/root`; mount the prepared source/build tree at `$GUEST_AREA/build` and configured build tools. Remaining arguments are the guest command. |
| `acceptance/guest/consumer.sh` | Enter a prepared `debian` or `ubuntu` consumer root; mount candidate packages and these tracked upgrade scripts read-only. First argument selects the image; remaining arguments are the guest command. |
| `acceptance/guest/upgrade.sh` | Forward package upgrade with retained initialized MCP; run only inside `consumer.sh`. Uses `upgrade-user.py` beside it. |

The prepared rootfs/build directories are not bundled with the tests. The Ubuntu baseline
build uses the existing repository packaging scripts. The existing
`scripts/check-debian-package-lifecycle.sh` remains the package-lifecycle entry used by CI;
stage the current repository source in the guest build tree before invoking it there.

```bash
# Recovery: use the dedicated running browser and that installation's XDG environment.
bash tests/linux/run.sh installed-journey.mjs \
  --bin-dir /path/to/dedicated/siblings \
  --browser /path/to/native/chromium \
  --devtools-port-file /path/to/dedicated/chromium/DevToolsActivePort \
  --exercise-installed-stack

# Keep this private-bus setup running while the no-tray drivers use its saved context.
dbus-run-session -- bash tests/linux/run.sh acceptance/no-tray/check.py

# Only inside a disposable consumer; each run needs a fresh scenario id/user state.
bash tests/linux/run.sh acceptance/guest/consumer.sh debian \
  bash /linux-tests/upgrade.sh candidate-check-1
```

Use a new area or fresh profile state when a driver requires cold startup. Package upgrade
does not prove rollback with newer audit history. The source adapter and development desktop
results do not establish store-adapter, provenance-bound package, or GNOME acceptance.
