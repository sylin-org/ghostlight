# Fixed installation candidate -- 2026-10-02

The owner authorized fixed package paths and reuse of the existing extension injector.
ADR-0193 records the decision. npm, PowerShell and shell downloaders now install the
verified sibling set into `~/.ghostlight/bin`, without a release directory. Downloads
remain bound to release checksums. A complete verified download invokes the staged
service's existing deployment seam before replacing any installed binary.

The native seam stops changed exact image paths, replaces siblings, and preserves
unchanged connectors. Legacy owned routes migrate once. Existing history and policy
custody survive; development selection remains authoritative. Ordinary replacement
errors restore displaced binaries. Registration migration failures retain old binaries
and election for retry. This is not a power-loss atomicity guarantee.

The packaged extension is unchanged. Its fixed `org.sylin.ghostlight` identity resolves
through the native manifest to the selected connector. The current service runtime is
sent and acknowledged again on reconnect. New privileged browser mechanisms still
require packaged extension support.

## Evidence

- Formatting, strict workspace clippy and workspace tests pass using `.target-dev-loop`.
- All 404 extension tests and 11 npm launcher tests pass. The injector accepts and
  executes two different service bundles with the same mechanism revision. The npm
  fixture keeps one path across releases, rejects a corrupt third binary before
  installation, preserves old bytes, and removes its temporary download.
- The real process journey passes against freshly built siblings. The same native
  peer installs the service runtime again after the authority restarts.
- The fixed-installation journey passes on Windows with an isolated user profile.
  Owned native/MCP routes move from `bin/v1.3.11`, comments and history survive,
  and the old binary directory disappears. A changed service image replaces a
  running Windows executable. Both unchanged connector PIDs and their client
  streams survive; the service epoch changes, runtime is acknowledged again,
  the MCP catalog remains available, and native registration is unchanged.
- The shipped PowerShell downloader passes offline with a space-containing profile,
  verified real executable bytes, waited native installation, corrupt final download
  preservation and temporary cleanup. PowerShell parser checks and shell syntax pass.

The installation fixture changes the service image with valid trailing executable
data; the distinct-runtime-content test is a separate injector component fixture.
Synthetic native framing is not real Chrome/MV3 acceptance. Linux execution was not
available on this Windows host; the Linux installer fixture remains a CI gate.
The new workspace fixture initially used a noncanonical Windows directory and entered
registration migration unexpectedly. It is corrected, and an explicit private-runtime
guard now prevents test elections from invoking user registration installers.
Automatic approval review rejected deletion of that failed unit test's temporary
directory with "blocked by policy". That one OS temporary directory remains; the
successful installer and process fixtures completed their own cleanup.

No service/extension versions were bumped and no release was published. The person's
running service binaries and browser profile were not deployed or reloaded. This is
a source candidate for a release whose service includes `deployment install`.

## Reproduce

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --target-dir .target-dev-loop -- -D warnings
cargo test --workspace --target-dir .target-dev-loop
cargo build --workspace --target-dir .target-dev-loop
npm test --prefix extension
npm test --prefix packaging/npm
$env:GHOSTLIGHT_BIN_DIR = '.target-dev-loop/debug'
node tests/process-journey.mjs
node tests/fixed-installation-journey.mjs
pwsh -NoProfile -File tests/installer-powershell.ps1 -BinaryDirectory .target-dev-loop/debug
```
