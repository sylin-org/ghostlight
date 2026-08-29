# public-distribution ACTION-LOG

One chronological entry per outward or state-changing action taken in the public
distribution arc, on 2026-08-29. The LEDGER carries the engineering detail; this file is the
who-did-what-when index. All pushes and submissions were owner-authorized.

## Research and verification (pre-work)

- Verified the plugin-distribution landscape live: ZCode has no official-store intake
  process; `zai-org/zai-coding-plugins` is a live PR-based Z.ai marketplace in Claude schema;
  Anthropic's third-party intake is the Plugin Directory Console form; ZCode reads
  `.claude-plugin` manifests and preloads Claude marketplaces.
- Verified the npm launcher's zero-argument invocation installs checksum-verified binaries and
  hands inherited stdio to the MCP connector, making `npx -y ghostlight` a complete static
  MCP command for marketplace manifests.
- Verified the real connector configuration surface (`GHOSTLIGHT_*` env vars) and the sibling
  demand-start rule (`crates/bridge/src/lifecycle.rs`), ruling out PATH-only and
  install-path-pinned manifest commands.

## Local build (commits on dev)

| Commit | Action |
| --- | --- |
| `54ccd4ed` | Opened the batch: ADR-0144, BOOTSTRAP, LEDGER |
| `6b6832d5` | Added `packaging/plugin/ghostlight/` (twin manifests, README) and the root one-address catalogs (`.claude-plugin/marketplace.json`, `marketplace.json`) |
| `665f38f3` | Added the `control-browser` agent skill |
| `519b3e57` | Pinned twin-manifest and catalog agreement in the repository-integrity gate |
| `218a0fd4` | Drafted the three external submissions |
| `ac154245` | Recorded the batch in STATUS and the tasks index |

- Full gate suite green before the commit series (fmt, warnings-denied Clippy, complete Rust
  suite, extension suite, repository integrity), plus JSON validation and ASCII checks.
- Live proof: real MCP initialize (protocol `2025-11-25`, serverInfo `ghostlight 1.1.0`) and
  the full 24-tool catalog through `npx -y ghostlight`.

## Publication

- Pushed `dev` and promoted `main` by fast-forward (`763ca798..ac154245`), owner-authorized.
- Verified all public marketplace surfaces serve at that revision (both catalogs, the ZCode
  manifest, the 128px icon: HTTP 200).
- `438a024b` recorded the promotion; pushed to `dev` and `main`.

## Send day (owner-authorized, driven through the browser via Ghostlight)

1. **Z.ai feedback issue filed.** `zai-org/feedback#419`, "Public intake process for the
   official plugin marketplace (follow-up to #66)", as `lbotinelly`. Duplicates searched
   first (closest is #66, shipped marketplace-adding, now cited); CONTRIBUTING.md read;
   category Tool use / MCP. First submit bounced on checkbox state; re-ticked with real
   clicks and resubmitted cleanly.
2. **Z.ai community marketplace PR opened.** Forked `zai-org/zai-coding-plugins` to
   `sylin-org` (owner is org admin; owner chose the org fork and the local clone path
   `F:\Replica\NAS\Files\repo\github\zai-org\zai-coding-plugins`). Branch
   `feat/ghostlight-plugin`, commit `704b7cb3` (catalog entry + `plugins/ghostlight/`:
   manifest, skill, marketplace-audience README). PR: `zai-org/zai-coding-plugins#30`.
3. **Anthropic Plugin Directory submission sent.** `claude plugin validate` passed first
   (CLI 2.1.250). Console flow: account onboarding (Individual), org created as Leo's
   Individual Org (billing skipped), wizard filled from the prepared answers. First Submit
   click bounced to the required terms acknowledgement on the introduction step; ticked and
   resubmitted with identical content. Confirmed: "Plugin submitted for review. Your plugin
   submission has been received." Submitted values: platforms Claude Code + Claude Cowork
   (owner's addition), license "Apache-2.0 OR MIT", privacy policy
   `https://sylin.org/ghostlight/privacy/` (path corrected against the website repository),
   contact `hello@sylin.org` (owner-set), repo `sylin-org/ghostlight`, path
   `packaging/plugin/ghostlight`.

## Record-keeping commits

| Commit | Action |
| --- | --- |
| `2061e1d9` | Recorded the send-day outcomes (#419, PR #30); pushed `dev` and `main` |
| `96d9dbee` | Recorded the Anthropic submission as sent; pushed `dev` and `main` |

## Open items and watch list

- Watch `zai-org/zai-coding-plugins#30` for review (four earlier third-party plugin PRs
  remain unmerged upstream).
- Watch `zai-org/feedback#419` through the triage pipeline (labels 待评估 -> 已采纳/已拒绝).
- Watch the Anthropic review; contacts go to `hello@sylin.org`. On approval the plugin is
  pinned by commit SHA in `anthropics/claude-plugins-community` and the pin auto-bumps.
- Optional local: P4 install verification of the plugin in the real ZCode client.
- If Anthropic or Z.ai asks for changes, the plugin source of truth is
  `packaging/plugin/ghostlight/` on `main`; a plugin release is the four-value edit the
  integrity gate enforces.
