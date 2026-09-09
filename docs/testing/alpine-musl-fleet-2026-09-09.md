# Alpine native-musl fleet acceptance -- test-03

Starting source: `a8cfd0337d83079f5fad3d5dfec846c31ba3b8fd` (service 1.3.5,
source adapter 1.1.2). Local branch: `codex/fleet-test-03`.
This record is native-musl feasibility and installed development acceptance,
not a declaration of Alpine support or a release attestation. The initial
`main` checkout at `48ef29e` was left unchanged.

## Machine and delivery boundary

- Alpine Linux 3.24.1, x86_64, musl 1.2.6-r2, kernel 6.18.48-0-lts.
- Host PID 1 is init; OpenRC, real KDE Plasma 6.6.6 Wayland session.
  The initial tool container boundary was identified before host acceptance.
- No glibc loader or compatibility layer was installed.
- Root package mutation is blocked: sudo and doas require authentication.
- Chromium 152.0.7977.82 came from signature-verified Alpine Chromium,
  chromium-common and crc32c APKs, extracted into a user-owned directory.
  This is a native musl executable and a real desktop browser, not an APK
  database installation. It uses its ordinary default profile, normal sandbox,
  Wayland, and a user Applications entry. The source extension is explicitly
  loaded unpacked; store-extension acceptance is not claimed.

The published `npx -y ghostlight@1.3.4 install` downloaded and verified all three
siblings, then failed with spawn ENOENT. The files exist, but their ELF interpreter
is `/lib64/ld-linux-x86-64.so.2`, which is absent. This is an unsupported ABI,
with an unhelpful consumer error. No published install success is claimed.

The default native musl build reached the authority link and failed because
static GTK/GLib/intl libraries are absent. A native dynamic build passed:

```sh
RUSTFLAGS='-C target-feature=-crt-static' cargo build --workspace --locked \
  --target x86_64-unknown-linux-musl --target-dir <isolated-target>
```

Rust is 1.95.0, host `x86_64-unknown-linux-musl`. The authority uses
`/lib/ld-musl-x86_64.so.1`. These are unoptimized, unstripped debug binaries.
All three siblings live in one explicit installed user directory. Deliberate
`ghostlight install` created the actual native registration, command,
Applications entry and Codex registration. Connector demand-start worked on OpenRC
without a system service. No machine-local notes or private project material were read.

## Results before reboot

| Boundary | Result | Evidence file or directory |
| --- | --- | --- |
| Candidate native build, fmt, Clippy, Rust | PASS; 536 Rust tests before fix | `native-*`, `candidate-format.log` |
| Candidate extension, workbench, npm, policy grammar | PASS; 210 extension tests | `candidate-*.log` |
| Installed full browser journey | PASS; 23 tools, 15 check groups | `installed-live-journey.json` |
| Native workbench Open/minimize/Open/close/Open/concurrent Open | PASS; exact KWin window/PID evidence | `desktop-lifecycle.json` |
| Human navigation, overlapping wait, child tabs, moved tabs | PASS; no passive audit/hold, no child grouping/debugger adoption, no regrouping; next agent read governed | `human-boundary.json` |
| Real popup pause/resume/stop/new-session handlers | PASS | `human-boundary.json` |
| Installed registration and connector/authority/worker recovery | PASS on rerun; original failure retained | `recovery-diagnostic/`, `recovery-fixed/` |
| Interrupted browser effect | PASS; effect count 1, outcome unavailable, no replay | recovery results above |
| Full normal-browser exit and Applications relaunch | PASS; stable adapter identity, unchanged authority | `browser-restart.json` |
| Repeated real uninstall/reinstall | PASS; audit byte preservation, owned manifest restored exactly, unrelated sentinel retained | `install-lifecycle.json` |
| Two real Chromium profiles and workspace isolation | PASS; absent profile does not switch into survivor | `plural-profiles.json` |
| No-tray private D-Bus session with actual KDE windows | PASS for close/Open; supplemental context | `no-tray-result.json` |
| Useful WebView accessibility tree | NOT ESTABLISHED; application/frame visible only | `no-tray-result.json` |
| Signed managed authority in isolated native browser | PASS; seven checks including fail-closed, cache and rollback | `managed-policy.json` |
| Actual Codex application browser call | BLOCKED by client's enforced approval policy | `actual-codex-*` |
| Actual OpenCode 1.18.30 with opencode/big-pickle | FAIL before schema fix; PASS browser read/fill/click/read after fix | `opencode-*-events.jsonl`, `opencode-dom-proof.json` |
| Actual MCP Inspector | Catalog passed; call FAIL before schema fix; PASS afterward | `inspector-*.json` |
| Native portable debug prototype | PASS member-byte verification; no APK install or provenance | `native-package.json` |
| Reboot | Pending at this checkpoint | `reboot-*.json` when completed |

