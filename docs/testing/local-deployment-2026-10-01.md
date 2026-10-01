# Authorized local deployment -- 2026-10-01

Leo explicitly asked to deploy Ghostlight locally and test it live. Reviewed implementation:
`c491a2643fbab7874606110452899e37b5c5d60f`, branch `codex/in-service-outcome-ux`.
The checkout was clean before deployment. No implementation, permissions, registration, public
version, or remote publication changed during deployment. This record is a documentation follow-up.

## Deployed identity

Used the supported `scripts/dev-loop.ps1 -Action Deploy` with a real PowerShell component array:
`@('orchestrator','browser-connector')`. The release build used `.target-dev-loop`; the exact-path
swap replaced `target/release/ghostlight.exe` and `ghostlight-browser-connector.exe`, then restarted
the selected authority. The unchanged MCP connector remained byte-identical.

All executable versions remain 1.3.12. The deployed files match their fresh release build:

| Component | SHA-256 |
| --- | --- |
| Orchestrator | `48599b8a82b76068c23db94c5902b5e433c54d6c2feb3d74e96c624aa30fc7d2` |
| Browser connector | `bbd1c2e3dd7786a82af26e34fb7c7fdd24b21558871c6df992a9891eb130c060` |
| Unchanged MCP connector | `0b9ce152c05d0710cd9ec6c18e359b87eecbd4ee5f14c6b67017b76e7de5216d` |

The existing unpacked Chrome adapter already points at this repository's `extension/`. Reloaded
only the identified Ghostlight extension through its internal details-page Reload control.
Worker file SHA-256: `b0cd4e1c0ca0786508e7090fc6c9ae321dbe425bc40257941c7d1d06849d950c`.
Actual background-attention navigation succeeds, establishing negotiated candidate functionality;
the file hash alone is not used as running-worker attestation.

## Live verification and actual limit

- Installed service and actual Chrome adapter report Ready.
- Running authority PID 23548 uses the replaced exact executable. The current native connector
  uses the replaced sibling, with Chrome itself still running.
- The native Workbench is open, responsive, and belongs to that candidate process. Its bundled
  UI comes from the reviewed build. Physical visual inspection is not established: the desktop
  region capture was occluded by another application and was discarded as UI evidence.
- The unchanged installed MCP connector negotiates the service and all 24 tools.
- A fresh synthetic localhost tab opens successfully under background attention.
- The subsequent read fails with `document_unavailable`, effect `none`. One bounded retry after
  a content-bootstrap interval fails identically. Fill is not reached. The complete smoke is
  therefore **not green**. No permission relaxation or unreviewed implementation fix was made.

Only fresh synthetic localhost pages were used. Closed only windows positively identified as
containing fixture tabs exclusively. Existing browser tabs and their data were not navigated,
refreshed, or closed. The internal extension-details test tab may remain open. The application
and browser connection are left running for Leo's live testing.

## Preservation and rollback

Local evidence/rollback directory:
`.tmp/local-deployment-2026-10-01T04-41-52Z/`.
It contains prior binaries, the prior Doctor/configuration-selection record, audit prefix identity,
both smoke attempts, final identity verification, and a guarded `rollback.ps1`.
The prior service and connector hashes are recorded in `before.json`.

Verified the installation selection and policy-file state unchanged, Chrome grants unchanged,
MCP binary unchanged, and every prior audit byte preserved as the prefix of the current audit.
No client setup or native-host registration update was needed. No browser restart occurred.

Open the app from this repository:

```powershell
& .\target\release\ghostlight.exe open
```

There is no HTTP Workbench address; it is the native application. Tray Open uses the same authority.
To explicitly roll back the replaced binaries from this repository:

```powershell
pwsh -NoProfile -File .tmp/local-deployment-2026-10-01T04-41-52Z/rollback.ps1
```

The rollback verifies saved hashes, quiesces demand-start, and stops/replaces only exact selected
Ghostlight images. It leaves current policy, history, diagnostics, selection, and registration in
place. Reload Ghostlight at `chrome://extensions` afterward, then open the native app again.
The controller is syntax-checked but was not executed, since that would undo the requested deployment.

Before the successful swap, two wrapper attempts stopped before any live mutation: a native
stdout/PowerShell JSON pipeline closed early, and `pwsh -File` flattened the component array.
Complete native-output capture and direct PowerShell script invocation resolved those setup errors.
No permission review rejection occurred. The remaining actual live blocker is document verification
on the synthetic read; source/native-isolated acceptance does not erase that installed result.
