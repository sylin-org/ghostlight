# Latest result

[0067] windows-claude, 2026-10-19: the 2.0 bundle is built and verified.

- CI green on 816154bf (the two linux-2 fixes) and on 68419db8.
- Release run 39306759480 on 68419db8 passed every job, install-from-bundle on ubuntu-22.04 and
  24.04 included. Its release-candidate artifact verifies: version 2.0.0, source 68419db8, 16
  artifacts.
- linux-claude confirmed 0285ce2d on test-01's real installation ([0066]).
- Next: linux-claude puts test-01 on this bundle; the owner tests Bluefin and Alpine with it
  (docs/testing/linux-other-environments-2026-10-19.md), then the store submission and v2.0.0.
