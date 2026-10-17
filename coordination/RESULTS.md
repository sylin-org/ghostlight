# Latest result

## [0027] linux-1: the machine is ready and the full gate passes on Linux

linux-claude, on test-01 (CachyOS rolling, kernel 7.2.9, KDE Plasma 6.8 on Wayland), at dev
17a0c060. Details are in [the ledger](../docs/tasks/linux-1/LEDGER.md), L01 and L02.

- L01 pass. rustc and cargo 1.95.0, node 22.22.1, pwsh 7.6.5 (the extracted build in
  `.tmp/linux-tools`, linked into `~/.local/bin`; this session has no sudo password for pacman),
  Chromium 154, webkit2gtk-4.1 2.54.1, Zed 1.24.3. The 1.x-era home, the Chromium host manifest
  and 14 agent files that named a deleted fleet build are backed up under ignored `.tmp/linux-1/`.
- L02 pass. fmt and clippy clean; `cargo test --workspace` 2251 passed, 0 failed (the writer
  golden with `native/linux.txt`, the Codex forwarding, the agent list, the 16 icon tests and the
  private-file tests all ran on Linux); extension 492, page assets 270, workbench 140, workbench
  surface ok, npm 26, guards 10, `sh -n` and the integrity script clean.
- One wording fix for the batch, no product change: `node --test tests/guards/` fails on Node 22
  (a directory is read as a module path). The glob `node --test "tests/guards/*.test.mjs"` is what
  ran.

Next: L03, the journeys, from a fresh target directory.
