# Testing 2.0 on Bluefin and Alpine: the owner's steps

The owner tests the 2.0 release candidate on Bluefin and Alpine; linux-claude tested it on CachyOS
([linux-2 ledger](../tasks/linux-2/LEDGER.md), L2-15). These are the steps, from the release
workflow's bundle, with what each result means. Nothing here needs a release tag.

## Get the bundle

1. Download the `release-candidate` artifact of the release workflow run named in the ledger
   (L2-11), from the run's page on GitHub or with
   `gh run download <run id> -n release-candidate -D ~/Downloads/release-candidate`. Unzip it if
   the browser saved a ZIP. The folder holds `SHA256SUMS`, `release-candidate.json` and `assets`.
2. Note the bundle's revision: `sourceRevision` in `release-candidate.json`.
3. Get the extension from the same revision, because the store has no 2.0 yet:
   `https://github.com/sylin-org/ghostlight/archive/<revision>.zip`, unzipped. Its `extension`
   folder is the one to load. The bundle's own extension ZIP is the store upload; loaded unpacked it
   gets an ID the browser registration refuses.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate sh
```

The first line printed is `Ghostlight 2.0.0 copied from <folder>. Checksums verified.` (or
"Checksums and build provenance verified." with the GitHub CLI installed). A line that starts
with `ghostlight:` and names `SHA256SUMS` or a checksum means the folder is incomplete or changed:
download it again.

## Bluefin

Bluefin is Fedora Atomic: a read-only `/usr`, SELinux enforcing, GNOME, browsers from Flathub
and command-line tools from Homebrew. Ghostlight installs only under the home folder.

| Step | Expected | If not |
| --- | --- | --- |
| The install above | The install summary, and the window opens on its setup steps | A defect: keep the output |
| With only Flathub browsers | The install and `ghostlight doctor` say no supported native browser was detected and to install Chrome, Edge, Brave, or Chromium as a native package | 2.0 has no Flatpak browser route (ADR-0197 Decision 22); this result is the design, not a defect |
| A native browser, to go further | Layer one (`rpm-ostree install chromium`, reboot), then `ghostlight install`; doctor names Chromium as registered. Applications calls the layered one "Chromium Web Browser" and the Flathub one "Chromium"; use the first, or uninstall the Flathub one | A defect if doctor does not see the layered browser |
| The extension | In `chrome://extensions`, Developer mode, Load unpacked, the `extension` folder from step 3; the window moves past the browser step | A defect if the window stays on the browser step with the browser open |
| An agent | Install it as usual (Homebrew is fine), then `ghostlight install`; the agent is listed | A defect if a detected agent is not listed |
| The first task | Ask the agent: "Open sylin.org/ghostlight/demo/desk and ring the bell." The page shows the answer under the bell | A defect: keep `ghostlight doctor` output |
| SELinux | `sudo ausearch -m avc -ts recent` lists no denial that names Ghostlight after the task | Any denial naming Ghostlight is a defect: keep it |
| The tray | The Ghostlight icon in the top bar; Bluefin ships GNOME's AppIndicator support | Without the icon, `ghostlight open` still opens the window |
| Uninstall | `ghostlight uninstall` prints `Ghostlight is uninstalled.` and stops the running Ghostlight | A defect: keep the output |

## Alpine

The release has Linux images for glibc systems only. Alpine uses musl, so the images cannot start
there (test-03, September), and the owner chose on 2026-10-19 not to build musl images for 2.0
(ledger Notes, "Scope").

| Step | Expected | Meaning |
| --- | --- | --- |
| The install above | The checksums verify, then the shell says the service is `not found` and `ghostlight: installation failed` | The image's glibc loader, `/lib64/ld-linux-x86-64.so.2`, is absent. Expected for 2.0, not a regression |

Anything else on Alpine, such as a checksum refusal, is worth keeping.

## Reporting

For each system, the useful record is: the bundle's revision, the system and its version, the
browser and how it was installed, each step's result, and `ghostlight doctor` output for any step
that failed. Page text, screenshots and secrets stay out of it.
