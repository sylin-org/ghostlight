# Windows fleet packaging and acceptance, 2026-09-09

Lane: LEO-DESKTOP-02, Windows 11 Pro 25H2 x64, build 26200.9445.
Starting candidate: `a8cfd0337d83079f5fad3d5dfec846c31ba3b8fd`.
Peer fix: `5afcf7b9cc683a2eb88071c1b54d00e212420c85`.
This is a development-host lane. Rust, MSVC, Node, VC runtimes, and WebView2
were already installed; it does not establish a pristine consumer installation.

## Original failure and owning fix

Public NSIS 1.3.4 installed successfully at its default selected destination.
Actual VS Code started its installed MCP connector and discovered 24 tools.
The default interactive candidate upgrade reached an uninstaller that Computer
Use could not target. Its installer processes were canceled; that is an
automation limit, not proof of a product deadlock.

The same candidate's silent `/S /UPDATE` path returned zero but left the running
public MCP connector's old bytes beside the newer authority and browser relay.
Stopping that connector through VS Code and rerunning the same installer made
the MCP bytes converge. An updated live catalog alone had concealed mixed files.

The original hooks registered the browser host but did not quiesce all siblings
or hold the existing deployment lock. Tauri's generated process check matched
the authority by image name and missed the connectors. The fix stays in the
NSIS lifecycle seam required by ADR-0115:

- Atomically create `deploy.lock`; preserve and refuse another owner's marker.
- Resolve the selected installation to its physical directory.
- Enumerate processes and register only exact installed executable paths with
  Windows Restart Manager, binding each PID to its process creation time.
- Stop those registered processes before copying or unregistering. Override the
  generated image-name check so a separate Ghostlight installation survives.
- Check existing file access before package mutation. An unrelated reader is
  never a shutdown target; incompatible sharing should refuse the operation.
- Release only the hook's own marker on success or failure. Registration failure
  returns a nonzero installer result rather than only a log warning.

No service, CLI command, resident helper, restart task, or connector protocol was
added. The access probe is a preflight, not a transactional rollback guarantee;
it cannot exclude a new unrelated file opener racing subsequent replacement.

## Experiments and retained failures

An early prototype registered file resources. It replaced the running installed
sibling set and preserved a separately running authority, but file registration
could also target unrelated readers. The final mechanism registers process
identities instead. Its fresh-install test initially failed before copying with
Windows error 80: last-error was captured too late. NSIS `System::Call ... ?e`
now captures it at each relevant call. The corrected fresh-install retest passed.

With the process-registration package, actual VS Code's installed MCP connector
and the installed authority stopped during upgrade. A separate source authority
and a Python process reading the installed authority file both remained alive.
The installer returned zero and removed its lock. This is the concrete isolation
regression, not a source-text assertion.

The final additional access preflight built successfully and its silent installed
upgrade returned zero. The attempt to run its uninstaller while holding a file
open was rejected by automatic approval review with only `blocked by policy`.
It was not retried through another surface. That final refusal/uninstall branch
remains unverified; an earlier prototype's successful uninstall is narrower evidence.

## Artifact identity

All hashes below are SHA-256. Service and sidecars are release builds containing
the peer fix. The final installer additionally contains the uncommitted hook at
the source fingerprint below; the packaging commit changes no binary source.

| Artifact | Hash |
| --- | --- |
| Public 1.3.4 installer | `2c2225a75e208e0164b79d92b413b1b156ce9bd557162e97157009308aed250b` |
| Original a8cfd033 candidate installer | `c0d93b41a31e65e20eac2a8ae011a8c67da8dc9064de7a59e972a1f30c8f6a27` |
| Process-registration package used for isolation proof | `9a06b35a11118ad39fb23caace4891debf76cf4129153cd6fee59563c92c202c` |
| Final access-preflight installer | `c687442f840b615c217f587a5a2236dddb171eed8c0697bb73d0a0020f78e6ba` |
| Final hooks.nsh source bytes | `c34cac3decccc7423b41139faf81f727e91cd962ba739c35413b56283a74f408` |
| Installed authority | `adc4a0691c0d24192246bdb5cce922abe88d4e9ea775ed1c7517e150e016db99` |
| Installed MCP connector | `d01465b67c3535985f20205f933126f90ce709bcacb6141a09d0600f191a92ed` |
| Installed browser connector | `7595fa7a8149386a496254cf8f4681ac620d605db54a107fc12eabe311f382d4` |

