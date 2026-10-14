# Break 2.0: switch ledger

The switch runs after the three lanes, one slice at a time, and owns every file. Ground rules are
in [BOOTSTRAP.md](BOOTSTRAP.md). The target is
[ADR-0197](../../adr/0197-contained-seams-and-the-2-0-break.md); the wire is
[adapter-protocol-4](../../design/adapter-protocol-4.md).

Gate, with `CARGO_TARGET_DIR=.target-assessment`: `cargo fmt --all --check`, `cargo clippy
--workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `npm test
--prefix extension`, the protocol-4 and level-3 page asset tests, `npm test --prefix
packaging/npm`, `node --test packaging/mcpb/test/launcher.test.js`, `node --check` on changed
scripts, and `scripts/check-repository-integrity.ps1`. No journeys and no binaries are started.

## RESUME HERE

Next: the second commit of switch-1, which deletes the typed service bridge (`service.rs`,
`client.rs`, `relay.rs` and `runtime.rs` in `crates/bridge`) and moves the session vocabulary into
`crates/orchestrator/src/intake/session.rs`.

## Landed

| Slice | What | Commit |
|---|---|---|
| switch-1, commit 1 | The service serves relay protocol 3 on one loopback listener (`intake/listener.rs`): the lease is the relay lease on `home/ghostlight-runtime.lock`; `publish_ready` writes `relay-templates.json` and then the relay-3 runtime file; openings are read under the 5 s deadline with bounded pre-auth slots, the token is compared in constant time, and refusals carry a code and a sentence from `language/intake.rs`. `mcp` streams feed `McpSession`, send a resume token after initialize and after every result, and a resume token restores the revision, client, channel and the tabs one `ListTabs` still finds. `browser` streams attach to the browser port. `control` answers readiness, `activate_workbench` and `quit`. The command line is an MCP client (`cli/client.rs`) on the `cli` channel; `status`, `doctor` and `open` use control streams, and `call` and `open` start `serve` when no service answers. `GHOSTLIGHT_TEST_HOME` replaced `GHOSTLIGHT_RUNTIME_FILE` in the service; service tests hold one shared test home. | see `git log` |

## Notes

- `status --json` now reports `{version, relay, running}`; `running` means a readiness control
  opening was answered.
- A channel that policy does not admit is refused at the opening with the `unsupported` code and
  the policy sentence. The channel switches go away in switch-11.
- Protocol-4 page assets are not yet handed to the service's browser port; switch-2 does that.
