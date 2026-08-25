# CONTINUATION: Ghostlight 1.0 release -- G2 held, G3 next

Read AGENTS.md first, then docs/STATUS.md ("In flight" section), then this prompt. You are
continuing in F:\Replica\NAS\Files\repo\github\sylin-org\browser-mcp, branch `dev`.

## Where things stand (all verified 2026-08-25, tree clean at/after 1f7367c4)

The 1.0 candidate is BUILT and CUSTODY IS HELD:

- Build: release workflow run 32846030216, all jobs green, at candidate revision
  `994b6c85dcd7c8df74237cf329461d85ce49b13a`. Product bytes are identical to `8779e11b`
  (the ADR-0137 feature commit); the delta between them is CI tooling only.
- `docs/release/freeze.json` pins `994b6c85...` per the owner's explicit decision: the freeze
  ceremony is DROPPED ("we publish when we're done"). The freeze machinery only pins whichever
  revision becomes the candidate. Do not re-impose change-ban rules.
- Custody: two verified local copies under repo-local `.target-g2-custody/release-candidate`
  and `.target-g2-custopy-copy` (note: second dir name is `.target-g2-custody-copy`).
  `scripts/verify-custody.ps1 -IncludeProvenance` passes all five steps against BOTH copies
  (freeze binding, deep checks, SHA256SUMS rehash of 18 assets, GitHub provenance on all six
  raw binaries). Evidence: docs/testing/candidate-custody-2026-08-25.md.
- G2 is fully ticked in docs/RELEASE-CHECKLIST.md. Release state: G0-G2 done; G3 next.
- Candidate contract is now 18 artifacts / five SBOMs (win-peer joined the workspace);
  every checker agrees (check-release-candidate, verify-custody, release.yml, assemble script).
- The extension ZIP in the candidate is `ghostlight-extension-v1.0.0.zip`, SHA-256
  `9ae88e6729c830a9871802a39a2301c27c1d2baa00a2213332c310a7746a6db8`. It deliberately does NOT
  match the `f7b9a6ad...` bytes under the pending Chrome Web Store review: ADR-0137 (tab/group
  reuse) changed the service worker after that submission. The stale review must be REPLACED.

## ADR-0137 context (shipped, live-proven)

The owner reported tab/group spam; ADR-0137 landed: duplicate same-title groups merge into the
canonical one (self-healing), plain `browser_navigate` opens adopt the nearest unbound
same-host tab (`reuse: "domain"` default; `new_tab:true` and `reuse:"never"` create fresh;
stale-handle recovery always fresh), and the summary says "Reused the sylin.org tab." when it
happens. A refused release-close unbinds the tab so it becomes adoptable. Foundry runs green
with the open beat printing "Reused the sylin.org tab." The unpacked extension on the dev
machine is RELOADED and current.

## Standing owner decisions (do not re-litigate)

- Freeze ceremony dropped; publish when done. Gates are verification, not change-bans.
- Store mutations still need explicit owner authorization at the moment of the action.
- Never: main merge, version tag, publish, npm publish, store publish, phone-home, reference/
  copying, /private/ or saps/ reads, local/ reads beyond release credential LOCATIONS
  (~/.ghostlight-release.env holds values; never print them).

## Your tasks, in order

1. G3 -- replace the stale store review (ASK OWNER FIRST; they have authorized this pattern
   twice before). Use scripts/publish-extension.ps1 (CWS API; chrome.google.com is unscriptable)
   to upload the candidate's exact ZIP
   (.target-g2-custody/release-candidate/assets/ghostlight-extension-v1.0.0.zip,
   sha256 9ae88e67...) as the existing item's 1.0.0 draft and submit STAGED_PUBLISH, replacing
   the stale f7b9a6ad review. Record a dated testing doc. Tick G3's upload/submit rows.
2. G4/G5/G7 environment lanes (Ubuntu GNOME Wayland, clean Windows, public harnesses) are
   owner-run machines -- prepare instructions, do not improvise environments.
3. G8/G9/G10 (accessibility matrix, publication adapters in plan mode, tag/release/publication)
   are ALL owner-authorization boundaries. Draft, then wait.
4. Keep docs honest: STATUS.md "In flight" still says "pending: owner reloads the unpacked
   extension ... then the G2 candidate build" -- that is STALE. Update it to: custody held at
   994b6c85, G3 store resubmission next, environment lanes after.

## Gotchas learned this session (do not rediscover)

- ubuntu:24.04 docker image strips /usr/share/man via dpkg excludes; the lifecycle smoke
  strips the excludes before install (scripts/check-debian-package-lifecycle.sh).
- The candidate contract is 18 artifacts / five SBOMs -- check-release-candidate.ps1,
  verify-custody.ps1, release.yml, and assemble-release-candidate.ps1 all agree; do not
  "fix" the count back to 17.
- verify-custody.ps1 resolves SHA256SUMS names against assets/ (with root fallback).
- After ANY extension JS change, the unpacked extension must be manually reloaded in
  chrome://extensions (owner action; protected origin).
- `ghostlight call` sessions are keyed by the PARENT process: two calls from one shell share a
  workspace; a fresh cmd/pwsh parent is a fresh workspace.
- The release workflow's quality gate runs check-repository-integrity.ps1, which validates
  every local doc link -- relative links inside coordination/*.md must start with ../.
- Windows-local gates cannot see Linux-only Clippy failures; for cfg-split code run
  `cargo clippy -p <crate> --target x86_64-unknown-linux-gnu --all-targets -- -D warnings`.

## Verification commands

git status --short                          # must be empty
git log --oneline -5                        # HEAD 1f7367c4 or later (docs only beyond 8779e11b product)
pwsh scripts/assert-freeze.ps1              # pins 994b6c85...
pwsh scripts/verify-custody.ps1 .target-g2-custody/release-candidate -IncludeProvenance
Select-String -Path docs/RELEASE-CHECKLIST.md -Pattern '^- \[ \]'   # G3+ rows only
