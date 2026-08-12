# ADR-0113: Agent Plugin connection pack

- Status: Accepted
- Date: 2026-08-12
- Amends: ADR-0102 amendment A3
- Builds on: ADR-0015, ADR-0028, ADR-0077, ADR-0091, ADR-0096, ADR-0104, and ADR-0112

## Context

Agent Plugins v1 defines a portable directory format for Agent Skills and MCP server declarations.
Compatible clients can discover a package and own its install, enable, update, and removal
experience. The standard does not install a native application or browser extension, choose an
operating-system or CPU-specific executable, define one public catalog, or establish process
trust and product governance.

Ghostlight is already one separately installed local product. Its signed operating-system package
must place `ghostlight`, `ghostlight-mcp-connector`, and `ghostlight-browser-connector` together,
register the browser connector as the native-messaging host, and pair with the Chrome Web Store
extension. The MCP connector then demand-starts only its exact sibling `ghostlight`. It does not
search `PATH` for the authority or accept an authority path from a caller.

An Agent Plugin can remove hand-edited MCP configuration and put Ghostlight where users discover
agent capabilities. It cannot replace that installation. A self-contained plugin would duplicate
the machine-wide engine for each client, while a remote MCP endpoint would contradict the
local-only ingress and never-phone-home boundaries. A package that reports itself installed while
its separately installed connector cannot be found would also be worse than the direct journey.

ADR-0095's self-contained MCPB is a separate distribution contract with a platform-selecting
launcher. Agent Plugins v1 has no equivalent operating-system selector. This decision neither
changes that MCPB contract nor imports its bundle shape into the Agent Plugin.

The complete proposal and client survey are recorded in
[`../research/23-agent-plugins-v1-2026-08.md`](../research/23-agent-plugins-v1-2026-08.md).

## Decision

### 1. Publish one thin MCP-only portable declaration in the source tree

The repository root contains the Agent Plugins v1 `plugin.json` and `mcp.json`. The MCP document
declares one local stdio server named `ghostlight` whose command is exactly the bare executable
name `ghostlight-mcp-connector`.

The portable package declares no Skill and no client extension. It carries no hooks, alternate
tool language, policy, credentials, or per-plugin product state. The existing connector continues
to retrieve the one orchestrator-owned 22-tool catalog and render invocations generically.

### 2. Keep the signed Ghostlight installation authoritative

The Agent Plugin is a connection declaration, not an installer or a second Ghostlight product.
The signed platform package, its version-matched sibling executables, native-messaging
registration, and the matching store extension remain prerequisites.

The package does not embed a connector, engine, browser connector, extension, launcher, or runtime
download. It does not use `${PLUGIN_DATA}` as a second state authority. Once the client resolves
the bare connector command, exact-sibling demand-start, the operating-system lifetime lease, and
all existing service and browser recovery behavior remain unchanged.

Making the bare name reliably discoverable from supported GUI and terminal clients is a platform
installer responsibility and a release gate. Source conformance does not prove that future signed
packages have met it.

### 3. Give the client ownership of plugin registration

A client that installs the Agent Plugin owns that plugin entry and its enable, disable, update,
and removal lifecycle. Ghostlight's Workbench does not inspect, edit, or remove a client's plugin
store or cache.

The Workbench continues to own only explicit direct configuration entries created through **MCP
integrations**. It labels those as direct registrations and explains that a plugin-managed
connection must be managed in the client. Direct mutations remain ownership-checked and must not
replace foreign configuration. Installing both routes may create two MCP connector processes, so
the product does not present direct registration as required after a plugin is installed.

The portable bare connector command is not evidence of Workbench ownership. If it appears in a
supported client's ordinary configuration, the Workbench reports **Managed in client**, offers no
mutation, and leaves it untouched. A direct registration writes an absolute sibling path.

Both routes still converge on one lifetime-leased Ghostlight authority. Plugin ownership changes
only how a client starts the MCP connector; it grants no browser authority and changes no
governance decision.

### 4. Add no compatibility-only ingress or duplicate runtime

Ghostlight does not add Streamable HTTP, localhost MCP, hosted ingress, or a remote proxy for
directory eligibility. The Agent Plugin package does not bundle a second engine or use a symlink
or package-relative escape to reach the installed product. Ghostlight does not become an Agent
Plugin loader.

Client-specific metadata may later improve discovery and presentation, but it cannot change tools,
outcomes, approvals, audit, browser behavior, or recovery. A Skill or client extension requires a
separate decision backed by evaluation evidence.

### 5. Separate source compatibility from a released user claim

A repository contract test validates the closed v1 manifests, matching schema versions, one exact
stdio declaration, and the absence of remote transport, credentials, Skills, and client
extensions. The Agent Plugin journey reads `mcp.json`, resolves its bare command through an
isolated installer-like executable search path containing the sibling set, and verifies the exact
Ghostlight catalog.

That evidence proves the source package and connector seam. Public compatibility remains unclaimed
until clean signed installations prove command discovery, prerequisites, duplicate-route behavior,
disable, update, uninstall, and first useful browser work for every named client and platform.
Marketplace presence is a separate client-owned publication decision and requires explicit owner
approval.

## Consequences

- A compatible client can consume Ghostlight's portable MCP declaration without a second tool
  language or runtime architecture.
- The most useful benefit is client-native discovery and connection, while the signed desktop and
  browser installation stay truthful prerequisites.
- The portable command depends on platform executable discovery, which must be designed and tested
  in the signed installer before release.
- The Workbench and client have singular mutation boundaries: direct configuration belongs to
  Ghostlight, and plugin state belongs to the client.
- Installing both connection routes can still create two connector sessions. The UI explains the
  distinction; real-client testing must determine whether more duplicate detection is warranted.
- OpenAI's hosted MCP directory requirements do not justify a remote Ghostlight transport.
- Agent Plugins v1 conformance does not imply listing in any catalog or support in every client.

## Evidence

- `tests/agent-plugin-contract.mjs` validates the portable documents and negative boundaries.
- `tests/agent-plugin-journey.mjs` launches through the declared command from an isolated executable
  search path and checks the existing catalog and process topology.
- The Workbench names its existing configuration path as direct registration and sends
  plugin-managed users back to the owning client.
- Signed installer and real-client evidence remains a release gate recorded in
  [`../RELEASE.md`](../RELEASE.md).
