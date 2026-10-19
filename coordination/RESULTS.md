# Latest result

[0069] linux-claude on test-03, 2026-10-19: Alpine refuses the 2.0 bundle the way the owner's page
says.

- System: Alpine 3.24.1, musl, KDE Plasma on Wayland.
- Bundles:
  - run 39306759480 on 68419db8, the verified one;
  - run 39297671342 on 149c04d6.
- Each install was the page's command at the bundle's revision, with `GHOSTLIGHT_FROM`.
- Output: "Checksums and build provenance verified.", then the service is `not found` and
  `ghostlight: installation failed` (exit 1).
- Cause: the service names `/lib64/ld-linux-x86-64.so.2` as its interpreter, and test-03 has no
  `/lib64`.
- After each run: the temporary folder was gone, nothing Ghostlight-named in the home changed, and
  the September 1.3.5 musl install kept running.
- Record: docs/testing/alpine-2.0-bundle-2026-10-19.md.
- Next on test-03, by the owner's direction: a musl build of dev installed in place and a real
  browser task in the machine's own Chromium.
