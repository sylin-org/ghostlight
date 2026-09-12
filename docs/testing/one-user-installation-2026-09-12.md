# One user installation: September 12 verification

Decision: [ADR-0167](../adr/0167-one-user-installation.md).

## Source and process evidence

- `cargo fmt --check` and workspace Clippy with warnings denied passed.
- Workspace Rust tests passed: 540 tests across the reported suites.
- Extension tests passed: 222 tests. No extension source changed.
- The ordinary process journey passed against freshly built debug binaries.
- The additional single-installation mode passed against the deployed release
  binaries. It uses an isolated home but no `GHOSTLIGHT_RUNTIME_FILE` and hard-links
  the sibling set into distinct development/package directories. Package CLI
  inspection selects development; both package-side connectors reach that service;
  concurrent foreign desktop launches reuse its identity; restart retains routing.
- Selection tests cover concurrent bootstrap, staged warm release upgrade,
  development custody through package setup, restore, missing selected binaries,
  malformed records, and preservation of history and policy locations.
- An inactive-uninstall process fixture proves early no-mutation exit. The actual
  installed registration preservation check below supplies Windows registry evidence.

Single-installation fixtures remain under `.tmp/single-installation-<pid>` as
evidence. All tracked child processes must exit. Windows WebView profile cleanup
can fail after native parent exit, so recursive profile deletion is not a pass
condition. Earlier exploratory runs exposed this cleanup race. The fixture no
longer installs into Windows' shared isolated registry namespace.

## Installed Windows evidence

The three freshly built release binaries replaced `target/release` through
`scripts/dev-loop.ps1`. Their SHA-256 values matched the isolated build:

| Binary | SHA-256 |
| --- | --- |
| ghostlight.exe | 2E49E10A99CC965882E9631CB6B09C21A5D7D50B3272485D14CB5C8A38B20554 |
| ghostlight-browser-connector.exe | 65D957AF6075254E982F21ED4D306ABAD9EC757CCE600748F2CF285F3F524D93 |
| ghostlight-mcp-connector.exe | A8ED302323F098FC9E57A031D97CD14D9F8DE437913ADA9D9E099B309448D011 |

The canonical runtime is `%USERPROFILE%/.ghostlight/ghostlight-runtime.json`.
Development selects `target/release`; no packaged restore candidate was invented.
The selection retained the existing audit/diagnostics directory and resolved the
pre-existing policy directory to its physical package-local Windows path. Later
browser-launched processes therefore do not reinterpret redirected AppData.

- The verified stray npm-cache authority was stopped. Cached release artifacts
  and their history were not deleted or modified.
- Doctor invoked from `.target-dev-loop/release`, not the selected directory,
  reported development and `Ready: Connected and idle. Agents can work when they ask.`
- The same foreign CLI completed `browser_tabs` with `action: list`.
- Chrome PID 19712 stayed running. Native connector PID 42060 survived authority
  crashes. The service recovered automatically to PID 10276, then PID 22388.
- An initialized MCP connector from `.target-dev-loop/release`, PID 28592,
  completed `browser_tabs` before and after the second service crash on the same
  stdio stream. It was then closed. This was a real Chrome/native-messaging path,
  not the synthetic adapter used by the process journey.
- Uninstall invoked from the inactive build preserved the actual Chrome registry
  value and manifest hash. No browser re-registration was needed for the check.
- The original 1,815,390-byte audit prefix survived exactly. Its SHA-256 was
  `536B37A6DCF2425DFCB2C0CDC3F71C70E17B2288B3BC1E0CAF71649EE3C9DAF3`.
  Verification calls appended ordinary history; `diagnostics.on` remained present.

The coordinating task's MCP transport was already closed before deployment and
stayed closed. Fresh connections and a retained initialized connection passed;
this does not prove a client automatically recreates a terminated stdio process.
This first lifecycle migration replaces both connectors, so affected clients need
a reconnect. Later orchestrator-only deployments retain the stable connectors.

The final diagnostics wording correction was deployed through the orchestrator-only
loop. Authority PID 14308 became Ready while native connector 42060 and existing
MCP connector 2404 stayed running. A foreign CLI again completed `browser_tabs`.
The final orchestrator SHA-256 is
`4DE33BB2AAF19D2E7135A72BEE60493FB0774E406AA3DE9E4CE09BAC96BF321C`.
The installed connector hashes above remain unchanged: the subsequent bridge diff
was documentation-only and did not require replacing either connector again.

## Not established

No public release was published. Explicit execution of archived pre-ADR-0167
code can still use its historical election; new source cannot retrofit that code.
Linux physical behavior, OS reboot, public package upgrade, and installed release
restore remain unverified. Restore/state selection is covered by source/process
fixtures, not claimed as an installed release journey. Browser content mutation
and every-tool acceptance were outside this routing check.
