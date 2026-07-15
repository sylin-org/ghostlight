# Cross-session coordination

When the owner says `execute coordination/CHAT.md`:

1. Identify yourself as `windows-codex` or `linux-codex` from the host OS.
2. Run `git fetch origin dev`. Check whether local `dev` is stale before trusting the local chat.
   Fast-forward when safe. Never discard, stash, or rewrite unrelated work merely to sync chat.
   If a dirty tree prevents a safe update, read the current files with
   `git show origin/dev:coordination/CHAT.md` and
   `git show origin/dev:coordination/RESULTS.md`, then reconcile before replying.
3. Read `coordination/CHAT.md` and `coordination/RESULTS.md`. Act on the newest message addressed
   to you that has no later reply.
4. Do the requested work within the authority stated in the message and the repository rules.
5. Replace `coordination/RESULTS.md` with the latest concise result. Append one reply to
   `coordination/CHAT.md` using the next four-digit message number and this exact shape:

       [0002] linux-codex says: windows-codex, <message>. See coordination/RESULTS.md.

6. Commit task changes separately when they are ready. Then commit only the coordination files
   with `chore(coordination): <short description>` and push `dev`.
7. If the push is rejected because another message landed first, fetch, preserve both messages in
   numeric order, use the next unused number, and retry.

Keep `CHAT.md` to communication only. Keep `RESULTS.md` to the latest result only. Put durable
project state in STATUS, ADRs, task ledgers, or `local/` as required by `AGENTS.md`. Never place
secrets, page content, screenshots, recordings, or credential values in coordination files.
