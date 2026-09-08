# Fresh-machine native-host discovery incident

Date: 2026-09-08. Source: `c8bfb905`. Windows; Google Chrome 152.0.7977.83;
service 1.3.4; unpacked adapter 1.1.1.

## Required experience

The owner requires service and extension installation to make the existing browser usable without
a complete browser shutdown. Installation acceptance must exercise the real browser, extension,
native-host registration, and installed executables. A simulated adapter or replacement native
pipe does not establish this promise.

## Observed failure

The dev-loop built and deployed all three executables. The separate `ghostlight install --no-open`
registered browser native hosts and detected MCP clients. The enabled unpacked extension had the
expected development ID and source folder, but reported `Not installed here`. Doctor reported a
running authority, current registrations, and no connected browser. No browser connector process
or genuine Chrome-originated connector-start record was present.

Read-only checks found the expected HKCU Chrome key in both registry views, a valid UTF-8 JSON
manifest, the correct host name and allowed extension origins, and an existing executable at the
registered path. The connector could execute directly. There was no discovered native-messaging
policy override. These checks did not establish what Chrome actually read during the failure.

An extension reload did not help. The owner reported browser restarts without recovery. During
the later logging setup, an additional Chrome window remained open after a reported shutdown.
After that window was closed and no Chrome process remained, a diagnostic Chrome launch started
the registered connector and completed negotiation. Chrome's diagnostic log captured this
successful launch, not the earlier failed lookup. Neither the remaining window nor caching is
therefore a proven root cause of the original incident.

The registration and production source were unchanged when connection succeeded. The restart
restored this session; it is not a demonstrated product fix. Setup should not have been described
as complete before browser connectivity was proved.

## Real installed recovery check

The controlled follow-up used the person's installed Chrome, actual unpacked extension, production
HKCU native-host registration, and the deployed connector and authority. It used no replacement
native port, synthetic browser adapter, or copied browser profile.

1. Confirm the installed authority is Ready and idle; record the exact Chrome, authority, and
   connector process IDs and the native manifest hash.
2. Remove the owned registration through `ghostlight native-host uninstall`.
3. End only the exact installed browser connector and observe the browser become disconnected.
4. Restore registration through `ghostlight native-host install`, while Chrome remains open.
5. Observe automatic native-connector startup and Ready within 1,138 ms.
6. Verify the Chrome and authority process IDs are unchanged and the restored manifest hash
   matches its original bytes.

This check passed. Machine-local evidence and the executed script are retained under
`.tmp/native-debug/installed-20260908T154422/` and
`.tmp/native-debug/installed-late-install.ps1`. The script restores registration in its failure
cleanup path. Browser pages, drafts, and governance were not changed by this recovery check.

A fresh MCP connection subsequently verified the 23-tool catalog, listed its controlled tabs,
opened `https://example.com` in a new controlled tab, and read the expected Example Domain text.
That disposable tab remains visible under the preserve-tabs posture.

## Separate isolated evidence

Before the installed check, Chrome for Testing 152.0.7977.82 ran the shipped MV3 worker and real
native connector with a unique test host name. Registering that host after the browser was running
led to connection in 4,227 ms without restarting its browser process. Its temporary registration
was removed afterward. This used actual Windows native messaging, but its isolated profile is
not evidence that the original installed failure was reproduced.

The existing `frame-browser-journey.mjs` replaces native discovery with a loopback test pipe.
It remains evidence for its browser-mechanism assertions, not installation acceptance.

## Still unresolved

- Capture the original failed lookup in an ordinarily launched installed Chrome. The successful
  diagnostic launch and subsequent successful registration cycle do not identify its cause.
- Establish service-first and extension-first cold installation on a real supported desktop,
  keeping the browser process alive through installation and proving an actual browser read.
- Avoid interpreting the absence of a native host reported by Chrome as proof that the service
  has never been installed. Any language change needs to respect or explicitly amend ADR-0126.
- Chrome diagnostic flags apply only to the diagnostic browser run; they are not persisted in
  a shortcut or browser preference. Raw browser logs remain ignored machine-local artifacts.

No production fix, publication, or cross-platform acceptance is claimed by this record.
