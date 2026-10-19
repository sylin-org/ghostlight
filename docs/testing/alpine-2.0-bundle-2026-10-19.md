# Alpine and the 2.0 release candidate bundle

The Alpine step of [the owner's page](linux-other-environments-2026-10-19.md), run on test-03 on
2026-10-19 by a Claude Code session at the owner's direction. Both bundles failed the way the page
says they should. Nothing on the machine changed.

## The system

- Alpine Linux 3.24.1, kernel 6.18.48-0-lts, x86-64, musl. KDE Plasma on Wayland.
- No glibc loader: `/lib64` does not exist, so `/lib64/ld-linux-x86-64.so.2` is absent.
- Browser: the machine's real Chromium, 152.0.7977.82, on its default profile in
  `~/.config/chromium`. It is Alpine's own package, unpacked under the home folder during the
  September fleet pass (no root), and its launcher still loads the 1.3.5-era extension. It was not
  started: the install stops before any browser step, and no other browser or profile was used.
- The GitHub CLI 2.102.0 was put in `~/.local/bin` from its official release (checksum checked) to
  download the bundles, so `get.sh` could also verify build provenance.
- Already present and left alone: the September 1.3.5 musl development install
  (`~/.local/bin/ghostlight` links into `~/ghostlight-fleet/test-03/installed/1.3.5-musl-debug/`),
  running before and after both runs.

## The bundles

| Release run | Revision | Run result |
| --- | --- | --- |
| 39297671342 | `149c04d67b27f68f211137f6b6976d788dcca4ed` | success (the run L2-11 names) |
| 39306759480 | `68419db8cbdf6517ca7b86783e2f6971391bd660` | success, both install-from-bundle jobs included |

Each `release-candidate` artifact was downloaded with `gh run download <run id> -n
release-candidate` into its own folder under `~/Downloads`. Both manifests say version 2.0.0. The
`get.sh` served at each revision is the same file as the tree at that revision.

## The install

The page's command, at each bundle's own revision:

```sh
curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate-<run id> sh
```

Both runs printed the same three lines, and the shell exited 1:

```text
Ghostlight 2.0.0 copied from /home/test/Downloads/release-candidate-<run id>. Checksums and build provenance verified.
sh: /tmp/ghostlight-install.<random>/ghostlight-service: not found
ghostlight: installation failed
```

| Step | Expected | Result |
| --- | --- | --- |
| The install, bundle 149c04d6 | Checksums verify, then `not found` and `ghostlight: installation failed` | As expected |
| The install, bundle 68419db8 | The same | As expected |

`readelf -l` on each bundle's `ghostlight-service-x86_64-unknown-linux-gnu` names
`/lib64/ld-linux-x86-64.so.2` as its interpreter. So the `not found` is about the missing loader,
not the file. That is the 2.0 scope the owner chose (no musl images), not a regression.

## After each run

- The temporary folder `get.sh` made under `/tmp` was gone.
- No file in the home folder with Ghostlight, sylin or native messaging in its path changed after
  the run started. The service never ran, so nothing was installed or registered.
- The 1.3.5 musl install and its running process were unchanged.

Not tried, because the install stops first: the extension, an agent, the first task, the tray and
uninstall. There was nothing to uninstall.
