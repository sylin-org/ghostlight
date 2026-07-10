# Ghostlight Licensing Guide

Ghostlight is open-core. This page is the plain-language guide; the license
files govern. Decision record:
[ADR-0027](docs/adr/0027-open-core-business-model-and-licensing.md).

## The split

| Part | License | SPDX |
|---|---|---|
| Engine: everything outside `src/governance/` -- the automation engine, the 13 trained tools, the Chromium extension, the CLIs and installers | Apache-2.0 OR MIT, at your option ([LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT)) | `Apache-2.0 OR MIT` |
| Governance module: `src/governance/` -- identity-bound grants, org policy locks, structured audit, sacred never-touch domains, the `explain` tool, central management | Ghostlight Commercial License, source-available ([LICENSE-GOVERNANCE](LICENSE-GOVERNANCE)) | `LicenseRef-Ghostlight-Commercial` |

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
tiers, plus the hardship and outgrew-the-tier accommodations. Once you have a license,
[docs/guides/licensing.md](docs/guides/licensing.md) shows how to install and refresh it.

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
- Contributions to `src/governance/` are not open yet; if you want to
  contribute there, open an issue first (a CLA will be required).
