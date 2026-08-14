# Agent Plugins v1 and Ghostlight

Date: 2026-08-12

Status: Research input. This document records the proposal, client behavior, and Ghostlight fit.
It does not by itself create a supported package or change a product contract. A later ADR must
decide the compatibility boundary.

## Executive summary

Agent Plugins v1 is a small portable package format for Agent Skills and MCP server declarations.
It standardizes what a plugin directory looks like and how a compatible client loads its portable
parts. It does not standardize a public registry, installation UI, permissions, trust, updates,
native application installation, authentication, or client-specific features.

Ghostlight can benefit most at the discovery and MCP connection seam. A client can discover a
Ghostlight package in a marketplace, install its MCP declaration, and avoid hand-editing client
configuration. That package cannot install or replace the complete Ghostlight product. The signed
desktop package, three sibling executables, native-messaging registration, and Chrome Web Store
extension remain separate prerequisites.

The least surprising portable shape is an MCP-only package whose stdio command is the bare name
`ghostlight-mcp-connector`. It preserves one canonical Ghostlight installation and the connector's
exact-sibling demand-start behavior. It also creates the central unresolved problem: Agent Plugins
v1 has no portable installed-application lookup, absolute command path, operating-system selector,
or architecture selector. A bare command works only when each client can find the installed
connector through its platform executable-search rules.

The standard is therefore useful, but conformance alone is not a complete user journey.

## Sources and source status

Primary sources:

