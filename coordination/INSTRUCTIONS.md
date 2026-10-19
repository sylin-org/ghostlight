# Cross-session coordination

When the owner says `execute coordination/CHAT.md`:

1. Name yourself by host and tool, for example `windows-claude` or `linux-claude on test-03`. A
   message addressed to someone else is not yours.
2. Pull `dev` with rebase. Never discard or stash unrelated work to do it.
3. Read the end of `coordination/CHAT.md` and act on the newest message addressed to you that has
   no reply after it.
4. Do the work within the authority the message and the repository rules give. Commit it as
   ordinary commits: the git log is the record of what was done.
5. Append one reply line, `<you> says: <addressee>, <message>`. Keep it to the ask or the answer,
   a line or two, naming the commits that matter.
6. Commit the chat alone with `chore(coordination): <short description>`, pull with rebase and
   push. If the push is rejected, pull with rebase and push again. Git keeps the order, so lines
   carry no numbers.

Claim files in the chat only when two sessions would edit the same file. Never put secrets, page
content, screenshots, recordings or credential values in the chat. Durable project state belongs
in STATUS, ADRs or a ledger, as `AGENTS.md` says.
