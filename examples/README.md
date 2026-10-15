# Ghostlight policy examples

These files use the strict schema-4 policy format the orchestrator decodes. Every one of them is
checked by the test suite, so an example that stops being valid fails the build rather than
misleading you.

## How to try one

Copy a file to the policy Ghostlight owns and it applies on the next action, with no restart:

- Windows: `%LOCALAPPDATA%\Ghostlight\user-policy.json`
- Linux: `$XDG_STATE_HOME/ghostlight/user-policy.json`

Then open the Policy page in the workbench to see what it did, and edit it there. Removing it is one
click on that page, or delete the file.

`GHOSTLIGHT_POLICY_FILE` still works and still wins when set, but Ghostlight does not own that path:
it reads that file and never writes back, so the workbench shows it read-only.

Check any file before you use it. Both commands run the production parser, and neither starts a
browser or writes audit:

```sh
ghostlight policy validate examples/personal-starter.json
ghostlight policy explain examples/personal-starter.json
ghostlight policy simulate examples/personal-starter.json audit.jsonl
```

Observe mode was removed in 2.0. `ghostlight policy simulate` previews a policy against recorded
audit instead: it lists the recorded work the policy would have refused. A file still in schema 3
moves with `ghostlight policy migrate <file>`, which keeps a `.schema3.bak` copy and refuses to
move a file whose meaning would change.

## Two ways a policy points

This is the distinction worth understanding before adapting anything. Rules do not "configure" a
baseline; they *are* the baseline once a policy exists.

- **Named sites, closed baseline.** List the hosts you want and nothing else is admitted. Most
  examples here work this way, and the workbench reads it back as "some sites allowed".
- **Universal site with holes, open baseline.** Grant `"*"` and carve exceptions out with `deny`.
  The workbench reads that as "some sites blocked".

Both leave a capability available "on some sites" and they are opposite situations, which is why the
window names which one you have.

Remember that a capability nobody grants is refused everywhere. Leaving `execute` out of every rule
is how you turn off page-code execution; there is no separate switch for it.

## Start here

| File | What it shows |
| --- | --- |
| [personal-starter.json](personal-starter.json) | The one to copy first. Two named-site rules. Run `ghostlight policy simulate` against your recorded audit to see what it would have refused before you rely on it. |
| [no-page-code.json](no-page-code.json) | Ordinary work anywhere, with page-code execution left out of every rule and therefore refused everywhere. The common "everything except running JavaScript" ask. |
| [personal-everywhere-except.json](personal-everywhere-except.json) | The other polarity: `"*"` with holes cut in it, plus never-touch destinations and model-driven tab close turned off. |

## Written by an organization

Both of these carry the optional `organization` block, which is informational and never decides
anything. It exists so the person being governed can see who is restricting them and where to ask.
Delivered for real, these are signed into a bundle; the manifest inside is exactly this.

| File | What it shows |
| --- | --- |
| [organization-support.json](organization-support.json) | A worked support-desk policy: named identity, contacts, two rules with a carve-out, and three settings. |
| [organization-locked-fleet.json](organization-locked-fleet.json) | One agreed configuration for a fleet: read and ordinary interaction on the approved applications, model-driven tab closing off, and `browser.startup: manual`, which keeps missing-browser recovery diagnostic-only. Supply a `statement` so the person reads a reason for the limits. A rule set on a machine can only narrow it further. |

## Older examples, kept

| File | What it shows |
| --- | --- |
| [developer-unrestricted.json](developer-unrestricted.json) | All-open written out explicitly. With no policy configured at all, Ghostlight is already this. |
| [research-read-only.json](research-read-only.json) | Reading a short list of research sites and nothing else. |
| [qa-staging.json](qa-staging.json) | Ordinary work across one domain with the admin console carved out. |
| [enterprise-healthcare.json](enterprise-healthcare.json) | Named internal applications with a carve-out. |
| [demo-policy.json](demo-policy.json) | The sylin.org demo surface. |
| [dev-live-test.json](dev-live-test.json) | The live browser-journey fixture. |

## Things that will bite you

- The document is typo-closed. An unknown field, an unregistered setting key, a misspelled
  capability, or a malformed host pattern makes the whole file invalid, and a configured source with
  no valid policy fails closed. Validate before you deploy.
- There is nowhere to put a comment. JSON has no comment syntax and the parser refuses unknown
  fields, so the per-rule `description` is the only place an explanation survives. Use it; the
  workbench shows it.
- Host patterns are `*`, an exact hostname, or one leading `*.` suffix. `*.example.com` covers
  subdomains **only** -- it does not match `example.com` itself. List both when you mean both.
- Rules are checked in written order and the first one that admits the whole operation wins, so a
  broad rule above a narrow one makes the narrow one dead. The workbench marks that in place.
- Layers intersect and only ever tighten. A user policy cannot re-open anything an organization
  closed, and no policy can reach localhost, loopback, link-local, or non-HTTP(S) addresses.
- `browser.startup` accepts only `on_demand` or `manual`. Windows defaults to `on_demand`; Linux
  defaults to `manual`. An organization-authored `manual` value pins the effective choice.
- Schema 4 is the policy format from 2.0 on. An older Ghostlight refuses it, so move a mixed fleet
  to 2.0 before you deploy these files.

See [`../docs/guides/governance-configuration.md`](../docs/guides/governance-configuration.md) for
the full reference, and [`../docs/1.0/LANGUAGE.md`](../docs/1.0/LANGUAGE.md) for the exact operation
map behind read, action, write, and execute.