Synthetic boundary drivers exercise the actual installed MCP connector, authority,
registered native host, extension and visible browser. They are not counted as
real AI-client acceptance. External browser manipulation uses the browser APIs;
this is deterministic human-boundary coverage, not a physical human participant.
Managed-policy and no-tray cases use explicitly isolated contexts; neither proves
the normal user's managed deployment or a different desktop shell.

OpenCode initially sent `new_tab` as the string `True` three times. These inputs
were refused before effects. It recovered by omitting that optional field and
completed the requested task. Independent DOM inspection found the synthetic
`OPENCODE-MUSL-PASS` draft and count `1`. Do not label this a first-attempt model pass.

## Local fixes

### Output schema must include the actual storage receipt

Both MCP Inspector and OpenCode rejected otherwise successful actual installed
results with `data must NOT have additional properties`. The closed output schema
omitted `history_storage`, which `InvocationResult` always serializes.

The fix adds the existing `Storage` enum to the orchestrator-owned common output
schema and required fields. The active language contract now documents it.
It keeps `additionalProperties: false`; no client-specific workaround, connector
change, extension change, or permission change was introduced. The Rust regression
compares actual serialized result fields with every advertised output schema and
covers both saved and unconfirmed storage. Both real clients accept the deployed fix.

Validation: formatting, warnings-denied native-musl Clippy, all 537 Rust tests,
210 extension tests, eight driver fault tests, JavaScript syntax and workbench
surface pass. The authority alone was deployed; connector and adapter bytes remain
unchanged. PowerShell 7.6.6's official native-musl archive was digest-verified.
A local copy of the existing dev-loop script adapts its repository/install roots
and explicit musl target path for this campaign; deployment lock, exact-image
process selection, bounded copy and normal startup are retained.

### Recovery reporter must preserve a protocol failure

The first installed recovery run passed registration and native-host crash phases,
then its reporter threw a TypeError in the authority-restart phase. JavaScript's
regex test coerced undefined to the matching string `undefined`, then the true
branch dereferenced absent facts. The underlying transient reply was lost; its
cause remains unresolved and must not be claimed fixed by the reporter correction.

Commit `98b7e93` adds an explicit string guard and records only a numeric protocol
error code. Injected missing-result and protocol-error regressions exercise the
actual reporter without starting processes. Private error text is not retained
in the report. The normal five-phase recovery journey passes after the fix.
The prior diagnostic rerun also passed; neither pass erases the initial failure.

## Artifact identities and evidence custody

Baseline candidate binary SHA-256:

| Binary | SHA-256 |
| --- | --- |
| authority | `03114683d1f422d066477aa8ddff56deafe308a31a59dacaacc12f5b0a1ea4e6` |
| browser connector | `4a148d448f08f68bfe4c7f44dc2074409efc7577b8dd84e81f8fcb3a70bfc2ed` |
| MCP connector | `fe77f4cfcc30e7aad28898d9e1b7dac13ed23861e07470a93d2a8e6bcb55177d` |

The fixed authority is
`f05fbeac3eff53d2e72229ea3b8a834f9352ff3d4e051a365986a9a38a369510`.
The two connectors retain their baseline hashes. Source adapter hashes are in
`source-adapter-sha256.json`. The baseline native debug archive is
`84098468fbe8aa93e1f52ac2e7e97d6ec80f098e0f6927385cf68d9193547c7c`.
A fixed-source archive must name its resulting commit separately.

Machine evidence root: `/home/test/ghostlight-fleet/test-03/`. Logs and reports are
under `evidence/`; recovery outputs are in their named sibling directories.
Only sanitized reports belong in transfer bundles. Runtime documents, browser
profiles, generated signing seeds, and Git credentials must never be bundled.

Failed fixture attempts are retained: stale APK mirror downloads, missing native
browser library path, numeric rather than string wait input, and managed-context
home masking of prerequisites. The managed rerun used exact copied source assets
and native browser bytes outside that masked home. It did not modify host policy.

Remaining lane limits: root APK/package-manager lifecycle, a second Chromium brand,
store adapter, useful WebView accessibility, Codex's enforced approval block,
three distinct real AI applications, and reboot evidence. Published GNU binaries
cannot establish Alpine upgrades/downgrades. The native debug prototype is not a
new supported release channel. Broader fleet and release approval remain separate.
