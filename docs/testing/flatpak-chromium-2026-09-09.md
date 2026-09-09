# Flatpak Chromium investigation, 2026-09-09

The owner prioritized Flatpak Chromium before remaining native Chromium acceptance.
New work is in the ordinary Ghostlight repository,
on codex/fleet-test-02-flatpak, based on f94e4a74. The previous fleet directories
and installed binaries remain intact; no parallel workspace was created for this work.

## Proven so far

- After the owner-authorized reboot, Bluefin 44.20260908 is active and native
  Chromium 152.0.7977.82 is installed. Normal native-browser registration passed.
  A post-reboot cold Codex invocation reached the installed authority and completed
  policy_explain with effect none and history_storage saved. Installed binary
  hashes remain those of the published integration report. This is not a native
  Chromium browser-journey claim.
- Installed Flathub Chromium: 152.0.7977.82, commit
  650cf951c98ab3be7c88d95a6591a947123c88e3791cdfd90ec144082bc47b01.
- The package's existing permissions include home filesystem and shared network.
  No overrides, general session-bus access or host-command permissions were added.
- Its XDG configuration root is
  $HOME/.var/app/org.chromium.Chromium/config. The host's ordinary native
  manifest is outside Chromium's lookup location there.
- A test manifest was added at that root's
  chromium/NativeMessagingHosts/org.sylin.ghostlight.json. The path was absent.
  It names the exact already-installed browser connector and the two existing
  authorized Ghostlight extension origins. It does not introduce a wrapper.
- A real connector process inside Flatpak reads the existing runtime document,
  authenticates to the running host authority and completes a synthetic adapter
  handshake: hello_accepted, protocol 2, service 1.3.5. The fixture then disconnects.
  This proves warm transport, not an installed browser journey or cold startup.
- The first synthetic probe used invalid browser/epoch prefixes and was rejected
  at adapter negotiation. Correct domain identifiers pass. That initial probe
  failure is not a Flatpak transport failure.
- After the owner reloaded the real extension, doctor reported Ready and the
  native relay recorded its actual adapter hello and service connection.
- A persistent installed MCP session opened a disposable loopback fixture through
  that extension and read back its expected text. Both operations succeeded.
  Closing the test tab was blocked by the person's preserve-tabs setting. The
  setting was not changed and the test tab remains open. The complete test report
  is not green because its cleanup assertion expected a successful close.

The installed siblings retain the hashes in the Bluefin integration report. Test
registration points to the existing installation under the old fleet source
directory; it has not been migrated into the main repository by this investigation.

## Unresolved

The real developer-mode extension's warm connection and open/read journey pass.
Native desktop controls are unavailable; no substitute UI automation was used.

Cold startup cannot run the host authority directly inside Chromium's runtime:
its WebKit dependency is absent there. The installed desktop portal exposes no
NativeMessaging or WebExtensions interface. A browser extension point supplies
files within the sandbox; it does not itself provide host process activation.

Do not claim complete support based on warm transport. A maintained solution must
retain the one host authority, existing governance and runtime ownership, bounded
recovery, ownership-safe install/remove, and browser-only relay responsibilities.
General flatpak-spawn --host permission, broad filesystem/bus overrides, a second
authority in the sandbox, or an always-running supervisor are not accepted fixes.
Any new activation mechanism needs an ADR and a cold-start/recovery proof before
the installer may advertise Flatpak support. Native Chromium work is deferred,
not discarded.

## Sources and evidence

The [Chromium Flatpak package](https://github.com/flathub/org.chromium.Chromium)
documents extension points for native-host manifests. Its
[manifest](https://github.com/flathub/org.chromium.Chromium/blob/master/org.chromium.Chromium.yaml)
defines the shipped permissions. The installed package's permissions were also
read directly; source documentation alone is not the machine-state evidence.
[Flatpak sandbox guidance](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
requires narrow bus access and recommends portals instead of blanket permissions.
The proposed [WebExtensions portal](https://github.com/flatpak/xdg-desktop-portal/pull/705)
is not evidence that this host implements it; host D-Bus introspection was checked.

Local generated evidence is under the repository's existing .tmp/flatpak convention:
warm-transport-probe.log (initial invalid fixture), check-warm-relay.mjs and
warm-relay-verified.log. Original Bluefin failure and post-reboot reports are retained.
check-live-page.mjs and live-page-result.json record the real installed open/read
test and preserve-tabs cleanup refusal.

## Coordination addendum

This documentation-only checkpoint records evidence collected after f94e4a74.
No tests were rerun for publication, as requested by the coordinator. No product
code, credentials, installed binaries, or schedules were changed for this check.
Flatpak cold-start and recovery acceptance remain pending. The added native-host
manifest is manual test registration, not implemented installer support.
