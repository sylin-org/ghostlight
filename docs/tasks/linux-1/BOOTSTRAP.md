# Linux batch 1: ground rules

This batch runs on the owner's CachyOS installation (Arch-based Linux), by a Claude Code session
that calls itself `linux-claude`. The coordinator is the Windows session, `windows-claude`. The
owner set it up on 2026-10-17 to run every check the 2.0 work owes on Linux, which no Windows host
can run. [LEDGER.md](LEDGER.md) holds the tasks and is the record of what happened.

## Read first

1. [AGENTS.md](../../../AGENTS.md) (it binds this batch) and [docs/MEMORY.md](../../MEMORY.md).
2. The current-state top of [docs/STATUS.md](../../STATUS.md).
3. [docs/DEV-LOOP.md](../../DEV-LOOP.md) and [tests/linux/README.md](../../../tests/linux/README.md).
4. The break-2.0 ledgers where a task points (owed live checks): `docs/tasks/break-2.0/seams.md`,
   `shell-m2.md`, `journey.md` and `design.md`.

## How the two sessions talk

Through the repository, as [coordination/INSTRUCTIONS.md](../../../coordination/INSTRUCTIONS.md)
describes: numbered messages in `coordination/CHAT.md`, the latest result in
`coordination/RESULTS.md`, both on `dev`. Fetch before you start, and pull with rebase before every
push. If `git push` is rejected because the other session pushed first, fetch, rebase and retry;
never force.

## What this session may do

- Install the build prerequisites with pacman or an AUR helper (`rustup`, `nodejs`, `npm`,
  `webkit2gtk-4.1`, `dbus`, `powershell-bin`, `chromium` or `google-chrome`, `github-cli`).
- Build, run the full gate, run every journey, and install the development channel into its own
  `~/.ghostlight` with `pwsh scripts/dev-loop.ps1`. This machine is a test machine: journeys and
  temporary test homes are fine here.
- Load the extension unpacked from `extension/` into the system Chromium's ordinary profile for
  the browser checks. The owner directed on 2026-10-17 that acceptance never uses a sandbox: no
  bubblewrap home, no dedicated or throwaway browser profile, always the real installation and
  the real system browser.
- Commit and push to `dev` as the owner (docs/MEMORY.md, "Coordinated machines may commit and push
  as the owner").

## What this session must not do

- No product code changes. Report a failure with its evidence in the ledger and the chat; the
  coordinator fixes it and pushes, and this session re-runs the check. The one exception is a
  Linux-only test or journey that is plainly wrong about Linux (for example a Windows path baked
  into a Linux test): fix it in its own commit, say so in the ledger, and tell the coordinator.
  Amended by the owner on 2026-10-17: this session may also investigate the issues it finds and
  land proper fixes, product code included, under AGENTS.md's rules (read the subsystem's ADRs
  first, run the checks for what changed, one logical change per commit). It claims each
  finding in the chat before it starts, so the coordinator does not fix the same one.
- Never publish, tag, release, submit to a store, or post anywhere. Never phone home.
- Never weaken a test to make it pass.
- Never read or publish `local/`, `private/` or `saps/`.
- Stage only your own paths (`git add -- <paths>`); the coordinator commits in the same branch.
- Never add, edit or delete `docs/testing/independent-user-delight-2026-10-02.md` or
  `docs/testing/quiet-success-local-deployment-2026-10-02.md` if they exist.
- ASCII only in every file you write; human-plain words; sign off every commit
  (`git commit -s`) and end its message with your own model's co-author trailer.

## Recording

For each task: set its status, write what you ran and what you saw (pass or fail, the exact
failing assertion or error, versions), and commit the ledger with
`docs(linux-1): <task> <result>`. Then post one chat message for a batch of results, not one per
task. Keep secrets, page content, screenshots and credential values out of every file.
