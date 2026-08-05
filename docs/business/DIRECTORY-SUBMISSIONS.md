# MCP directory submissions

Last checked: 2026-08-04.

This file separates facts, ready copy, and external gates for the Claude and OpenAI directory
paths. It is not proof that a submission was sent or accepted.

## Claude Connectors Directory

Anthropic accepts local servers as MCPB desktop extensions through a separate Google form:
https://clau.de/desktop-extention-submission. The public submission guide requires clear setup
documentation, complete tool titles and risk annotations, a Privacy Policy section in the MCPB
README, and a `privacy_policies` array in its manifest. Ghostlight meets those product requirements.

The live form inspected on 2026-08-04 additionally states that extensions must be public on
GitHub, MIT licensed, built with Node.js, and have the manifest author URL point to a GitHub
profile. It asks for:

- contact name and email;
- an MCP server description;
- the desktop-extension GitHub link;
- confirmation that the submitter is the primary party;
- the `.mcpb` file; and
- agreement to the MCP Directory Terms and Conditions.

Ghostlight's repository is public, its MCPB launcher is Node.js, and its manifest author URL
points to the Sylin GitHub organization. The complete package is not MIT-only: the engine is
Apache-2.0 OR MIT, while the governance module is under the Ghostlight Commercial License. Do not
describe the bundle as MIT licensed or submit it until Anthropic confirms that this open-core
license boundary is eligible.

The second gate is mechanical. The form requires an uploaded MCPB, and the package needs the new
`--no-clients` installer mode. Submit only after these changes ship in a new service release and
the release asset passes the official MCPB validator.

### Eligibility inquiry draft

Send to `mcp-review@anthropic.com` only after owner approval.

Subject: Eligibility question for an open-core local MCPB

Hello Anthropic MCP review team,

We are preparing Ghostlight, a local browser-automation MCP server, for the Claude Connectors
Directory. The public repository is https://github.com/sylin-org/ghostlight. The MCPB is
self-contained, uses a Node.js launcher around native Rust binaries, runs entirely on the user's
machine, includes no telemetry or runtime downloads, and publishes tool titles plus conservative
read-only and destructive annotations.

The current desktop-extension form says submitted extensions must be MIT licensed. Ghostlight is
open-core: its engine and relay are Apache-2.0 OR MIT, while its optional governance module ships
under the source-available Ghostlight Commercial License. The boundary is documented in
https://github.com/sylin-org/ghostlight/blob/main/LICENSING.md.

Are mixed-license open-core MCPB packages eligible for directory review, or is the form's MIT-only
requirement absolute? We will not submit or characterize the complete bundle as MIT licensed
without your guidance.

Thank you,
Leo Botinelly
Sylin

### Form copy after eligibility and release

- Is this an update: No
- Primary contact: Leo Botinelly
- Primary contact email: hello@sylin.org
- MCP server description: Use your signed-in Chromium browser from Claude. Ghostlight runs locally
  and provides navigation, page reading, screenshots, form filling, uploads, console and network
  inspection, and multi-step workflows. Optional governance adds capability grants, protected
  domains, and local structured audit logs. Requires Ghostlight in Browser from the Chrome Web
  Store.
- Desktop Extension GitHub Link:
  https://github.com/sylin-org/ghostlight/tree/main/packaging/mcpb
- Primary Party Confirmation: Yes
- File: the `ghostlight-v<service-version>.mcpb` asset from the matching GitHub release

The founder must personally read and accept the then-current directory terms. Never pre-check that
box on the founder's behalf.

## OpenAI public plugin directory

OpenAI's current public submission path accepts MCP-backed plugins only when the submitter provides
a public production MCP server URL. The portal scans that server, requires control of its HTTPS
host for domain verification, and requires the example endpoint to remain publicly accessible
during review. It does not currently accept a local stdio MCP package as the public server.

That requirement conflicts with Ghostlight's deliberate local-only browser-control boundary in
ADR-0077. Do not add Streamable HTTP, WebSocket, a cloud relay, or a hosted browser proxy just to
satisfy the directory form. Codex users remain supported directly through Ghostlight's lossless
TOML installer target, and a local Codex plugin can bundle stdio configuration without making the
browser remotely reachable. Neither path creates an eligible public OpenAI submission today.

### Local-package support inquiry draft

Post to the OpenAI developer forum or send through an official plugin-support channel only after
owner approval.

Subject: Public directory path for a local-only stdio MCP plugin

Ghostlight is a public, local-only browser automation MCP server for Codex and other clients:
https://github.com/sylin-org/ghostlight. Codex already connects through stdio, and Ghostlight's
installer safely merges the entry into `~/.codex/config.toml`.

The current public plugin submission flow requires a production HTTPS MCP URL and domain
verification. Hosting browser control would weaken Ghostlight's local trust boundary, so we will
not add a remote transport only for directory eligibility. Is there a planned public submission
path for local stdio MCP plugins, similar to MCPB, or a way to submit a Codex plugin whose MCP
server runs entirely on the user's machine?
