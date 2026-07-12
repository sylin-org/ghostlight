// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Namespaced wire-vocabulary constants: the magic strings and the composite-tabId arithmetic
//! that would otherwise be repeated (and drift) across call sites. Grouped by concern, not
//! dumped flat -- each sub-module documents the ONE wire shape its constants belong to.

/// The on-screen notification vocabulary (SAPS PRES-HIGH-01; see `crate::hub::outbound::browser::
/// Browser::notify` and `crate::messages` for the full wire shape this fills in).
pub mod notification {
    /// `class` values: the standard severity taxonomy this codebase's own `tracing` levels
    /// already use, so the notification primitive stays general-purpose rather than
    /// denial-specific.
    pub mod class {
        pub const ERROR: &str = "error";
        pub const WARN: &str = "warn";
        pub const INFO: &str = "info";
        pub const DEBUG: &str = "debug";
    }

    /// `icon` values in current use. Additive: a new icon is a new constant, never a rename of
    /// one already in use (the extension's `NOTIF_ICON_SVG` keys off `class`, not `icon`, today,
    /// so these are documentation of intent more than a rendering switch -- kept here so a call
    /// site never hand-types the string).
    pub mod icon {
        pub const LOCK: &str = "lock";
        pub const SHIELD: &str = "shield";
    }
}

/// Composite tab identifiers (ADR-0058): a `tabId` that crosses the wire to the MCP client
/// encodes BOTH the owning browser's pid and the extension's own native Chrome tab id, as a
/// single JSON number -- so routing a later call needs no server-side tabId->browser lookup
/// table, and the trained `"tabId": {"type": "number"}` schema (`crate::browser::directory`)
/// never changes shape. The extension itself never learns this encoding exists: only
/// `crate::mcp::pipeline` (decoding an inbound composite id to route the call, and encoding an
/// outbound native id in a tool result) touches these.
pub mod tab_id {
    /// Bounds the native (extension-side) tab id to `2^32` (matching Chrome's internal tab id,
    /// a 32-bit int -- LIVE-VERIFIED 2026-07-11 against a real browser: observed native ids up
    /// to `1_246_199_197`, over a billion, ruling out an earlier `10_000_000` bound that assumed
    /// a small per-launch counter and silently overflowed into the pid digits on decode) and
    /// keeps `browser_pid * MULTIPLIER` inside JavaScript's safe-integer range (2^53) for any
    /// realistic OS pid (`(2^53-1) / 2^32 =~ 2.1 million`, far above any pid a real OS assigns).
    pub const MULTIPLIER: i64 = 1i64 << 32;

    /// Combine a browser's pid and the extension's native tab id into one composite JSON number.
    pub fn encode(browser_pid: u32, native_tab_id: i64) -> i64 {
        (browser_pid as i64) * MULTIPLIER + native_tab_id
    }

    /// Split a composite tab id back into `(browser_pid, native_tab_id)`. Not meaningfully
    /// fallible: any `i64` decodes to SOME pair, even if the value never came from [`encode`] (a
    /// caller that used a plain small native id un-encoded just decodes to `browser_pid: 0`,
    /// which will simply fail to match any live session -- a clear "browser not connected"
    /// error, not a panic or a silent misroute).
    pub fn decode(composite: i64) -> (u32, i64) {
        let browser_pid = (composite / MULTIPLIER).max(0) as u32;
        let native_tab_id = composite.rem_euclid(MULTIPLIER);
        (browser_pid, native_tab_id)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn round_trips_realistic_values() {
            for (pid, tab) in [(1u32, 0i64), (4242, 7), (999_999, 9_999_999), (12345, 5)] {
                let composite = encode(pid, tab);
                assert_eq!(decode(composite), (pid, tab), "pid={pid} tab={tab}");
            }
        }

        /// Regression oracle (2026-07-11 live verification): the EXACT pid and native tab id a
        /// real browser produced, which broke the earlier `10_000_000` multiplier by overflowing
        /// the native part into the pid digits on decode (`21840` decoded back as `21964`).
        #[test]
        fn round_trips_the_live_verified_billion_scale_native_id() {
            let (pid, native) = (21840u32, 1_246_199_197i64);
            assert_eq!(decode(encode(pid, native)), (pid, native));
        }

        #[test]
        fn multiplier_leaves_headroom_for_a_full_32_bit_native_id_and_realistic_pids() {
            // Chrome's tab id fits a signed 32-bit int; a realistic OS pid (even a generous
            // upper bound of 2 million) must still round-trip without colliding into the next
            // pid's range or exceeding JavaScript's safe-integer ceiling (2^53 - 1).
            let max_native = i32::MAX as i64;
            let pid = 2_000_000u32;
            let composite = encode(pid, max_native);
            assert!(
                composite < (1i64 << 53),
                "composite must stay within JS safe-integer range"
            );
            assert_eq!(decode(composite), (pid, max_native));
        }

        #[test]
        fn decode_of_a_plain_small_native_id_never_panics() {
            // A caller that forgot to encode (or an old client echoing a pre-ADR-0058 value)
            // decodes to a pid that will not match any live session -- handled as an ordinary
            // routing miss downstream, not a special case here.
            let (pid, tab) = decode(5);
            assert_eq!(pid, 0);
            assert_eq!(tab, 5);
        }
    }
}
