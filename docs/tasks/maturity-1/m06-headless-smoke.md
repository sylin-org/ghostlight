# M06: headless end-to-end smoke (extension + binary + MCP, Playwright-launched)

## Goal

ADR-0026 Decision 6 (second layer): an automated smoke that loads the real
extension in headless Chromium, connects it to the real ghostlight binary over
native messaging, drives the binary as an MCP server over stdio, and asserts
navigate, read_page, computer (screenshot and click), and form_input against a
pinned fixture page. Runs in CI on ubuntu.

## Authority

ADR-0026 Decision 6; 00-design.md "Headless smoke (m06)" (architecture pinned
there). This is the riskiest task in the batch and is deliberately last: a
BLOCKED outcome here does not degrade m01-m05.

## Depends on

m03 (ci.yml exists for the job append; if m03 BLOCKED, do everything else and
record the CI sub-step blocked-by-m03). m05 is NOT a dependency (the smoke
drives whatever worker code is present).
STOP preconditions: tests/e2e/ does not exist; `rg -n "GHOSTLIGHT_ENDPOINT"
tests/mcp_protocol.rs` matches (the env-var isolation pattern exists); network
is available for `npm install`. If any fails, STOP.

## Current behavior (verified 2026-07-03 pre-m02; re-read before editing)

