# Linux batch 2: ground rules

The owner wants to release 2.0. The order is: get everything ready, test on CachyOS, then the
owner tests on Bluefin and Alpine, then 2.0. There is no rc.1 tag; "release candidate" is only
our name for the build the release workflow makes before the tag (the owner, 2026-10-19). This
batch is the Linux part. It runs on the owner's CachyOS installation, by the Claude Code session
that calls itself `linux-claude`. The coordinator is the Windows session, `windows-claude`.
[LEDGER.md](LEDGER.md) holds the tasks and is the record.

The owner, 2026-10-19: "don't try to solve the problem here on this windows machine. just
delegate the task to the corresponding agent." So the Linux problems below are this session's to
solve, not only to test.

## Read first

1. [AGENTS.md](../../../AGENTS.md) and [docs/MEMORY.md](../../MEMORY.md).
2. The current-state top of [docs/STATUS.md](../../STATUS.md).
3. [docs/DEV-LOOP.md](../../DEV-LOOP.md), [tests/linux/README.md](../../../tests/linux/README.md)
   and the [linux-1 ledger](../linux-1/LEDGER.md), which is what this machine already proved.
4. [docs/RELEASE.md](../../RELEASE.md), the 2.0 cutover, and `.github/workflows/release.yml`.
5. The September Alpine records: [docs/testing/alpine-musl-fleet-2026-09-09.md](../../testing/alpine-musl-fleet-2026-09-09.md)
   and [docs/testing/alpine-combined-musl-2026-09-09.md](../../testing/alpine-combined-musl-2026-09-09.md).
6. [ADR-0203](../../adr/0203-one-owner-for-each-fact.md), which the Windows session is landing now
   as batch arch-1, and its question about the Claude Code plugin (the owner's answer is in L2-04).

## How the two sessions talk

Through the repository, as [coordination/INSTRUCTIONS.md](../../../coordination/INSTRUCTIONS.md)
says: numbered messages in `coordination/CHAT.md`, the latest result in
`coordination/RESULTS.md`, both on `dev`. Fetch before you start and pull with rebase before
every push; if the push is rejected, fetch, rebase and retry, never force.

Claim files before you edit them, in the chat. The Windows session's arch-1 lanes are still
landing in `crates/orchestrator` and `crates/desktop`; the files this batch owns are listed in the
ledger. If a fix needs a file outside them, claim it in the chat first and wait for the
coordinator's answer.

## What this session may do

- Install build and test prerequisites with pacman or an AUR helper, podman or distrobox, and
  Flatpak with Flathub.
- Change product code, workflows, scripts and docs to solve this batch's tasks, one logical change
  per commit, each with its tests, and push to `dev` as the owner (docs/MEMORY.md, "Coordinated
  machines may commit and push as the owner").
- Build, run the full gate, run every journey, install and uninstall Ghostlight in this machine's
  own home. This machine is a test machine: journeys and temporary test homes are fine here.
- Load the extension unpacked into the system browser's ordinary profile (the owner, 2026-10-17:
  acceptance never uses a sandbox or a throwaway profile).
- Dispatch the release workflow (`gh workflow run release.yml --ref dev`) and download its
  artifacts. It builds and checks; it never tags, signs or publishes.

## What this session must not do

- Tag, publish, release, or submit anything to a store or registry. Those are the owner's.
- Rewrite history or force-push.
- Read `local/`, `private/` or `saps/`.
- Weaken a test, a guard or a public claim to make a check pass.
