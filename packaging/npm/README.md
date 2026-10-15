# ghostlight (npm launcher)

Delightful, responsible browser automation for AI coding agents. Ghostlight gives any MCP client
access to your own authenticated Chromium session, keeps the work visible, and adds inspectable
boundaries when you want them. All-open is a first-class default.

This npm package is a thin launcher. Every command except `doctor`, `status`, `help` and
`--version` runs the installed set under `~/.ghostlight/bin/` when its `install.json` records the
same or a newer service version than this package. Otherwise the launcher downloads the
version-matched Ghostlight binaries from the GitHub release into temporary storage, verifies all
three against the checksums carried by the package, and runs `ghostlight deployment install` from
there. The four never-install commands run the installed service, or with nothing installed a
verified temporary copy that installs nothing.
`npx ghostlight deployment install` always runs this package's own verified copy, and with
`--restore` it replaces a development installation (a build the development loop installed) with
this release; a release package never installs over one by itself.
A bare `npx ghostlight` starts `ghostlight-mcp-connector`, the stdio server your client talks to;
`npx ghostlight install` connects the browser side. On Linux the service needs
`libwebkit2gtk-4.1`; the launcher names the package to install when it is missing.

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