NOTE: if m02 ran, it added a header line to tests/*.rs, so the line numbers
below are +1. Locate by content, not line number (BOOTSTRAP rule 7).

- Rust integration tests spawn the binary via
  `Command::new(env!("CARGO_BIN_EXE_ghostlight"))` with a per-test unique
  `GHOSTLIGHT_ENDPOINT` env var and speak newline-delimited JSON-RPC on stdio
  (tests/mcp_protocol.rs, the `fn drive` helper near the top of the file). The
  smoke reuses that protocol shape from Node instead of Rust; read that helper
  for the exact framing (one JSON object per line, stdin closed to end).
- The extension id is pinned by the manifest "key" to
  cjcmhepmagomefjggkcohdbfemacojoa regardless of load path.
- extension/native-messaging-host.json is the host-manifest TEMPLATE (name
  org.sylin.ghostlight, placeholders for path and origin).
- scripts/live-demo.ps1 is the existing manual smoke and the reference for
  tool-call sequencing against a live browser.
- Chromium (with a custom --user-data-dir) resolves native-messaging hosts on
  Linux from <user-data-dir>/NativeMessagingHosts/<name>.json. On Windows it
  uses the registry instead; therefore CI TARGETS LINUX ONLY and the local
  Windows verification is the dry run (below).

## Required behavior

### 1. tests/e2e/package.json (new)

    {
      "name": "ghostlight-e2e",
      "private": true,
      "devDependencies": {
        "playwright": "^1.49.0"
      }
    }

### 2. tests/e2e/fixture.html (new; served over local http by the runner)

Exactly:

    <!doctype html>
    <html>
      <head><title>Ghostlight smoke fixture</title></head>
      <body>
        <h1>Ghostlight smoke fixture</h1>
        <p id="marker">marker-before-click</p>
        <input id="name-input" type="text" aria-label="Name input" />
        <button id="click-me" onclick="document.getElementById('marker').textContent = 'marker-after-click'">Click me</button>
      </body>
    </html>

### 3. tests/e2e/run-smoke.mjs (new; SPDX engine header + node:assert)

Sequential flow; fail fast with a nonzero exit and a one-line reason:

1. Resolve the repo root (two dirs up). Locate the binary at
   target/debug/ghostlight(.exe); if absent run `cargo build` first.
2. Choose GHOSTLIGHT_ENDPOINT `ghostlight-e2e-<pid>` (needed by step 4's
   wrapper).
3. `--dry-run` mode: do steps 4-5 (profile dir, manifests, wrapper) and the
   http server, print the resolved plan as JSON, and exit 0 WITHOUT launching a
   browser or the MCP-server binary. This is the local-verification mode.
4. Start a node http server on 127.0.0.1:0 serving fixture.html; record the
   URL.
5. Create a temp user-data-dir; under it NativeMessagingHosts/
   org.sylin.ghostlight.json with path -> a generated wrapper script
   (POSIX sh: `#!/bin/sh` + `export GHOSTLIGHT_ENDPOINT=<the endpoint from step
   2>` + `exec <abs binary path> "$@"`, chmod 755) and allowed_origins
   ["chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/"].
6. Playwright launch. Extensions do NOT load in the old headless shell, so use
   the new headless mode: `chromium.launchPersistentContext(userDataDir, {
   channel: "chromium", headless: true, args: [
   "--disable-extensions-except=<abs extension dir>",
   "--load-extension=<abs extension dir>"] })`. Wait up to 15s for a service
   worker (context.serviceWorkers() / waitForEvent("serviceworker")). If none
   appears, retry once headed (`headless: false`) under xvfb-run when DISPLAY is
   absent; if still none, exit 3 (the CI job treats exit 3 as a failure with
   BLOCKED evidence).
7. Spawn the binary as the MCP server (stdio pipes, same GHOSTLIGHT_ENDPOINT
   env). Speak newline-delimited JSON-RPC:
   - initialize; expect a result.
   - tools/list; assert the tool names include navigate, read_page, computer,
     and form_input.
   - navigate { url: <fixture url> }; expect a non-error result.
   - read_page (default args); assert its text content contains
     `Ghostlight smoke fixture` and `marker-before-click`, and capture the
     ref of the element whose accessible name is `Name input` and the ref of
     the `Click me` button (parse refs from the read_page output).
   - computer { action: "screenshot" }; assert the tools/call result's content
     array contains an item of type "image" with non-empty base64 data. If the
     result shape differs from that, use the same STOP-and-dump probe as the
     read_page ref case below.
   - form_input with the captured input ref and value `ghost`; expect a
     non-error result.
   - computer { action: "left_click", ref: <button ref> }; expect a
     non-error result.
   - read_page again; assert it now contains `marker-after-click`.
8. Teardown: kill server child, close context, remove temp dir. Exit 0.

If the exact read_page ref syntax differs from what you expected, STOP and
record the actual output shape in the ledger (do not guess a parser); the
sanctioned probe is running step 7 through tools/list only and dumping one
read_page result verbatim into the ledger entry.

### 4. CI job (append to .github/workflows/ci.yml)

Transcribe as a sibling of the existing jobs: the block below is shown at the
job's own indentation, i.e. `e2e-smoke:` sits at 2-space indent under `jobs:`,
exactly like `fmt:` and `test:`. Do not place it at column 0.

    e2e-smoke:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: dtolnay/rust-toolchain@stable
        - uses: Swatinem/rust-cache@v2
        - uses: actions/setup-node@v4
          with:
            node-version: "22"
        - run: cargo build
        - run: npm ci --prefix tests/e2e || npm install --prefix tests/e2e
        - run: npx --prefix tests/e2e playwright install --with-deps chromium
        - run: node tests/e2e/run-smoke.mjs

### 5. .gitignore

Append exactly one line to .gitignore: `/tests/e2e/node_modules/`. Do NOT
ignore the lockfile: if `npm install` produces tests/e2e/package-lock.json,
commit it (it is not added to .gitignore).

## Constraints

Linux is the only supported live path in this batch; the script may carry
Windows registry support only as clearly-marked untested scaffolding or omit
it entirely (record which). No changes outside tests/e2e/, .gitignore, and the
pinned ci.yml append. The extension and binary are consumed as-is. ASCII only.

## Tests

- `node --check tests/e2e/run-smoke.mjs` exits 0.
- `node tests/e2e/run-smoke.mjs --dry-run` exits 0 on the Windows executor
  machine and prints the plan JSON (this is the required local verification).
- The full live run is NOT locally verifiable on Windows; it runs in CI on
  the first push. Record this explicitly in the ledger entry.

## Verification

The two commands above; `npm install --prefix tests/e2e` succeeds; ASCII diff
scan; `cargo test` unchanged; ledger entry stating the live-CI deferral;
commit.

Commit subject: `test(e2e): headless extension+binary smoke over native messaging (ADR-0026 D6)`

## Out of scope

Windows live e2e; flake-retry infrastructure; screenshots-to-artifact
uploads; testing governance behavior (this smoke is all-open); any extension
or binary code change to make the smoke pass (if the smoke exposes a product
bug, BLOCKED with evidence is the correct outcome).
