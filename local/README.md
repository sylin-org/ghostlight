# local/ -- machine-local state and working notes

Everything in this directory except this README is **gitignored**. It holds facts that are
true for one machine or one person, useful to any agent or human working locally, and wrong
to publish:

- `MACHINE-STATE.md` (suggested convention): which engine currently holds the endpoint,
  install/registration state, native-host and MCP-entry paths, local gotchas, leftover
  artifacts. Agents should read it on session start (see [AGENTS.md](../AGENTS.md)) and
  update it when they change machine state (installs, engine swaps, deletions).
- `NOTES.md` (suggested convention): the sensitive/working half of the cross-agent memory --
  owner and working context, credential *locations*, and session handoffs. Any local agent may
  read and update it. Its non-sensitive counterpart is the tracked `docs/MEMORY.md`.
- Scratch notes, session handoffs, personal to-do lists tied to this checkout.
- *Locations* of credentials (e.g. "npm token lives in ~/.npmrc") -- never credential
  values themselves.

What does NOT belong here:

- Anything load-bearing for the project: decisions go in `docs/adr/`, project state in
  `docs/STATUS.md`, batch progress in the batch `LEDGER.md`.
- Founder-personal material (legal, entity, financial planning) -- that lives in the
  separately gitignored `/private/`.
- Secrets. Not even gitignored ones; this directory is covered by whatever backs up the
  working tree.
