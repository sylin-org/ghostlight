# Latest result

## [0029] linux-claude takes findings 1 to 7 of [0028]

The owner authorized linux-claude on 2026-10-17 to investigate the findings below and land
proper fixes, product code included, and to remove all bubblewrap content (done in dabde403).
linux-claude now works findings 1 to 7; finding 8 stays a note. The batch BOOTSTRAP records the
amended authority.

## [0028] linux-1: L03 and L04 pass; real install on test-01; findings for you

linux-claude, on test-01. Details and evidence are in
[the ledger](../docs/tasks/linux-1/LEDGER.md), L03 and L04.

Owner directions given during this run (2026-10-17):

- Fine tune or remove tests that are misaligned, overly eager or bureaucratic.
- Sandboxed versions are false integration tests; always use the real system browser. Recorded
  in docs/MEMORY.md ("Full integration means the installation in place") and the batch
  BOOTSTRAP.

What ran:

- L03: the hardening suite's process lane (11 journeys) and browser lane (5) pass, the live
  sylin.org frame journey passes (56 checks), and `tests/linux/startup-recovery.mjs` passes.
  These are regression gates, not integration evidence.
- The `tests/linux/acceptance` drivers (bubblewrap homes, dedicated browsers) ran before the
  owner's direction. Test-only commits: 2de3e190 (the sandbox now mounts a working /dev),
  71465b67 (startup-check retired, superseded by ADR-0199 Decision 11 and startup-recovery),
  d4d628a7 (the embedded-pixel samples avoid the frame's own input). They no longer count as
  evidence. Deleting them waits for the owner's explicit word; this session's permission check
  refused the deletion.
- L04: `pwsh scripts/dev-loop.ps1` installed the development build into the real
  `~/.ghostlight`, as its plan said. status, doctor, help, `call --help`, the c11 id selection
  and refusal, and the man page pass. Completions work in fish and bash. The system Chromium on
  its ordinary profile connected on its own; readiness Ready.

Findings for you (product code is yours; nothing changed):

1. After a lost service (SIGKILL), the MCP edge fails the first request with -32603 "Ghostlight
   restarted before this request finished.", even a `tools/list` sent after the loss; the next
   request works. `tests/linux/installed-journey.mjs` fails there in 4 of 4 runs; its earlier
   phases pass.
2. When the service cannot start, the installer always says "did not start within 30 s" and
   waits the full 30 s, also when the spawn failed at once or the service exited at once.
3. A service whose start fails (for example an endpoint path over the 108-byte socket limit)
   writes the cause only to its null stderr; the diagnostics log stops at "service starting".
4. `install --dry-run` prints Rust variant names: "(NeedsAttention)", "(Installed)",
   "(NotDetected)".
5. `install --client nope` prints the whole dry-run summary before refusing.
6. The zsh completion goes to `~/.local/share/zsh/site-functions`, which zsh does not search by
   default here.
7. 1.x leftovers neither removed nor named by doctor: `~/.ghostlight/adapters/v1.1.1`, three
   `~/.ghostlight/extension/v0.*`, and two 1.x connector man pages.
8. Cline (CLI)'s 0.5.7-era entry (`transport.command`) is read as foreign and left alone.

Next: L05, the window and the tray, on the real install.
