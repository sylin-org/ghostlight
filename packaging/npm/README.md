# ghostlight (npm launcher)

Governed browser automation for AI coding agents. Ghostlight gives any MCP client controlled
access to your own authenticated Chromium session, with capability grants, protected domains,
and a structured audit trail. All-open by default; governance when you want it.

This npm package is a thin launcher. On first run it downloads the version-matched Ghostlight
binaries from the GitHub release and caches them under `~/.ghostlight/bin/`, so there are no
runtime dependencies. A bare `npx ghostlight` starts the MCP server your client talks to;
`npx ghostlight install` connects the browser side.

## Quick start

Add to any MCP client as a stdio server:

```json
{ "command": "npx", "args": ["-y", "ghostlight"] }
```

Then connect the browser side (once, idempotent):

```
npx ghostlight install
```

and add the "Ghostlight in Browser" extension from the Chrome Web Store. Full walkthrough,
one-click client buttons, and the manual paths:
https://sylin-org.github.io/ghostlight/install.html

## Links

- Project: https://github.com/sylin-org/ghostlight
- What it is and why: https://sylin-org.github.io/ghostlight/
- License: engine is Apache-2.0 OR MIT; governance module is source-available (see LICENSE).
