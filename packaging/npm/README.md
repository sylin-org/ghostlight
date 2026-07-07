# ghostlight (npm launcher)

Governed browser automation over your own authenticated Chromium session, for AI coding
agents: any MCP client, your real logged-in browser, with capability grants, sacred domains,
and a structured audit trail. All-open by default; governance when you want it.

This npm package is a thin launcher: on first run it downloads the version-matched
`ghostlight` binary (a single Rust executable, no runtime dependencies) from the GitHub
release and caches it under `~/.ghostlight/bin/`. Everything real lives in the binary.

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
- License: engine Apache-2.0 OR MIT; the governance module's source is readable under the
  Ghostlight Commercial License (see the repository's LICENSE for the split).
