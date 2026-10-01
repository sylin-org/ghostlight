//! Human guidance following a completed action and an unsuccessful observation.

/// A separate observation failure never makes repeating the completed action safe.
pub const APPLIED_BEFORE_CHECK_FAILURE: &str =
    "Keep the confirmed change. Inspect the page before preparing unfinished work.";

/// Exact tab reveal is observation, separate from human takeover and browser authority.
pub const SHOW_TAB_GUIDANCE: &str = "Show tab lets you look. Pause before taking over. Showing a tab does not change permissions or resume work.";

/// Explain global runtime scope and the current browser connection independently.
#[must_use]
pub fn detail(
    state: ghostlight_bridge::browser::RuntimeControlState,
    browser_connected: bool,
) -> String {
    use ghostlight_bridge::browser::RuntimeControlState;
    let instruction = match state {
        RuntimeControlState::Active => "Pause prevents new browser effects across all Ghostlight sessions. Completed changes stay in place.",
        RuntimeControlState::Held => "Paused across all Ghostlight sessions. Resume permits new requests without repeating earlier actions.",
        RuntimeControlState::Attention => "Browser work is held across all Ghostlight sessions. Resume permits new requests without repeating earlier actions.",
        RuntimeControlState::Ended => "Stopped across all Ghostlight sessions. Start session permits new requests without repeating earlier actions.",
    };
    if browser_connected {
        instruction.into()
    } else {
        format!("{instruction} Controls apply now and remain in effect when a browser reconnects.")
    }
}

/// Confirm an accepted human intent without presenting it as a browser failure.
#[must_use]
pub fn confirmation(
    state: ghostlight_bridge::browser::RuntimeControlState,
    browser_notified: bool,
) -> String {
    use ghostlight_bridge::browser::RuntimeControlState;
    let message = match state {
        RuntimeControlState::Held => "Ghostlight is paused across all sessions.",
        RuntimeControlState::Ended => "Ghostlight is stopped across all sessions.",
        RuntimeControlState::Active => {
            "Ghostlight can accept new requests. Earlier actions will not be repeated."
        }
        RuntimeControlState::Attention => "Ghostlight is waiting for you across all sessions.",
    };
    if browser_notified {
        message.into()
    } else {
        format!("{message} The browser will receive this state after reconnecting.")
    }
}
