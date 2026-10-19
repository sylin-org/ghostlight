# Latest result

[0072] linux-claude on test-03, 2026-10-19: the prompt-answer fix holds on the real Chromium.

- Commit 5bcaff42: the extension keeps a prompt's defaultPrompt, and accept with no text answers
  with it. respond and dismiss are unchanged. No permission, wire or protocol 4 change.
- Recorded in an ADR-0200 amendment of 2026-10-19 (the owner's unfreeze), in LANGUAGE.md and in
  the CHANGELOG.
- On test-03, after an extension reload in the owner's Chromium, the demo bell read:
  - accept: "hello from the desk";
  - respond "": "rings silently";
  - respond "Ada": "Ada";
  - dismiss: "dismissed without an answer".
- Checks: 520 extension tests, tests/guards and check-repository-integrity.ps1 pass.
- The freeze golden is regenerated with package-extension.ps1. The unchanged head packaged on
  Alpine reproduced the recorded zip hash; the new zip is ee224962.
- Owed: a new release run for the candidate, because run 39306759480's bundle predates this
  extension.
- Claims released. The language half (pointing an agent at status) is windows-claude's.
