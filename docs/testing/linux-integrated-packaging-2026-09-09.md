# Integrated Linux package validation, 2026-09-09

## Result and custody

The exact integrated baseline `ac1becab78e5a8a6b89e553e82f6779d93dcb39c`
(tree `b6e0b7b3c5c84cb4fb82a3fc15fe5cd2d4395b88`) now has Ubuntu-baseline
portable and Debian candidates with passing Debian 12 and Ubuntu 24.04 consumer
checks. This closes the earlier test-01 gap where package artifacts were still
a8cfd033. No product or tracked packaging-script change was needed.

Work ran on `codex/fleet-test-01-packaging` in the ordinary repository. No new
worktree was created. The original guest roots, package artifacts, failure evidence
and live installation paths remain intact. Service/package is 1.3.5; the source
adapter is 1.1.2. The [candidate and evidence manifest](linux-integrated-packaging-2026-09-09.json)
records source, artifact, installed and evidence-file SHA-256 identities.

## Verified matrix

| Lane | Result and boundary |
| --- | --- |
| Required source gates | Formatting, warnings-denied Clippy, 542 Rust tests and 222 extension tests passed. No repeated full hardening campaign. |
| Source custody | All 1,029 staged Git blobs match ac1becab before build and after packaging. Workspace outputs were cleaned; cached dependencies were reused. |
| Ubuntu baseline build | Ubuntu 22.04.2, glibc 2.35, Rust/Cargo 1.95.0, Tauri CLI 2.11.0, PowerShell 7.6.5. Locked offline workspace release build, sidecar staging, offline Tauri Debian bundle, finalization and native inspection passed. Cargo.lock remained byte-exact. |
| Artifact content | Three exact sibling executables at 0755; portable legal files at 0644; six-file portable roster; Debian legal, desktop, manuals, native manifests and conffiles verified. Manifest lists every payload file, mode and hash. |
| ABI and dependencies | All siblings require at most GLIBC_2.34, with no RPATH/RUNPATH. Both Debian consumers resolve all runtime libraries and pass dpkg verification. |
| Portable reproduction | Two separate packager processes produced byte-identical archives. This is same-environment archive reproducibility, not independent reproducibility of the compiled ELF or Debian archive. |
| Debian 12 lifecycle | Exact candidate installed, queried, started as an ordinary user under Xvfb, initialized MCP, removed, reinstalled and purged. Runtime mode 0600 and retained user runtime were verified. |
| Ubuntu 24.04.4 lifecycle | Same checks passed in the serial follow-up. The initial concurrent readiness timeout remains a failed run below. |
| Registration ownership | Both consumers repaired a valid owned stale per-user Chromium path on packaged first launch, preserved a foreign same-filename Brave manifest byte-for-byte, and created no per-user desktop shadow. Package purge removed package-owned system registrations. |
| Retained-MCP upgrade | Both consumers upgraded public 1.3.4 to this exact 1.3.5 Debian candidate. Package replacement left the old authority running; explicit authority restart/Open activated the new image. The same initialized MCP process recovered its 23-tool catalog and completed policy work. Foreign client and native registration bytes remained unchanged. Installed siblings matched bytes extracted from the candidate. |
| Portable installer | Both consumers passed install/uninstall/reinstall with exact owned command, desktop and native paths. Foreign client entries, same-filename native manifest, command file and synthetic retained history survived. The actual integrated Codex writer supplied the six desktop environment names while retaining a foreign server. No real Codex application or portable authority was started in these fixture checks. |
| Normal installed smoke | After authority-only native deployment, existing default Chromium and Brave profiles passed new-tab open, fill, targeted replacement typing, read, independent script draft readback and per-session tab isolation. Both actual catalogs declared all 23 history-storage schemas. Connectors and source adapter were unchanged. |

## Artifact identities

Debian archive SHA-256:
`e6c3776fb60c46d8f0f103b8bace7baac8ca093b9c648571667397f61310b8fe`.
Portable archive SHA-256:
`b7facfa6db38ecaf81b930c16ab65ed51e45b9f4d8546fcdbdfa557f1b5ea625`.

| Sibling | Raw/portable SHA-256 | Debian SHA-256 |
| --- | --- | --- |
| ghostlight | f6e654a472caa0f87e8587204e96ed50cff9957e3916247e623b116ae5c2743a | 0b148ecf2da25283fd90b162b3a7fc3ca367f8028b81f399b4f185f0bd3739eb |
| ghostlight-mcp-connector | 3e9c91e6181e8076985e05ba5a0cab15ae4f85db2adc8ef4d4b7b0b4f2a66143 | 3e9c91e6181e8076985e05ba5a0cab15ae4f85db2adc8ef4d4b7b0b4f2a66143 |
| ghostlight-browser-connector | 141283558888c9549b3d348fce544507845fcbf57f77525bc5d0e5bbe96c2599 | 141283558888c9549b3d348fce544507845fcbf57f77525bc5d0e5bbe96c2599 |