Tauri 2.11.0 temporarily replaces the authority's bundle marker `UNK` with `NSS`
while creating NSIS, then restores the raw build. The installed authority differs
from the raw release file by those three expected bytes. Applying that substitution
in memory produces a byte-for-byte match; both installed sidecars match raw builds.
The package is locally built and unsigned, not a GitHub provenance-bound release.

## Result matrix

| Lane | Result and boundary |
| --- | --- |
| Required source gates | PASS: fmt, warnings-denied workspace Clippy, 531 Rust tests, 210 extension tests; no changed extension JavaScript |
| Additional source checks | PASS: npm launcher, workbench surface, release build, NSIS bundle |
| Real executable process journey | PASS with `GHOSTLIGHT_BIN_DIR=.target-fleet/release`; browser shore is a fixture |
| Public default installation | PASS on this development host |
| Original silent upgrade | FAIL: zero exit with old running MCP bytes retained |
| Corrected fresh installation | PASS for process-registration package |
| Exact process scope during upgrade | PASS: installed authority/MCP stopped, foreign authority and unrelated reader survived |
| Existing deployment marker | PASS on earlier hook prototype: nonzero refusal, original marker retained |
| Final access-preflight installation | PASS: zero exit, all three expected files, no marker left |
| Uninstall and reinstall | Earlier prototype PASS: three siblings, four browser keys, and uninstall entry removed; final uninstaller BLOCKED by tool review |
| Real VS Code transport | PASS: installed connector, Running, 24 tools on public service and 23 after upgrade |
| Actual model-to-MCP invocation | PASS: signed-in Codex CLI invoked installed `policy_explain`, returned all-open authority and a durable receipt |
| Actual model browser call | BLOCKED: Codex required tool approval while approval policy was never |
| Workbench Close/Open | PASS: Close destroyed only the view; five concurrent Open calls yielded one replacement view in the same authority |
| Browser installation and jobs | BLOCKED: no adapter loaded; Browser Use rejected `chrome://extensions` and its alternate-surface workarounds |
| Store adapter | NOT VERIFIED: store navigation returned generic `Not allowed`; no installation or review conclusion follows |
| Reboot browser recovery | NOT RUN on this machine; cannot establish connection recovery without the adapter |
| Clean Windows without prerequisites | NOT RUN; this host already has development tools and runtimes |

Chrome was launched independently through Explorer's ordinary Start Menu shortcut
and remained running through installation and package tests. No browser launch from
the installer context was substituted for it. No native adapter process was observed.

## Handoff and machine state

The selected installation is the ordinary per-user Ghostlight location, but this
packaged caller redirects storage into its package cache. All four browser keys
name the existing physical manifest, as required by the ADR-0115 amendment.
The installed files match the final hashes above. One installed 1.3.5 authority
is running, no deployment marker remains, and doctor truthfully says Not connected
because no browser adapter is connected. The independently started Chrome remains
available. The disposable foreign authority and reader from the isolation proof
have ended. VS Code is reconnected through its own server controls after replacement.

Exact machine paths, PIDs, command outputs, failed runs, and package copies are
retained under the host's ignored `.tmp/fleet-leo-desktop-02` evidence directory.
Its original public/candidate files were not overwritten by later package results.
Key evidence files include `silent-upgrade-result.json`, `retry-upgrade-result.json`,
`process-scope-proof.json`, `reinstall-final.json`, `file-access-install.json`,
`fixed-authority-bundle-check.json`, `package-final-state.json`, and
`codex-policy-acceptance.jsonl`.

Remaining acceptance needs the current adapter loaded through an allowed route,
the final package's uninstall/refusal test in an environment permitting it, and
the exact candidate/store/clean-machine lanes. Do not label this matrix release-ready.
