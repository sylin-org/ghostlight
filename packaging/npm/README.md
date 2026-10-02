# ghostlight (npm launcher)

Delightful, responsible browser automation for AI coding agents. Ghostlight gives any MCP client
access to your own authenticated Chromium session, keeps the work visible, and adds inspectable
boundaries when you want them. All-open is a first-class default.

This npm package is a thin launcher. On first run it downloads the version-matched Ghostlight
binaries from the GitHub release into temporary storage and verifies all three before installing
them at fixed paths under `~/.ghostlight/bin/`. Updates reuse those paths. The native deployment
seam replaces only changed components; unchanged connectors keep running. Every launch verifies
the installed files against the checksums carried by the package. The browser injector receives
the current page runtime from the service without an extension update or reload.
A bare `npx ghostlight` starts `ghostlight-mcp-connector`, the stdio server your
client talks to; `npx ghostlight install` connects the browser side.

## Quick start

Install the service, browser connection, and detected MCP-client entries in one idempotent step:

```
npx -y ghostlight install
```

Restart your MCP clients when both halves are installed. Full walkthrough, client buttons, and
manual paths:
https://sylin.org/ghostlight/

Check the local connection at any time:

```
npx -y ghostlight doctor
```

For a client the installer does not recognize, use Ghostlight as this stdio server:

```json
{ "command": "npx", "args": ["-y", "ghostlight"] }
```

## Links

- Project: https://github.com/sylin-org/ghostlight
- What it is and why: https://sylin.org/ghostlight/
- License: Apache-2.0 OR MIT for the whole product (see LICENSE and LICENSING.md).
