# Windows public 1.3.12 consumer smoke -- 2026-09-30

Status: PASS. A fresh public npm invocation downloaded and verified all three Windows candidate
executables and printed the 1.3.12 CLI help.

## Scope

- Host: Windows x64.
- Entry: `npx --yes ghostlight@1.3.12 --help`, with a fresh npm cache and empty user config.
- Working directory: ignored `.tmp/release-1.3.12/public-consumer-rerun/`.
- `GHOSTLIGHT_HOME` selected an isolated directory inside that consumer folder. The launcher
  populated only that versioned download cache; this check did not install or uninstall browser
  registrations or MCP client configuration.

## Public bytes

The ordinary public npm tarball URL returned HTTP 200 at 14:19:36 UTC. Its SHA-256 matches the
frozen candidate: `b92e1aa2c33bfab4dccb2fe788f4ee9e5270e3949d6895cf74cd609b682b3da7`.
The launcher then downloaded all three executables from public GitHub release `v1.3.12` and
verified its embedded checksums. Independent recomputation matched the candidate manifest:

- `ghostlight.exe`:
  `ee0ab7d887c6bb45fbc8ee76c4f465c38e8d2b0dcd983dce18959b4992e6370b`.
- `ghostlight-mcp-connector.exe`:
  `0b9ce152c05d0710cd9ec6c18e359b87eecbd4ee5f14c6b67017b76e7de5216d`.
- `ghostlight-browser-connector.exe`:
  `a2af2cbe936061dc92bcc64838854fc4c51e0bd0b82dabd168702f4ef877a2df`.

Output is retained in `.tmp/release-1.3.12/public-consumer-rerun/consumer.log`.

## Retained publication delay

npm accepted the package once and reported asynchronous processing. Version and latest metadata
became public at 14:15:13 UTC, but the first fresh consumer attempt at 14:15:59 received HTTP 404
for the tarball. Its log remains in `.tmp/release-1.3.12/public-consumer/consumer.log`.
A cache-busted public download matched the candidate at 14:17:00, while the ordinary URL still
served a cached 404. Once the ordinary URL returned 200, a new empty-cache consumer passed.
No second publication or artifact change was made.

## Limits

This proves public npm resolution, release download integrity, launcher verification, and CLI
execution on Windows. It does not repeat clean-machine installation, registration, uninstall,
reboot, or physical Linux acceptance. Candidate CI covers Debian 12 and Ubuntu 24.04 package
lifecycle, and the [candidate custody record](candidate-custody-2026-09-30.md) separately records
the exact candidate's installed Windows browser checks and retained first-read failure.
