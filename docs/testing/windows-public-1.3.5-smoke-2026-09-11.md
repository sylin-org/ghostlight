# Windows public 1.3.5 smoke

## Scope

This is an independent Windows consumer smoke of the already-published Ghostlight
1.3.5 npm package. It ran on leo-desktop-02 on 2026-09-10. It does not rebuild,
change, or repackage the repository source. The consumer environment lived below
the ignored repository directory `.tmp/public-windows-smoke-20260910`.

The consumer test was intentionally narrow. It verifies npm publication, public
binary retrieval, public installation, doctor output, MCP connector startup, and
connector cleanup. It does not verify a browser action, extension installation,
the Chrome Web Store adapter, a saved MCP-client registration, a reboot, or
uninstall behavior.

## Published identity

```
npm view ghostlight version --json
npm view ghostlight@1.3.5 dist.tarball --json
```

Results:

- Public npm `latest` is `1.3.5`.
- The resolved package tarball is
  `https://registry.npmjs.org/ghostlight/-/ghostlight-1.3.5.tgz`.
- `npx -y ghostlight@1.3.5 --help` downloaded and verified the authority, MCP
  connector, and browser connector for Windows x64, then identified itself as
  Ghostlight 1.3.5.

## Isolated install and doctor

The consumer commands ran with `APPDATA`, `LOCALAPPDATA`, `USERPROFILE`, and the
npm cache directed at the ignored test profile. The command form was:

```
npx -y ghostlight@1.3.5 install --dry-run --no-clients --no-open
npx -y ghostlight@1.3.5 install --no-clients --no-open
```

The dry run exited 0 and correctly identified existing per-user browser
registrations owned by the prior installed Ghostlight. The actual install exited
0, left MCP client configuration unchanged, and placed all three public binaries
in `profile/.ghostlight/bin/v1.3.5`. Its public binary hashes were:

| Binary | SHA-256 |
| --- | --- |
| Authority | `dbf449602e92a612ca9218d1c2271d6fc260bce25b3fff573d3383c995c49abe` |
| MCP connector | `1cc049219b5c2656818d1a37d928823140f1eba5a59c21dc45fb5e85d1d0aac4` |
| Browser connector | `66b2066acaff2f05e1aa9d679076eced15a6440ab36b4dbb779d53a4ceb73283` |

The installed public authority ran:

```
ghostlight doctor --json
```

Doctor exited 0. It reported all three v1.3.5 binaries ready, the isolated
connector path, all four browser families current, no running service, and no
saved MCP client configuration changed by the install.

## Public connector startup and parent exit

The public MCP connector was launched directly from the isolated v1.3.5 directory
with the same isolated profile environment. A small stdio client sent JSON-RPC
`initialize`, sent `notifications/initialized`, requested `tools/list`, then
closed its stdin. No browser tool was called.

| Check | Result |
| --- | --- |
| Initialize protocol | `2025-03-26` |
| Catalog result | 23 tools |
| Connector PID | 6520 |
| Connector exit | Exit 0 |
| Time from client closure to exit | 1 ms |
| Connector after client exit | Absent |

The connector demand-started one isolated authority, PID 17724. That authority
is the intended desktop authority, not a connector orphan. It was stopped by its
verified exact isolated executable path during test cleanup. The final isolated
process inventory was empty. An earlier identical exchange also had connector
PID 15736 exit 0; its isolated authority PID 9248 was cleaned up the same way.

## Per-user registration limitation and restoration

The consumer profile isolates files and runtime state, but Windows native-messaging
registration is per-user. The actual public install therefore temporarily repointed
Chrome, Edge, Brave, and Chromium registrations to its isolated browser connector.
This was not visible in the dry-run ownership report. The prior installed
authority's supported `native-host install` command restored the four entries,
and each was then verified to point to:

```
C:\Users\leo\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\Ghostlight\NativeMessagingHosts\org.sylin.ghostlight.json
```

No user browser profile, saved MCP configuration, or unrelated process was
removed. The final audit found no process running from the isolated public
installation. The registry scope means a truly side-by-side browser-registration
consumer test needs a separate Windows user account or VM; this host can isolate
the files and process identities, but not that registration boundary.

## Evidence

Ignored evidence remains under `.tmp/public-windows-smoke-20260910`, including
the dry-run and install output, doctor JSON, connector protocol results and
disconnect timing, registration restoration, and final cleanup audit. No smoke
command was repeated while preparing this record. No repository runtime file was
changed, and no public action was taken by this evidence publication.
