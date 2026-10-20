# The public 2.0.0 install on Bluefin

test-02, the owner's Bluefin machine (Bluefin 44.20261013, GNOME Shell 50.5 on Wayland, SELinux
enforcing, Chromium 155 layered with rpm-ostree), on its real installation and the owner's
Chromium, 2026-10-20. The first of windows-claude's two asks after 2.0.0 was released. The second,
the prompt-default change on `dev`, waits for its report.

## Before

The installation was the release-channel 2.0.0 that `get.sh` installed from the candidate bundle
of release run 39323041650 (eba78013) on 2026-10-19
([record](bluefin-2.0-bundle-2026-10-19.md)). All three installed images were already byte-equal
to the v2.0.0 `SHA256SUMS` entries: the release carries the same Linux images as that bundle. So
the same-version case test-01 met (an older candidate service kept,
[record](cachyos-public-2.0.0-2026-10-20.md)) could not change anything here. The extension was
the unpacked one from the bundle's revision, loaded on 2026-10-19.

## The public install command

`https://sylin.org/ghostlight/get.sh` redirects to `scripts/get.sh` on `main`, byte-identical to
the tree at eba78013. The session fetched it and ran that copy with `sh`, as the one-liner does,
with the GitHub CLI present but not signed in, as a person would have it.

| Step | Result |
| --- | --- |
| The public `get.sh` | "Ghostlight 2.0.0 downloaded. Checksums verified." (no provenance line: the GitHub CLI was not signed in), then "Ghostlight 2.0.0 is installed.", Applications, and agents Codex, Claude Code and Zed. Exit 0. |
| The installed images | Unchanged, and each equals its v2.0.0 `SHA256SUMS` entry (`sha256sum -c`). `install.json` says 2.0.0, relay 4, channel release. The running service was left as it was. |
| `ghostlight doctor` | "Not connected" while Chromium was closed. An agent's `browser_navigate` then answered "No browser is connected. Ask the user to open a Chromium browser window...": on Linux the default browser-start posture is `manual`, so Ghostlight leaves the start to the person by design. The owner opened Chromium Web Browser normally (no profile or extension flags), and doctor read Ready, one agent and one browser connected. |
| SELinux | No AVC record from the start of the run through the browser connection. |

Not repeated here: the bell, which the 2.0.0 extension and service answer as the 2026-10-19
record shows; it reruns with the page's own words once the prompt-default change lands.
