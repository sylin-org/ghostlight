# public-distribution BOOTSTRAP

The batch that packages Ghostlight as an installable plugin member, bundles the agent-facing
skill, and drafts every external distribution submission. It implements
[ADR-0144](../../adr/0144-public-plugin-distribution.md).

Authority order: this BOOTSTRAP, then ADR-0144, then the current tree. The ledger is the
authority on progress; a task here describes intent, the ledger records what happened.

## Ground rules

- One task = one commit, and every commit leaves a green tree: formatting, warnings-denied
  Clippy, the full Rust suite, the extension suite, and the syntax or integrity gates its
  change touched.
- Nothing in this batch changes Rust or the extension. The one script change (P3) only adds
  assertions to the existing repository-integrity gate; it relaxes nothing.
- Nothing leaves the machine. No push, no pull request, no marketplace submission, no feedback
  ticket, no store action. X1 drafts them; the owner sends them.
- The plugin manifests and the skill are public surfaces. ASCII only. Every behavioral claim
  stays inside what `docs/1.0/` and the trust center already promise; no claim appears in the
  plugin before it is true in the tree.
- Never copy from `reference/`; never weaken a trust-doc claim; the standing rules in
  `AGENTS.md` apply in full.
- A task that cannot close honestly is BLOCKED in the ledger with the reason. Do not improvise
  around a changed tree; STOP and record.

## Tasks

1. P1 plugin package and one-address catalogs: the twin manifests, the plugin README, the root
   Claude-schema and ZCode-native catalogs, and a live proof that `npx -y ghostlight` completes
   a real MCP initialize and tool listing on this machine, recorded in the ledger.
2. P2 the control-browser skill: `skills/control-browser/SKILL.md`, written against
   `docs/1.0/LANGUAGE.md` and reviewed claim by claim against it.
3. P3 twin agreement in the repository-integrity gate: name, version, skills, and MCP server
   command must match across the two manifests, and both catalog entries must carry the plugin
   version.
4. X1 external submission drafts: the Z.ai feedback request for a public intake process, the
   `zai-coding-plugins` pull-request text, and the Anthropic directory submission answers.
   Draft, then wait.
5. D1 index and status reconciliation: the tasks README row, the STATUS section, and any
   durable learning that the tree cannot already say.
