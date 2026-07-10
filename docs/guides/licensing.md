# Entering and refreshing a Ghostlight license

How to install, check, and renew a Ghostlight license key. Most people never need this:
a license is required for exactly one situation -- an organization of more than five
people running the governance features operationally ([PRICING.md](../../PRICING.md)).
Individuals, evaluation, all-open use, nonprofits, and teams of five or fewer never need a
key. If that is you, skip this page; nothing here changes how Ghostlight behaves.

A license never gates behavior. Installing, expiring, or not having one changes exactly one
thing: a `license` marker in your own audit records while governance is operating. Your
deployment keeps working either way (the Continuity Promise).

## What a license looks like

A license is a small, offline-verifiable file. You will receive it in one of two forms,
and both install the same way:

- an **armored block** (easy to paste), like an SSH key:

      -----BEGIN GHOSTLIGHT LICENSE-----
      eyJ2IjoxLCJrZXlnZW4iOjEsImNsYWltcyI6Ii4uLiIsInNpZyI6Ii4uLiJ9
      ...
      -----END GHOSTLIGHT LICENSE-----

- or a **`license.json`** file.

The binary verifies it locally with a composite Ed25519 + ML-DSA (post-quantum) signature.
There is no activation server and no network call, ever.

## Getting your license

After you buy a subscription (or join the founding program), we email you the license --
an armored block, or a `license.json` attachment. There is nothing to activate. Keep the
email; re-installing is just pasting the same block again.

## Installing it

All three of these do the same thing: validate the license and store it. The default is a
per-user install; add `--org` for a system-wide install (see [Where it lives](#where-it-lives)).

**Paste the armored block** (no file needed). Run:

    ghostlight license install

then paste the whole block (including the `BEGIN`/`END` lines) and send end-of-input:
Ctrl+Z then Enter on Windows, or Ctrl+D on macOS and Linux.

**Install from a saved file** (a `.json` envelope or a `.txt` armored block):

    ghostlight license install path/to/license.json

**Pipe it in** (handy in scripts):

    # Windows PowerShell
    Get-Content license.txt | ghostlight license install
    # macOS / Linux
    ghostlight license install < license.txt

On success it prints where it stored the license. An invalid license is refused with a
reason; an expired one installs with a warning (it still never affects behavior).

## Checking what is installed

    ghostlight license status

prints the resolved state (valid / evaluation / expired / invalid / none) and, when a
license is present, its tier, licensee, seats, and expiry. `ghostlight doctor` shows the
same in its `License:` section. Both are read-only; they never change anything.

To inspect a license file before installing it:

    ghostlight license status --file path/to/license.json

## Refreshing or renewing

When your term renews we email you a new license. Install it exactly as above -- it
overwrites the old one in place:

    ghostlight license install path/to/renewed-license.json
    ghostlight license status        # confirm the new expiry

Nothing needs to be stopped or restarted for the *audit marker* to clear on the next
service start, and nothing breaks in the meantime: an expired license keeps working, so
there is no renewal deadline that can interrupt you. Renew when procurement is ready.

## Where it lives

`ghostlight license install` writes `license.json` to the per-user location by default, or
to the org-wide location with `--org` (which a central deployment uses so every user on the
machine is covered; writing there needs administrator rights). The service reads the
org-wide file first, then the per-user file.

| Scope | Windows | macOS | Linux |
|---|---|---|---|
| Per-user (default) | `%APPDATA%\ghostlight\license.json` | `~/Library/Application Support/ghostlight/license.json` | `~/.config/ghostlight/license.json` |
| Org-wide (`--org`) | `%ProgramData%\ghostlight\license.json` | `/Library/Application Support/ghostlight/license.json` | `/etc/ghostlight/license.json` |

`ghostlight doctor` prints the exact resolved path under `License:`. To remove a license,
delete that file.

## What the states mean

`ghostlight license status` (and the audit `license` marker, when governance is operating)
can report:

| State | Meaning |
|---|---|
| `valid` | An in-date paid license. No audit marker. |
| `evaluation` | A self-signed or founder-issued evaluation license. Marked `evaluation` -- fine for trying things, never a paid production license. |
| `expired` | A past-term license. Still works; renew when ready. Marked `expired`. |
| `invalid` | Present but unusable (wrong signature, corrupted, or a development key claiming a paid tier). Marked `invalid`. |
| `none` / `unlicensed` | No license installed. Only surfaces as `unlicensed` in audit when an org policy is operational and no license is present. |

The audit marker appears **only** while governance is actually operating via an
org-deployed policy. In all-open use, or with a personal manifest, the licensing layer is
dormant and writes nothing.

## Questions

See [PRICING.md](../../PRICING.md) for who needs a license and the standing hardship and
grace accommodations, [LICENSING.md](../../LICENSING.md) for the plain-language license
terms, or email **hello@sylin.org** for anything about billing, keys, or procurement.
