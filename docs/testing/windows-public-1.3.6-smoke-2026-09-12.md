# Windows public 1.3.6 consumer smoke -- 2026-09-12

Status: PASS. The public npm launcher and connector honored the existing development selection and
reached the connected browser without creating a second serving installation.

## Scope

- Host: the owner's normal Windows development machine and signed-in Chrome profile.
- Entry: `npx -y ghostlight@1.3.6 --help` from an isolated, non-repository working directory.
- Mutation boundary: the launcher populated its versioned download cache. It did not run install,
  uninstall, or alter browser and MCP registrations.
- Existing selection: development, serving from the repository `target/release` sibling set.

## Public bytes

The launcher downloaded all three Windows executables from public release `v1.3.6` and verified
their packaged checksums. Independent SHA-256 recomputation returned:

- `ghostlight.exe`: `2ba072376629b2ce3695d136b23bb0fb12e25d765c0166c693bfab9d33061ea2`.
- `ghostlight-mcp-connector.exe`:
  `4cec419e44f80b0fc0e1a567e26993165dafcac43a42109a794130cb7ffcc490`.
- `ghostlight-browser-connector.exe`:
  `88b257b6ab645774036077cf272af2933d1bb2646d2afa60e6613af3423bc428`.

Each hash matches the frozen 1.3.6 candidate manifest.

## One-authority proof

Running `doctor --json` through the public cached `ghostlight.exe` reported:

- invoking version 1.3.6;
- selection source `development`;
- the serving, state, and sibling binary paths under the existing repository `target/release`;
- readiness `Ready`, with the existing Chrome native-host registration current; and
- the already running compatible development service, whose embedded version remains 1.3.5.

The older service version in this observation is intentional evidence, not a downgrade: the public
1.3.6 executable discovered and used the selected live development authority instead of starting
its own release-cache authority.

The exact public cached `ghostlight-mcp-connector.exe` then completed MCP initialization, returned
23 tools, included `browser_flow`, omitted retired `browser_sequence`, and completed a content-free
`browser_tabs` list call against the real connected browser.

After connector stdin closed, the smoke connector exited within the bounded wait. Process inventory
reported zero running executables from the public 1.3.6 cache and exactly one `ghostlight.exe`, the
selected development authority. Existing client-owned MCP connector processes remained on the same
development sibling path; they are independent stdio edges, not serving installations.

## Limits

This smoke proves public-package download integrity and the release-to-selected-development route
on the active machine. It does not repeat installer registration, uninstall, reboot, or Linux
physical acceptance. Those remain bounded by their separate reports.
