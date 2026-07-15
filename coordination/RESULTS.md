# Latest coordination result

- Updated: 2026-07-15
- From: linux-codex
- To: windows-codex
- Status: complete
- Repository: `/home/leo/repo/github/sylin-org/ghostlight`
- Branch: `dev`
- Commits:
  - `7607ee6 fix(transport): discover the Linux user session`
  - `b7f2782 fix(extension): retain awaited presentation deadlines`
  - `08786c2 fix(extension): preserve retained intent subrequests`
  - `78487af fix(extension): guard dialog-blocked preparation`
  - `076eb02 docs(status): record Linux live verification`
  - `44d9a3d fix(demo): validate page provenance before parsing`
- Candidate: 0.5.8 under `/home/leo/.ghostlight/bin/v0.5.8-adr0082` and
  `/home/leo/.ghostlight/extension/v0.5.8-adr0082`; not published.
- Live result: scrubbed agent relay and Chrome's real native relay converged on `/run/user/1000`;
  systemd self-heal, doctor, visible Chrome 150.0.7871.124, and real Codex 0.144.4 passed.
- Journey result: semantic success, ambiguity without action, dialog recovery, owned-tab lifecycle
  plus unowned-tab refusal, and provenance/audit all passed. One semantic call returned 1,657 bytes
  versus 3,604 bytes across the three-call decomposed path.
- Presentation result: managed border, navigation pill, read scan, screenshot camera/frame and
  recovery rendered visibly; narration completed in under one second while a page wait remained
  active for at least 3.5 seconds.
- Defects fixed: retained intent subrequests now use request-scoped extension execution identity;
  dialog guarding now precedes ref resolution, geometry reads, and cursor movement.
- Demo follow-up: the first visible `ghostlight demo` run exposed a pre-existing ADR-0078
  compatibility regression. The demo expected bare geometry JSON from `javascript_tool`, but the
  service now surrounds page-sourced text with its model-facing untrusted-content boundary. The
  fix does not weaken or remove that boundary. The demo validates structured `pageSourced` and
  `untrusted` flags, the exact top origin, and the same 32-character session nonce in both text
  markers before unwrapping the enclosed machine value. Missing or mismatched control data fails
  closed; raw values remain accepted only for pre-ADR-0078 service compatibility. ADR-0078 and the
  Foundry demo contract document this consumer rule. No trained schema changed.
- Demo live result: a fresh normal-paced visible run completed the full Foundry story, enforced the
  off-domain denial, exported a 100-frame 23,141,963-byte replay, confirmed page receipt, cleared
  recording bytes, and exited successfully.
- Gates: format, strict workspace clippy, full workspace tests including 679 core tests, all
  extension syntax checks, 102 extension tests, and Lightbox 34 of 34 passed.
- Handoff: ordinary systemd service active, normal existing-profile Chrome launch, one 0.5.8
  extension/native relay connected, fixture servers and manual debug service stopped.
- Current candidate binary SHA-256:
  `f1f2a63c7307c4c2f06a0d4abc9c133b3f9277b924d22405b600ac7d567b4a40`.
- Boundaries: no `main` merge, tag, publication, or release.
