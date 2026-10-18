# Linux checks

Linux-only checks that run against the real installation and the browser the person uses.
Acceptance never runs in a sandbox: no bubblewrap home, no dedicated or throwaway browser
profile (docs/MEMORY.md, "Full integration means the installation in place"). The bubblewrap
drivers, their runner and the guest build roots that used to live here were removed on
2026-10-17 by the owner's direction during the Linux batch
([linux-1](../../docs/tasks/linux-1/LEDGER.md), L03). Git history keeps them, and the dated
records under `docs/testing/` keep their results.

These checks are opt-in and separate from the ordinary `node tests/hardening-suite.mjs` gates.
Reports and generated state stay under the ignored `.tmp/`.

## What is here

| File | What it checks |
| --- | --- |
| `installed-journey.mjs` | Recovery of the real installation: the browser registrations reinstalled while the browser runs, a crashed browser edge, a crashed service under an initialized MCP stream, and an in-flight effect that is not replayed. It uses `~/.ghostlight/bin`, the real registered native host and the running system browser, and stops the service and the browser edge by exact process. |
| `installed/desktop-check.py` | The real installation's window as KWin counts it: Open shows one active window, Open after minimize brings one back, closing keeps the service, eight concurrent Opens give one window, and a lost service comes back without opening the window. Needs KDE KWin, `qdbus6` and the user journal. |
| `startup-recovery.mjs` | A start with no desktop session and four MCP clients at once converge on one service. It runs in a temporary test home with no browser, from the build `GHOSTLIGHT_BIN_DIR` names. |
| `installed-reporting.test.mjs`, `lib/reporting.mjs` | The installed journey's result reporting. The hardening suite's browser-harness runs this test. |

## Running

`installed-journey.mjs` needs an idle, Ready development installation
(`pwsh scripts/dev-loop.ps1`), every browser registration naming it
(`ghostlight install --all-browsers`), and the system browser running with the extension
connected. Its report goes under `.tmp/linux-installed/recovery/`, or under
`GHOSTLIGHT_LINUX_INSTALLED_AREA` when that is set.

```bash
node tests/linux/installed-journey.mjs --bin-dir ~/.ghostlight/bin \
  --browser /usr/lib/chromium/chromium --exercise-installed-stack

python3 tests/linux/installed/desktop-check.py

GHOSTLIGHT_BIN_DIR=.target-ghostlight-1.0/debug node tests/linux/startup-recovery.mjs
```

## Lessons

- **Exact processes.** Signal only the exact process you mean: match its executable path and
  start time, never an image name, because the person's own stack and browser are running. See
  docs/testing/linux-combined-packaging-2026-09-09.md.
- **Fresh state for a journey that starts its own service.** A service left running by an
  earlier build can satisfy a readiness wait for the wrong one. See
  docs/testing/linux-container-evidence-2026-08-15.md.