- [Agent Plugins overview](https://agent-plugins.org/)
- [Agent Plugins v1.0.0 specification](https://github.com/agentplugins/agent-plugins-spec/blob/main/spec/1.0.0.md)
- [Agent Plugins MCP author guide](https://agent-plugins.org/plugin-authors/mcp-servers)
- [Agent Plugins compatible clients](https://agent-plugins.org/compatible-clients)
- [Agent Plugins future considerations](https://github.com/agentplugins/agent-plugins-spec/blob/main/FUTURE_CONSIDERATIONS.md)
- [Vercel launch announcement](https://vercel.com/blog/introducing-agent-plugins)
- [AWS launch announcement](https://aws.amazon.com/blogs/opensource/aws-supports-agent-plugins-an-open-standard-for-portable-agent-extensions/)
- [VS Code Agent Plugins documentation](https://code.visualstudio.com/docs/agent-customization/agent-plugins)
- [Cursor Plugins documentation](https://cursor.com/docs/plugins)
- [Kiro Powers documentation](https://kiro.dev/docs/powers/)
- [GitHub Copilot plugin documentation](https://docs.github.com/en/copilot/concepts/agents/about-plugins)
- [OpenAI plugin packaging documentation](https://developers.openai.com/plugins/build/plugins)
- [OpenAI public MCP deployment requirements](https://developers.openai.com/plugins/build/mcp-server#deploy-the-endpoint)

Secondary context:

- [The New Stack overview](https://thenewstack.io/agent-plugins-open-standard/)

The canonical repository's versioned source describes v1.0.0 as published. On 2026-08-12 the
rendered website still described the specification as a Working Draft, and the repository had no
v1 tag or GitHub release. The inspected upstream source commit was
`bd383552095128f6effe895b9257cfd580a6d179`. Treat the versioned specification and schemas as the
technical authority, while recording that publication mechanics were incomplete.

## What the standard is

An Agent Plugin is a directory with one required root manifest and two optional portable component
types:

```text
my-plugin/
|-- plugin.json
|-- skills/
|   `-- example/
|       `-- SKILL.md
|-- mcp.json
`-- com.example.client/
    `-- client-specific files
```

- `plugin.json` identifies the package and the Agent Plugins version it targets.
- `skills/` contains Agent Skills using the separate Agent Skills specification.
- `mcp.json` declares MCP servers using a closed Agent Plugins schema.
- Reverse-domain extension namespaces let a client add private capabilities without changing the
  portable core.

The standard deliberately defines a small interoperability floor. A client may support Skills,
MCP, or both. An MCP-capable client must support at least one of local stdio or Streamable HTTP;
it need not support both. Unsupported independent components are skipped rather than making the
whole package unusable.

The package directory, not an archive or registry record, is the normative unit. Distribution can
wrap, copy, cache, or index that directory, but those mechanisms belong to clients and
marketplaces.

## What the standard does not define

Agent Plugins v1 does not define:

- one public registry or vendor-neutral catalog;
- package submission, review, publisher identity, signing, reputation, or revocation;
- install, enable, disable, update, rollback, or uninstall user experience;
- native application or browser-extension installation;
- operating-system or CPU-specific variants;
- sandboxing, process isolation, runtime consent, or per-tool approvals;
- OAuth fields, credential references, secret storage, or authentication UI;
- a portable relationship between a Skill and a particular MCP server;
- hooks, commands, custom agents, rules, LSP servers, or other client features;
- a standard conformance executable, linter, or certification program.

Clients own those choices. A package can be conformant while still being undiscoverable, unsafe,
impossible to run on one platform, or confusing to uninstall.

## Exact package requirements relevant to Ghostlight

### `plugin.json`

The root manifest is required. Its required fields are:

- `$schema`: `https://agent-plugins.org/schemas/1.0.0/plugin.schema.json`
- `name`: a lower-case package name from 1 to 64 characters, using ASCII letters, digits, dots,
  and hyphens under the specification's start, end, and repetition constraints.

The closed top-level field set also allows `version`, `description`, `author`, `homepage`,
`repository`, `license`, `keywords`, and `extensions`.

### `mcp.json`

An MCP component lives only at the root `mcp.json`. The document contains the matching schema and
an `mcpServers` object. A stdio entry requires:

- `type: "stdio"`
- one nonempty executable token in `command`

It may also contain `args`, `env`, and `cwd`.

The command is either a bare executable name or a package-relative path beginning with `./`.
There is no command interpolation. A bundled executable must use a package-relative path. A bare
command is resolved using client-selected platform search behavior.

Only `${PLUGIN_ROOT}` and `${PLUGIN_DATA}` are portable placeholders, and only in `args`, `env`,
and `cwd`. `PLUGIN_ROOT` names the installed package copy. `PLUGIN_DATA` names a client-managed
writable directory for that installed plugin instance. Neither is a portable route to an
independently installed desktop application.

All package paths must remain inside the resolved plugin root. An escaping symlink, junction, or
reparse point is invalid. A symlink from the package to Ghostlight's installed connector is not a
conforming workaround.

### Validation

The project publishes JSON Schemas and a manual checklist, but no official runnable conformance
harness. Schema validation alone cannot prove path containment, schema-version agreement,
executable semantics, transport support, or absence of secrets. A product should add its own
semantic and live-client tests.

## Governance and ecosystem

The initial Technical Steering Committee includes maintainers from Amazon, Cursor, Microsoft,
OpenAI, and Vercel. Proposals and material changes begin in public GitHub discussions. That is a
meaningful cross-vendor governance signal, but not a guarantee that every client implements the
same components or experience.

Version 1 was intentionally narrowed from broader early proposals. Skills and MCP had enough
independent standardization and adoption to form the portable core. Commands, hooks, agents,
rules, and LSP integrations remain client-specific.

The New Stack article usefully highlights the operational risks outside the format: a plugin may
expand the agent's security blast radius, while update testing, rollback, identity, access, and
visibility remain host or operator responsibilities. Its broad "write once" framing should be
read narrowly: package the supported portable components once, then adapt distribution and client
experience as necessary.

## There is no universal Agent Plugins catalog

`agent-plugins.org` publishes the specification, schemas, author guidance, and compatible-client
matrix. It does not operate a canonical public plugin registry.

Catalogs are client-owned:

- OpenAI operates one universal public Plugin Directory shared by supported ChatGPT and Codex
  surfaces, plus local, workspace, and personal marketplaces.
- VS Code and GitHub Copilot can use repository-backed marketplaces and ship default catalogs.
- Cursor operates a public marketplace and supports team marketplaces.
- Kiro operates the Powers catalog and accepts GitHub sources.
- Other vendors and communities can publish their own catalogs.

Standards compatibility and marketplace eligibility are separate. A conformant directory is not
automatically indexed anywhere, and each catalog can require richer metadata, review, licensing,
transport, signing, or hosting.

OpenAI is a concrete boundary for Ghostlight. Its public MCP submission route currently requires a
stable, publicly reachable Streamable HTTP endpoint. Local stdio and local endpoints do not satisfy
that route. Creating a remote Ghostlight proxy only for directory eligibility would contradict
Ghostlight's permanent local-only ingress and never-phone-home decisions.

## Real client experience

The actual clients confirm that the standard does not standardize the journey.

| Client | Discovery and install | Activation and lifecycle | Important limit |
| --- | --- | --- | --- |
| VS Code | Extensions search, Agent Customizations, marketplace, Git source, and workspace recommendations | Global or workspace enablement; MCP starts with the enabled plugin; normal update controls | Consumes portable Skills and MCP, but currently ignores Agent Plugins extension namespaces |
| Cursor | Marketplace and Customize UI with user or project scope; team marketplaces can use Default Off, Default On, or Required | Installed MCP servers and Skills can be managed from Customize | Public review and richer Cursor Plugin features remain Cursor-specific |
| GitHub Copilot CLI | Default and additional repository marketplaces, repositories, URLs, and local paths | Plugin, Skill, and MCP enablement are separately manageable; updates are explicit | Marketplace and enterprise policy are Copilot contracts, not Agent Plugins contracts |
| Kiro | Curated Powers catalog, IDE panel, GitHub URL, or local directory | Powers activate dynamically from conversation context and unload when irrelevant | Dynamic activation and keywords are Kiro behavior, not the portable guarantee |
| ChatGPT and Codex | OpenAI public directory plus workspace, repository, and personal catalogs on supported surfaces | Install and enable state are client-owned; a new chat or session may be needed | Public MCP submission requires hosted Streamable HTTP; local package support and public listing are different routes |

Explicit Skill invocation also differs. A Skill might appear as `@plugin`, `$skill`, `/skill`,
`/plugin:skill`, or another client-chosen name. Portable identifiers should therefore be short and
stable, but literal invocation syntax should not be part of Ghostlight's product contract.

## User-delight opportunity

The largest potential gain is not a new browser capability. It is reducing setup archaeology.

### Discovery

Ghostlight could appear where a user already looks for agent capabilities. A marketplace card can
explain the product in terms of the visible, authenticated browser experience instead of requiring
the user to begin with MCP configuration.

### Understanding and trust

Client-specific listing metadata can provide icons, screenshots, short demonstrations, starter
tasks, publisher review, and links to the trust center. The portable manifest itself is too small
to carry the whole product story.

Plugin installation trust and Ghostlight runtime governance are separate layers. Installing a
package authorizes the client to load its components. Ghostlight still decides which browser work
is admitted and records its content-minimized audit.

### Installation

A client-owned plugin can replace a hand-edited MCP entry. It cannot honestly collapse the entire
Ghostlight installation into one click. Users still need:

1. the signed Ghostlight operating-system package;
2. its native-messaging registration;
3. the matching Chrome Web Store extension; and
4. the client-owned Agent Plugin connection.

The experience can still be delightful if those prerequisites appear as one coherent journey with
truthful state and one next action. A plugin that says Installed while its MCP process cannot start
would make the experience worse.

### First success and daily use

A starter task can lead to a bounded visible first proof. Kiro-style dynamic activation could also
keep Ghostlight's 22 tools out of unrelated conversations. Those are client presentation and
selection benefits; they do not require a second Ghostlight tool language.

### Teams

Workspace recommendations and organization marketplaces can make Ghostlight discoverable and
approved. Native deployment still belongs to the platform package or device-management system. An
organization that requires the plugin without installing the desktop product and extension would
create a managed broken state.

## Ghostlight architecture mapping

The existing process boundary is:

```text
Agent client
  -> ghostlight-mcp-connector
  -> local typed service bridge
  -> ghostlight orchestrator and workbench
  -> local typed browser bridge
  -> ghostlight-browser-connector
  -> Chromium native messaging
  -> Ghostlight in Browser
```

The Agent Plugin integration point is `ghostlight-mcp-connector`. It is already the generic MCP
stdio edge. It owns protocol lifecycle and generic rendering, while the orchestrator remains the
sole owner of tools, product language, governance, workspaces, browser execution, and completion.

The connector also demand-starts only the exact sibling `ghostlight` executable beside its own
current executable. It deliberately does not search `PATH` for the orchestrator or accept a path
over the wire. That creates several consequences:

- A plugin that copies only the connector cannot start the installed Ghostlight authority.
- A self-contained plugin would need all three platform-matched executables for each supported OS
  plus native-host setup,
  duplicating the installed product and its lifecycle.
- Per-client plugin copies could race versions and registration ownership while Ghostlight is
  designed as one machine-wide authority.
- Per-plugin `PLUGIN_DATA` is the wrong owner for Ghostlight's machine-wide state.
- A localhost or hosted MCP listener would add a new ingress and trust boundary only for packaging.

The existing Workbench owns explicit client configuration today. If a client owns the same MCP
entry through a plugin, ownership must remain singular. Ghostlight must not overwrite or remove a
client-owned plugin registration, and it should not show a misleading direct-registration state.
Duplicate manual and plugin-managed entries are a foreseeable failure mode.

## Candidate package shapes

### Thin declaration using an installed command

```json
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "ghostlight": {
      "type": "stdio",
      "command": "ghostlight-mcp-connector"
    }
  }
}
```

This is the best architectural fit. It preserves the signed product and one engine. Its gate is
portable executable discovery. Ghostlight's future installers would need to make the command
reliably discoverable to GUI and terminal clients without creating ambiguous or stale `PATH`
behavior.

### Self-contained package

Bundling all Ghostlight executables can make `command` package-relative, but the standard cannot
select operating system or architecture. Distribution would need to choose a platform-specific
package outside the standard. This also creates duplicated engines, update owners, native-host
registration races, and uninstall residue. It is a poor default.

### Launcher or shim

A bundled launcher could find the canonical installed product, but it becomes a new cross-platform
compatibility product that must be signed, versioned, tested, and distributed. A script is not
uniformly executable across Windows and Linux. This option should exist only if installer
and client experiments prove a direct installed-command route cannot work.

### Remote MCP

Reject as a packaging workaround. A public or localhost HTTP server would add a new listener,
authentication problem, and ingress model. Public hosting would also contradict the product's
local authenticated-browser promise and no-phone-home boundary.

### Ghostlight as an Agent Plugins client

Reject as a category error. Ghostlight provides governed browser capability to agent clients. It
does not host a model or need to become another general plugin loader.

## Skills and client extensions

An initial package does not need a Skill. Ghostlight already has one canonical, tested 22-tool
language and typed recovery contract. A broad SKILL.md would duplicate that language, consume
context, and create another mutable prompt supply chain.

A future Skill is justified only by evaluation evidence that it improves activation or first
success. It should then be narrow, generated or checked against the canonical catalog, and useful
without changing tool authority or outcome language.

Client extensions may improve presentation, metadata, or onboarding. They must not add hooks or
client-specific policy that changes tools, approvals, audit, browser behavior, or recovery. VS Code
currently ignores Agent Plugins extension namespaces, so a portable package must remain complete
without them.

## Recommended evaluation gates

A Ghostlight package should not be claimed as a supported user journey until it proves:

1. Both JSON documents validate against pinned v1.0.0 schemas and semantic checks.
2. One signed Ghostlight installation remains the sole engine and provenance chain.
3. The client reaches the exact existing 22-tool catalog through stdio.
4. No connector, engine, browser connector, or extension is downloaded or copied at runtime.
5. The absent desktop product, absent extension, stale version, and disconnected browser each
   produce a truthful diagnosis and useful next action.
6. Installing through a plugin does not create a duplicate direct MCP registration.
7. Workbench state distinguishes Ghostlight-owned configuration from client-owned plugin state.
8. Multiple clients converge on one authority.
9. Disable, update, and uninstall leave no false status and do not remove independently installed
   Ghostlight components.
10. The package adds no remote ingress, phone-home behavior, alternate model language, or hidden
    operating-system mutation.
11. The journey is tested on every claimed operating system and client.

Useful experience measures are time to first visible browser result, number of manual
configuration edits, number of prerequisite surprises, duplicate-server incidence, and whether a
new user can explain which trust decision belongs to the client and which belongs to Ghostlight.
Ghostlight's no-telemetry promise means these should be measured through local test evidence and
moderated usability work rather than product analytics.

## Research conclusion

Agent Plugins can benefit Ghostlight, primarily through discoverability, client-native lifecycle,
and a shorter route to the first browser task. It cannot replace the desktop product, browser
adapter, native messaging, or Ghostlight governance.

The best initial direction is a thin MCP-only connection package over the canonical installed
Ghostlight product. Do not make it a 1.0 release blocker, do not create a remote endpoint for
catalog eligibility, and do not bundle a second Ghostlight engine by default. First prove the
installed-command and ownership journey in real clients; then add client-specific listing polish
around the same portable core.
