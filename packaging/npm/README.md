# ghostlight (npm launcher)

Delightful, responsible browser automation for AI coding agents. Ghostlight gives any MCP client
access to your own authenticated Chromium session, keeps the work visible, and adds inspectable
boundaries when you want them. All-open is a first-class default.

This npm package is a thin launcher. On first run it downloads the version-matched Ghostlight
binaries from the GitHub release and caches them under `~/.ghostlight/bin/`, so there are no
runtime dependencies. A bare `npx ghostlight` starts the MCP server your client talks to;
`npx ghostlight install` connects the browser side.

## Quick start

Install the service, browser connection, and detected MCP-client entries in one idempotent step:

```
npx -y ghostlight install
```

The command opens the current extension walkthrough on the first run. Until the Chrome Web Store
listing is public, the walkthrough provides the manual release-archive path. Restart your MCP
clients when both halves are installed. Full walkthrough, client buttons, and manual paths:
https://sylin.org/ghostlight/

For a client the installer does not recognize, use Ghostlight as this stdio server:

```json
{ "command": "npx", "args": ["-y", "ghostlight"] }
```

## Links

- Project: https://github.com/sylin-org/ghostlight
- What it is and why: https://sylin.org/ghostlight/
- License: engine is Apache-2.0 OR MIT; governance module is source-available (see LICENSE).