Tauri changes the authority's three-byte UNK-to-DEB bundle marker. Extraction was
checked against that exact expected transformation; connector bytes are identical.
The predecessor Debian SHA-256 is
`30e525d6cd21da30c6569e713a53e9e892adb8b1f0f2aaf325086a0c76eea401`.
It is the previously verified public artifact; this round made no new remote
attestation check. Neither new candidate has CI provenance or publication authority.

## Reproduction and evidence

The ignored area is `.tmp/linux-local/integrated-ac1becab/`. `environment.sh`
selects its prepared builder/consumer roots, the established PowerShell/toolchain
locations and the retained predecessor package. The roots were copied with
`cp -a --reflink=auto` inside the existing user-namespace mapping. Source was staged
with `git archive ac1becab78e5a8a6b89e553e82f6779d93dcb39c`; only compiled dependency
caches and the Tauri CLI were reused. The package manifest includes the builder's
complete dpkg package/version inventory. These are prepared disposable guests,
not pristine OS installation images.

The existing drivers carry every package mutation inside the guest namespace:

```bash
source .tmp/linux-local/integrated-ac1becab/environment.sh
python "$GHOSTLIGHT_LINUX_AREA/verify-staged-source.py"
bash tests/linux/run.sh acceptance/guest/enter.sh   /bin/bash /work/integrated-build-candidate.sh
python "$GHOSTLIGHT_LINUX_AREA/record-artifacts.py"
bash tests/linux/run.sh acceptance/guest/consumer.sh debian   bash /work/acceptance-inputs/consumer-lifecycle-valid.sh debian
bash tests/linux/run.sh acceptance/guest/consumer.sh debian   bash /work/acceptance-inputs/consumer-upgrade.sh
bash tests/linux/run.sh acceptance/guest/consumer.sh debian   python3 /work/acceptance-inputs/portable-consumer-valid.py
```

Use the corresponding `ubuntu` consumer for that row. Use fresh guest copies or
fresh fixture labels for a new run; do not reuse old runtime state as readiness
proof. The wrappers, build script, source verifier and artifact inspector are
included in the selected local evidence archive. They call the tracked drain-safe
lifecycle script and existing upgrade drivers. The wrappers add candidate-bound
ownership and extracted-byte assertions, not a release orchestration framework.
The evidence manifest lists each selected input/result/log and its hash. Package
assets remain only at the local paths in that manifest; they were not published.

## Preserved failures and qualifications

- An initial build command ran before cache copying finished and stopped at a
  missing Tauri command before compilation. The subsequent verified build passed.
- The first owned-registration fixture omitted the required description field.
  Ghostlight correctly preserved the malformed manifest; the wrapper's repair
  assertion was wrong. A fresh complete fixture passed. No parser was weakened.
- Ubuntu's first concurrent lifecycle attempt expired the tracked five-second
  readiness wait after registration reconciliation and before runtime publication.
  The failure and authority log remain local. A fresh serial run passed with the
  same product bytes and unchanged wait. Build/package I/O pressure is a possible
  explanation, not an established root cause; no startup fix is claimed.
- The first portable Codex setup omitted `--all-browsers` in a guest with no native
  browser. Its ordinary refusal was correct. Explicit preregistration in a fresh
  fixture completed the installer checks.
- The first installed smoke correctly refused unavailable Chromium with effect
  none. Its desktop launch had not selected a profile. A visible profile chooser
  confirmed that state; opening the already-existing Default profile recovered
  the original Chromium binding before the successful smoke. Both browsers had
  been closed at the beginning of this round. No browser restart/reboot continuity
  from the prior campaign is claimed.

## Installed state and remaining release gaps

The main-repository dev loop built into the existing isolated target and replaced
only the authority at `.tmp/fleet-test-01/target/release/ghostlight`. That native
CachyOS image has SHA-256
`d0ee844b26a0e032f3aab97a3063e1ed7eff7ad5425bf67c0694c4bab0a09030`.
It differs from the Ubuntu-baseline package image; the manifest separates them.
Both connector hashes, source adapter files, policy, native registrations and saved
Codex configuration remained unchanged. The existing source adapter matches the
integrated tree, so no reload was necessary. Normal Chromium is the native distro
binary; Brave is the previously extracted native distribution, not a distro package.

Final inspection found one installed authority, Ready with both browser bindings,
no remaining guest processes, and remote-debugging port 9222 closed. Smoke tabs
remain visible under the existing preserve-tabs preference. Neither credentials
nor browser permissions were changed. Preserve the live worktree path while it is
still the installed sibling location.

Remaining release gaps: CI build-provenance attestations for these candidates;
Ubuntu GNOME Wayland visible L1-L9 against exact package bytes and the matching store
adapter; submitted ZIP/clean-machine store acceptance; and the separately owned
sanitized-MCP startup/retry and Flatpak activation work. This lane did not reboot the
host, validate musl packages or add architecture/platform support. A later integrated
startup change requires a new candidate and its affected downstream package checks;
it does not justify repeating unrelated broad campaigns now.
