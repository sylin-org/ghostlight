# Ghostlight guides

Task-oriented guides. Each one owns its topic; the persona walkthroughs point back to the
mechanics guides instead of repeating them, so nothing here drifts out of sync with another page.

| If you want to...                          | Read                                                       |
| ------------------------------------------ | ---------------------------------------------------------- |
| Install Ghostlight and verify it works     | [installation.md](installation.md)                         |
| Run the non-author first-success gate      | [greenfield-first-success.md](../testing/greenfield-first-success.md) |
| Get going fast as a solo developer         | [solo-developer.md](solo-developer.md)                     |
| Publish and grow any open-source project   | [open-source-publication.md](open-source-publication.md)   |
| Write and apply a governance policy        | [governance-configuration.md](governance-configuration.md) |
| Roll governance out across an organization | [compliance-team.md](compliance-team.md)                   |
| Send the audit trail to your SIEM          | [siem-integration.md](siem-integration.md)                 |
| Enter or check a license key (paid tier)   | [licensing.md](licensing.md)                               |
| Review Ghostlight for procurement or security | [Trust Center (procurement and security review)](../trust/README.md) |

## Reference

The reference sources, generated from the binary or maintained with the implementation, so a guide
does not need to repeat them:

- `ghostlight config docs`: every configuration key and its meaning.
- `ghostlight config schema`: JSON Schema for the user config file.
- [open-spec/](../../open-spec/): the RAWX capability model, vendor-neutral.
- [examples/](../../examples/): ready-to-adapt policy manifests.
- [../SPEC.md](../SPEC.md): the original deep design; accepted ADRs and the live tree supersede it
  where they differ.
