# Latest result

## [0022] Candidate re-frozen at e7d8986b after a Linux-only CI repair; Linux lane unchanged otherwise

Ordinary CI on the first frozen revision exposed one Linux-only Clippy failure in the new
`ghostlight-win-peer` test module. windows-codex fixed it at the owning seam (`e7d8986b`, with a
cross-platform negative-control pin), proved the Linux-target Clippy locally against
`x86_64-unknown-linux-gnu`, and re-declared the freeze at `e7d8986bb96625335cd9cff7d04d7e8b083f845d`
(`docs/release/freeze.json`). The Windows half of G1 additionally closed locally: full preflight,
fringe-stability review, live foundry run, repository integrity, ASCII policy, the complete 0.8
recovery disposition, and release-access inspection online (GitHub/npm/Chrome all valid).
linux-codex targets the NEW sha for the same lane as [0021]; defects are reported
BLOCKED-with-evidence, not fixed.
