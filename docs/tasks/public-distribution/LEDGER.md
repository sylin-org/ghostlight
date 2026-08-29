# public-distribution LEDGER

Durable progress for the public-distribution batch. One task = one commit. This file, not the
BOOTSTRAP's task list, is the authority on where the batch stopped.

## RESUME HERE

The batch's local work is complete through D1, and the owner-authorized send-day happened on
2026-08-29 (see the Outcomes section of `submissions.md`): the Z.ai feedback request is live as
`zai-org/feedback#419`, the community-marketplace PR is live as
`zai-org/zai-coding-plugins#30` from the org fork, and `main` carries the one-address
marketplace catalogs at the pushed revision. The Anthropic Plugin Directory submission is also
sent: confirmed received ("Plugin submitted for review") through the Console form from Leo's
Individual Org, with the exact submitted values in `submissions.md` section 3. All three
public submissions of the arc are therefore live and nothing is blocked. What remains is
waiting and watching: PR #30's review queue (four earlier third-party plugin PRs are still
unmerged), issue #419's triage pipeline, and the Anthropic review reaching out to
`hello@sylin.org` if it needs more. The optional P4 install verification in the real ZCode
client is the only open local work. Every outward and state-changing action of the arc is
indexed in [ACTION-LOG.md](ACTION-LOG.md).

## P1 plugin package and one-address catalogs

Status: complete (2026-08-29).

- Added `packaging/plugin/ghostlight/` with the twin manifests
  (`.claude-plugin/plugin.json` canonical, `.zcode-plugin/plugin.json` native), a plugin
  README, and the empty skill directory the next task fills.
- Added the root catalogs: `.claude-plugin/marketplace.json` (Claude schema) and
  `marketplace.json` (ZCode native), each listing the one plugin from
  `./packaging/plugin/ghostlight` at version 1.0.0.
- `userConfig` omitted per ADR-0144; the ZCode twin carries `timeoutMs: 120000` for first-run
  download headroom, matching the largest observed official precedent (computer-use, 90000)
  with margin.
- Live proof on this Windows machine: a node driver spawned `npx -y ghostlight`, completed the
  MCP initialize handshake, and received the full 24-tool catalog through a tools/list round
  trip. The launcher downloaded nothing new (binaries already verified at
  `~/.ghostlight/bin/v1.1.0`) and wrote nothing to stderr this run; the protocol stayed clean
  on stdout. The transcript is preserved in this ledger below.
- Windows finding from the proof, recorded not acted on: raw Node `spawn("npx", ...)` on this
  machine fails with ENOENT because `npx` is `npx.cmd`; the handshake needed shell resolution.
  This is the concrete mechanism behind the README's `cmd /c` note for clients that spawn
  without shell resolution. ZCode documents plain `npx` in its own examples, so the manifests
  keep plain `npx`.
- Watch item, recorded not acted on: the root `marketplace.json` could in principle be noticed
  by ZCode while working inside this repository. Workspace plugin discovery was not observed to
  read a root catalog, and nothing in this batch changes the working configuration. If a future
  session sees a phantom plugin card, this file is the first place to look.

### P1 live transcript (abridged)

- initialize request: protocolVersion `2025-11-25`, client `public-distribution-p1`.
- server result: protocolVersion `2025-11-25`, serverInfo `ghostlight` version `1.1.0`,
  capabilities `tools` with `listChanged`, plus the connector's instructions field.
- after the initialized notification, tools/list returned 24 tools: the 23 `browser_*` tools
  plus `policy_explain`, the complete 1.0 catalog.
- the driver then closed stdin and exited 0.

## P2 the control-browser skill

Status: complete (2026-08-29).

- Added `packaging/plugin/ghostlight/skills/control-browser/SKILL.md`.
- Written against `docs/1.0/LANGUAGE.md`; every bound quoted in the skill (timeouts, wait
  durations, flow and sequence step counts, upload ceilings, zoom and window ranges) was
  checked against the contract text in the same commit.
- Frontmatter follows the observed official-plugin style: one `name`, one dense trigger
  `description`. No policy content; the skill teaches observation, composition, and the
  honest-effect rules, and points refusals at `policy_explain`.

## P3 twin agreement in the repository-integrity gate

Status: complete (2026-08-29).

- `scripts/check-repository-integrity.ps1` now parses both plugin manifests and both catalog
  entries and asserts: equal plugin names, equal versions, equal skill declarations (string or
  array normalized to a set), equal MCP server command and args, and catalog entry versions
  equal to the manifest version.
- This gate prevents the demonstrated-class failure where a version bump edits one manifest and
  not its twin or catalogs (the ADR-0142 disease in a new member). It relaxes nothing.
- Deviation, worth keeping visible: the first version of the check was itself wrong, and running
  the gate caught it -- PowerShell evaluates `-join` and `-ne` with equal precedence,
  left-associatively, so an unparenthesized `a -join "`n" -ne b` compares the joined left side
  against `b` and then joins the boolean. The comparison now parenthesizes both sides, and the
  gate was run to green (859 tracked files, capability matrix included) before this batch's
  commits.

## X1 external submission drafts

Status: complete (2026-08-29).

- `submissions.md` in this directory holds three drafts: the ZCode in-app feedback ticket
  asking Z.ai for a public plugin intake process and proposing Ghostlight, the
  `zai-coding-plugins` pull-request text, and the Anthropic `claude-plugins-official`
  directory submission answers.
- Research facts the drafts rely on, verified 2026-08-29: the Z.ai community marketplace is
  `zai-org/zai-coding-plugins` (PR-based intake, owner contact `user_feedback@z.ai`, currently
  two GLM-plan plugins, four open third-party plugin PRs and none merged yet); the Anthropic
  directory accepts third-party plugins through `clau.de/plugin-directory-submission` with an
  `external_plugins/` review; ZCode documents npm as a marketplace source kind, resolving the
  earlier local-notes discrepancy in favor of the docs.
- Nothing was sent. Sending any of the three is an owner action.

## D1 index and status reconciliation

Status: complete (2026-08-29).

- `docs/tasks/README.md` gained the batch row.
- `docs/STATUS.md` gained the batch section and its Last updated line moved to 2026-08-29.
- `docs/MEMORY.md` is deliberately unchanged: the durable facts (the zero-argument launcher
  handoff, the twin-manifest rule, the marketplace landscape) all live in ADR-0144 and this
  ledger, and the memory file's own rule is that the tree wins when the tree can say it.

## Gate note for the opening commit series

Observed results, 2026-08-29, on the final tree of this series: `cargo fmt --check` clean;
`cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace` all
green (341 orchestrator library plus the other crates, zero failures); the extension npm suite
zero failures; `scripts/check-repository-integrity.ps1` green under PowerShell 7 with the new
plugin assertions (859 tracked files, links valid, capability matrix 21 COMPLETE and 4
SUPERSEDED rows, all evidenced). The Rust and extension trees are byte-identical across the
series' commits; every commit is a strict subset of this gated tree, so greenness transfers to
each of them.
