# Ghostlight Licensing Guide

Ghostlight is open-core. This page is the plain-language guide; the license
files govern. Decision record:
[ADR-0027](docs/adr/0027-open-core-business-model-and-licensing.md).

## The split

| Part | License | SPDX |
|---|---|---|
| Engine: everything outside `crates/orchestrator/src/governance/` -- the orchestrator application, browser engine, relays, desktop workbench, and installers | Apache-2.0 OR MIT, at your option ([Apache-2.0](LICENSE), [MIT](docs/licenses/MIT.txt)) | `Apache-2.0 OR MIT` |
| Governance module: `crates/orchestrator/src/governance/` -- authority snapshots, protected-host ceiling, local and managed restrictions, runtime controls, and payload-free audit | Ghostlight Commercial License, source-available ([license text](docs/licenses/LicenseRef-Ghostlight-Commercial.txt)) | `LicenseRef-Ghostlight-Commercial` |

## Am I free to use it?

| You | Engine | Governance module |
|---|---|---|
| An individual or solo developer, including operational use and your own one-person business | Free | Free |
| A nonprofit organization or an open-source project (noncommercial use) | Free | Free |
| A company evaluating, developing, or testing | Free | Free |
| A team of up to 5 people, including operational governance use | Free | Free |
| A company running operationally in all-open mode (no governance manifest or org policy configured) | Free | Free (the governance layer is a pass-through) |
| An organization of more than 5 people running governance operationally (manifests, org policy, audit -- what the license text calls "production use") | Free | Commercial subscription |

If your row says "commercial subscription", contact hello@sylin.org. In short: exactly
one situation pays, and everything else is free -- see [PRICING.md](PRICING.md) for the
tiers, plus the hardship and outgrew-the-tier accommodations. Ghostlight 1.0 has no runtime key,
activation, status command, or behavior gate; [docs/guides/licensing.md](docs/guides/licensing.md)
explains the source boundary.

## Vendored third-party code

One dependency ships inside the browser extension rather than through a package manager, because
the extension never loads remote code:

| File | Component | License |
|---|---|---|
| `extension/vendor/gifenc.js` | [gifenc](https://github.com/mattdesl/gifenc) 1.0.3, a JavaScript animated GIF encoder | MIT ([text](extension/vendor/gifenc.LICENSE.md)) |

It is pinned to an exact version and reviewed like any other dependency
([ADR-0109](docs/adr/0109-browser-owned-gif-encoding.md)). Rust dependencies are declared in
`Cargo.toml` and resolved normally.

## Labels, precisely

- The engine is open source (OSI-approved licenses).
- The governance module is source-available: the code is published and
  inspectable, but it is not open source and not "Fair Source".
- The product as a whole is open-core.

## Commitments

- The engine stays Apache-2.0 OR MIT. It will not be relicensed.
- A bug fix, a security fix, or a core automation capability is never moved
  behind payment.
- A later version of the commercial license will not retroactively narrow
  rights granted by the version you received the software under.

## Contributing

- Engine contributions are accepted under the Developer Certificate of
  Origin (inbound = outbound, Apache-2.0 OR MIT).
- Contributions to `crates/orchestrator/src/governance/` are not open yet; if you want to
  contribute there, open an issue first (a CLA will be required).
