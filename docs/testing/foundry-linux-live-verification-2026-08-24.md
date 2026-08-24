# Foundry sprint Linux live verification -- 2026-08-24

Status: PASS on the CachyOS development host. This is source, user-candidate, and unpacked-adapter
evidence. It is not package, provenance, matching-store-adapter, or release evidence.

## Environment

```text
source_revision: 793e25854510c9bc69fd8971a7b6754d07c6b223
architecture: x86_64
distribution: CachyOS rolling
kernel: 7.2.0-1-cachyos
desktop_and_display_protocol: KDE Plasma 6.7.4, Wayland
browser: Chromium 151.0.7922.173, ordinary graphical profile
rust_and_cargo: 1.95.0
node_and_npm: 22.22.1 and 10.9.4
ghostlight_version: 1.0.0 development candidate 793e258
extension_version: 1.0.0 unpacked source adapter
```

## Source gates

The required lane gates passed before deployment:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`: 389 tests passed -- 335 orchestrator library, 11 orchestrator binary,
  36 bridge, and 7 MCP connector tests
- `npm test --prefix extension`: 132 tests passed

## Exact user candidate and adapter refresh

`cargo build --locked --workspace --release --target-dir .target-linux-foundry` built all three
optimized siblings from the recorded revision. They were installed without removing any older
candidate under `~/.ghostlight/bin/v1.0.0-dev-793e258`:

| Sibling | SHA-256 |
| --- | --- |
| `ghostlight` | `31e14318187614a9348997d8f2ca95313515c4a464dd1cf05ba4ee1067fc21df` |
| `ghostlight-mcp-connector` | `1acb4e11278cbc19e5a690f083b35ad7c97aae6e68d3220189f72348b1b9b721` |
| `ghostlight-browser-connector` | `0530fcc6836227de322c94e845044d52805169eb7c36941dbdd6289bc89df264` |

The ownership-checked installer updated the command, Applications entry, four browser registrations,
and detected owned MCP client entries to the exact installed siblings. It left a foreign Cline
entry untouched. Chromium's native-host manifest and `~/.local/bin/ghostlight` both resolved to
the revision-qualified install.

The ordinary Chromium profile had Developer mode enabled but no Ghostlight adapter loaded. The
repository `extension/` directory was loaded through Chromium's visible Load unpacked control.
The Ghostlight card then appeared with the expected pinned development id. Its visible Reload
control was clicked explicitly; the native connector restarted from the revision-qualified path.
`doctor --json` then reported all three siblings current, the Chromium package native, the browser
relay connected, and readiness `Ready`.

## Whole-catalog foundry result

The normal-paced command

```text
scripts/demo-foundry.sh --ghostlight ~/.ghostlight/bin/v1.0.0-dev-793e258/ghostlight
```

exited zero with all 41 beats green against the ordinary visible profile. In particular:

- `key to end` succeeded while the Release name field remained visible;
- `ring once` returned a confirmed success without an eight-second hang;
- the immediate `dialog status` truthfully reported that no dialog was visible yet, the documented
  early-reply timing nuance;
- `dialog answer`, `bell answered`, `ring again`, `dialog dismiss`, and `bell silent` all succeeded;
  and
- the story completed its governed off-domain refusal, replay delivery, and recording erasure.

No product defect appeared.

## Honest refusal and audit

A fresh CLI-owned foundry tab supplied a current target for the Rejection reason field. A page
script kept the field connected but made it invisible. `browser_press_key` then failed before key
dispatch with the exact result:

```text
status: failed
effect: none
summary: The browser refused this job: target is not visible for focus.
facts.reason: browser_primitive_failed
facts.detail: target is not visible for focus
```

The matching content-minimized local audit record carried `refusal_facts` with that reason and
detail. A succeeding `browser_execute` record omitted `refusal_facts`, and no inspected success
record carried the field.

## Limits

This pass used a revision-qualified user install and the unpacked source adapter on CachyOS KDE.
It did not use an Ubuntu-built Debian package, Ubuntu GNOME Wayland, a matching Chrome Store
adapter, a clean machine, build provenance, or a publishable release candidate. It performed no
tag, merge, publication, store, or release action.
