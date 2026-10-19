# Latest result

[0058] linux-claude, 2026-10-19: L2-02, the installers install from a local bundle.

- 422e4696 `feat(install)`: `GHOSTLIGHT_FROM=<folder>` in `get.sh` and `get.ps1`.
  - The folder is the release workflow's unpacked `release-candidate` artifact, or a folder of
    release downloads.
  - Each image is copied and the copy is checked against `SHA256SUMS` by the download rules.
  - Anything unbound, missing or different is refused before anything runs.
  - Nothing is fetched from GitHub. The GitHub CLI's provenance check still runs when present.
  - It cannot be combined with `GHOSTLIGHT_VERSION`. `--restore` and `GHOSTLIGHT_NO_REGISTER`
    work with it.
- Docs: both script heads, the installation guide ("Test a build before its release"), and
  RELEASE.md cutover steps 1 to 3, which now prove the install on the bundle, not on rc.1.
- Tests:
  - `tests/installer-shell.mjs` passes on test-01.
  - `tests/installer-powershell.ps1` gains the local cases; CI's Windows job runs them.
  - `get.ps1` was also run here in PowerShell 7.6.5 against stub images.
- Still naming rc.1, for windows-claude: `docs/release/store-disclosure-2.0.md`,
  `docs/release/notes-v2.0.0-rc.1.md` and STATUS's release order.
- For Part 2: the bundle's extension ZIP has no development key, so testers load `extension/`
  from the bundle's revision.
- Seen once on ubuntu CI (run 39253295760): `a_quit_answers_new_invocations_with_shutting_down`
  read a closed connection. It passed on the next run.
