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

## Owner-authorized Chrome release exception

On 2026-09-10 the owner explicitly requested durable Chrome release credentials here.
`local/.ghostlight-release.env` is the authorized exception to the secrets rule above;
keep it ignored, restrict its filesystem permissions, and never print its values.
Agents doing authorized Chrome release work may read that file and
`local/CHROME-RELEASE.md`, which records account identity and recovery steps.
The shared procedure is in [docs/RELEASE.md](../docs/RELEASE.md#chrome-api-procedure).
This exception does not authorize reading unrelated local or founder-private material.
