# Latest result

## [0030] linux-1: the findings of [0028] are fixed, pushed and proven on the real install

linux-claude, on test-01, under the owner's 2026-10-17 authority to fix what it finds. Every
solution is judged by delight, and in an MCP the user to delight is the model (now in
docs/MEMORY.md). Details: [the ledger](../docs/tasks/linux-1/LEDGER.md), L04.

Product fixes on dev, each with its tests, fmt and clippy, deployed with the dev loop into the real
`~/.ghostlight` and checked there with the system Chromium:

- eaf7948d `fix(edge)`: a line whose write failed is held; a lone `initialize` or catalog listing
  cut off by a lost service or a `replace` goodbye is sent to the next service once. Tool calls are
  still answered "may have taken effect" and never resent; after quit nothing is held. ADR-0197
  has a marked amendment; relay-protocol-4, ARCHITECTURE and ACCEPTANCE say the same. Relay 4 is
  unpublished, so the source lock does not apply. The edge was force-replaced on test-01.
- bd29366c `fix(install)`: a failed start writes its reason to `run/start-failure` and
  diagnostics; the installer stops waiting once no process runs the service image and names the
  spawn error, the recorded reason or the timeout.
- d6c55ef3 `fix(install)`: the dry run says what each client would become, in plain words.
- 1e67edee `fix(install)`: a mistyped `--client` or `--browser` is refused before anything stops,
  changes or prints (it used to register browsers and write the Applications entry first). The CLI
  journey pins it.
- 00c6eccf `fix(install)`: the two 1.x connector man pages are retired by install and uninstall.
- aadf76e6 `docs(install)`: the man page gives zsh's one `fpath` line; the retired Debian package
  is no longer named.
- 22f882db and ad43f65c `fix(diagnostics)`: `diagnostics show` merges the edge's own logs, a full
  edge log is rotated instead of going silent for good, and a component filter no longer reports
  the others as off.

Test changes: 9c439606 aligns the installed journey (it waits for the racing start to give way and
no longer needs a dedicated browser); dabde403 removed every bubblewrap driver, as the owner
authorized. `tests/linux/installed-journey.mjs` now passes 4 of 4 phases on the real install.

Not changed: finding 8 (Cline CLI's old `transport` entry stays foreign). The 5 s wait of a start
that loses the lock is by design (`lifetime.rs`).

Next: L05, the window and the tray.
