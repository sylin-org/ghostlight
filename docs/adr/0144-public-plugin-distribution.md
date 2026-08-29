# ADR-0144: Public plugin distribution

- Status: Accepted
- Date: 2026-08-29
- Builds on: ADR-0141, ADR-0142

## Context

Ghostlight reaches MCP clients three ways today: installer-written client configuration, the
npm launcher, and the MCP Registry record. None of those make Ghostlight discoverable inside
a client's own plugin marketplace, and none carry agent-facing guidance about when and how to
reach for the browser tools.

Distribution research on 2026-08-29 (the batch ledger records the sources) established:

- ZCode's official Public store has no submission or review process. Every rostered plugin is
  Z.ai-authored, and the store's own description promises community plugins that cannot yet
  join through any documented route.
- `zai-org/zai-coding-plugins` is a live Z.ai-operated marketplace with a plain pull-request
  intake. It uses the Claude Code plugin schema, and ZCode reads that schema and accepts
  Claude marketplaces as first-class sources.
- Anthropic's `claude-plugins-official` has the only documented third-party intake (a
  submission form and review), and ZCode preloads that marketplace.
- A static plugin manifest cannot point at an installed connector path: the connector
  demand-starts its sibling orchestrator beside its own executable, and install locations vary
  by platform and channel. The npm launcher already solves this. Invoked with no arguments it
  installs checksum-verified binaries into one versioned location and hands its inherited
  stdio to the MCP connector, so `npx -y ghostlight` is itself a complete MCP stdio command.

The owner directed a public distribution arc on 2026-08-29: package Ghostlight so any client
can install it from a marketplace, teach agents through a bundled skill, and prepare every
external submission as a draft that waits for owner action.

## Decision

- The plugin is a new distribution member with its own version space, the ADR-0142 model. Its
  source lives at `packaging/plugin/ghostlight/`, and its version line starts at 1.0.0.
- The plugin carries two manifests that must move together. `.claude-plugin/plugin.json` is
  canonical for Claude-schema marketplaces (Claude Code official, the Z.ai community
  marketplace, ZCode through its compatibility reading); `.zcode-plugin/plugin.json` is the
  native twin for ZCode marketplaces. The repository-integrity gate asserts the twins agree on
  name, version, skills, and MCP server command.
- The plugin's MCP server is the npm launcher handoff: `npx -y ghostlight` with no arguments.
  This rides the checksum-verified public release transport, pins nothing to an install
  location, requires no PATH entry, and satisfies the sibling rule because the launcher
  installs all three executables beside each other. The launcher's progress lines go to
  stderr and never enter the protocol stream.
- The repository is itself a one-address marketplace in both ecosystems: a Claude-schema
  catalog at `.claude-plugin/marketplace.json` and a ZCode-native catalog at `marketplace.json`,
  each listing the one plugin from `./packaging/plugin/ghostlight`.
- The plugin ships `skills/control-browser/`, which teaches the 1.0 language: the result
  envelope, the handle model, capability restrictions, the composition tools, and the
  honest-effect rules. The skill describes; it never decides policy, and it stays in lockstep
  with `docs/1.0/LANGUAGE.md` the same way trust claims do.
- `userConfig` is omitted from both manifests until empty-value expansion semantics are
  verified against a real client. A wrong empty expansion could shadow a `GHOSTLIGHT_POLICY_FILE`
  default with an empty string and fail closed.
- Every external step is an owner action: the Z.ai feedback request for a public intake
  process, a pull request proposing Ghostlight to `zai-coding-plugins`, and the Anthropic
  directory submission form. The batch drafts all three and sends nothing. Local commits are
  normal; anything outward waits for explicit owner confirmation.

## Consequences

- A plugin release is four small edits that must agree: the two manifest versions and the two
  catalog entries. The repository-integrity gate pins that agreement.
- A plugin registration and an installer-written client entry may coexist. Explicit client
  configuration overrides a same-named plugin server, which is the desired precedence, and the
  installer's foreign-entry protection is unchanged.
- The released installers do not yet register ZCode; ADR-0141 lands with the next service
  release. The plugin is the discovery and skills route and does not depend on that work.
- Verification for this member is live: the batch proves a real MCP initialize and tool
  listing through `npx -y ghostlight` on the development machine, and both manifests parse.
