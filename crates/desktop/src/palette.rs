//! The window's palette as the shell draws it outside the webview: the tray's dots, the window's
//! ground and the state screens. Each colour is a token of the window's stylesheet
//! (`crates/orchestrator/ui/styles.css`), and the test below reads it there, so the shell and the
//! window cannot drift apart.

/// A colour as red, green and blue.
pub type Rgb = [u8; 3];

/// The night ground, `--bg`.
pub const GROUND: Rgb = [0x0f, 0x0e, 0x12];

/// The ink of a heading, `--ink`.
pub const INK: Rgb = [0xf4, 0xf4, 0xf5];

/// The ink of body text, `--ink-2`.
pub const INK_2: Rgb = [0xc9, 0xc9, 0xd1];

/// The accent, `--a`.
pub const ACCENT: Rgb = [0x5e, 0xea, 0xd4];

/// What a pause holds, `--held`.
pub const HELD: Rgb = [0xfb, 0xbf, 0x24];

/// What needs attention, `--danger`.
pub const DANGER: Rgb = [0xf8, 0x71, 0x71];

/// `colour` as a CSS colour, `#rrggbb`.
pub fn hex(colour: Rgb) -> String {
    let [red, green, blue] = colour;
    format!("#{red:02x}{green:02x}{blue:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract: every colour the shell draws is the token of the same name in the window's
    /// stylesheet, so the tray, the ground and the state screens wear the window's palette.
    #[test]
    fn every_colour_is_a_token_of_the_windows_stylesheet() {
        let styles = include_str!("../../orchestrator/ui/styles.css");
        for (token, colour) in [
            ("--bg", GROUND),
            ("--ink", INK),
            ("--ink-2", INK_2),
            ("--a", ACCENT),
            ("--held", HELD),
            ("--danger", DANGER),
        ] {
            let declared = format!("{token}: {};", hex(colour));
            assert!(
                styles.contains(&declared),
                "{declared} is not in the stylesheet"
            );
        }
    }
}
