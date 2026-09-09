# Windows fleet peer attribution, 2026-09-09

Starting source: `a8cfd0337d83079f5fad3d5dfec846c31ba3b8fd`.
Host: Windows 11 Pro 25H2, build 26200.9445, x64, Rust 1.95.0.

The release-profile orchestrator suite passed 445 tests and failed
`a_live_connection_observes_the_executable_without_retaining_process_details`.
The focused win-peer suite also failed `a_live_loopback_pair_identifies_its_own_process`:
the live pair was absent from the returned rows. Repetition preserved the failure.

A separate read-only probe using the documented Windows declaration returned both
loopback endpoints immediately, in compartment 1. This ruled out a general inability
of this caller to observe its own sockets. The Rust declaration used `u16` for `ulAf`;
the Windows ABI requires `ULONG`, or `u32`. The narrow declaration allowed an incorrect
argument at the FFI boundary. Both the declaration and its constant now use `u32`.
See Microsoft's [GetExtendedTcpTable signature](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable).

The original focused release test passes after this change. Formatting,
warnings-denied workspace Clippy, the complete workspace test suite, and all 210
extension tests pass. Existing live tests cover the failure; no weaker fallback or
environment-based skip was added. Attribution semantics, collected data, and the
deferred signer-admission decision are unchanged.

This is source verification. Installing and exercising the resulting package remains
separate fleet work. The initial installed candidate and its failures are preserved
under the host's ignored `.tmp/fleet-leo-desktop-02` evidence directory.
