# Ghostlight Licensing Guide

Ghostlight is free and open source. The entire product -- the orchestrator, browser engine,
relays, governance module, desktop workbench, extension, and installers -- is offered under
Apache-2.0 OR MIT, at your option ([Apache-2.0](LICENSE), [MIT](docs/licenses/MIT.txt)).

Decision record: [ADR-0027](docs/adr/0027-open-core-business-model-and-licensing.md) originally
split the tree into a permissive engine and a commercially licensed governance module.
[ADR-0140](docs/adr/0140-fully-open-source-licensing.md) superseded that split on 2026-08-25:
every paid option was withdrawn, and the whole repository now carries one permissive license.

## What this means

| You get | Because |
|---|---|
| Use for anything, including commercial and operational use | Apache-2.0 OR MIT place no field-of-use or seat limits |
| Full source to every component, including governance | There is no separately licensed module |
| Redistribution and modification | Both licenses permit it; preserve notices and follow the chosen license's terms |

There are no tiers, no seats, no activation, and nothing to buy. Support is community support:
GitHub Issues and [Discussions](https://github.com/sylin-org/ghostlight/discussions) first;
hello@sylin.org for what cannot be public (see [CONTRIBUTING.md](CONTRIBUTING.md)). The former
pricing page was removed entirely; the git history of that path records what pricing used to be.

## Runtime promise

License state never reached runtime under the old model and there is no license left to reach it:
Ghostlight has no key file, activation server, license-status command, telemetry, or network
traffic. An installed copy keeps working offline indefinitely. This is the Continuity Promise
([ADR-0028](docs/adr/0028-tripwire-licensing-and-continuity-promise.md) Decision 6), unchanged.

## Vendored third-party code

One dependency ships inside the browser extension rather than through a package manager, because
the extension never loads remote code:

| File | Component | License |
|---|---|---|
| `extension/vendor/gifenc.js` | [gifenc](https://github.com/mattdesl/gifenc) 1.0.3, a JavaScript animated GIF encoder | MIT ([text](extension/vendor/gifenc.LICENSE.md)) |

It is pinned to an exact version and reviewed like any other dependency
([ADR-0109](docs/adr/0109-browser-owned-gif-encoding.md)). Rust dependencies are declared in
`Cargo.toml` and resolved normally.

## Commitments

- The whole product stays Apache-2.0 OR MIT. It will not be relicensed restrictively.
- A bug fix, a security fix, or a core automation capability is never moved behind payment,
  because there is no payment layer to move it behind.

## Contributing

Contributions are accepted under the Developer Certificate of Origin
(inbound = outbound, Apache-2.0 OR MIT) across the entire repository; sign off your commits
(`git commit -s`). See [CONTRIBUTING.md](CONTRIBUTING.md).
