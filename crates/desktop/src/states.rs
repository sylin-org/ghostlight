//! The shell's own screens (ADR-0199 Decision 7 and its amendment of 2026-10-15, shell
//! milestone 2).
//!
//! When the shell has no workbench to show it says what is happening, in four states: Starting,
//! Updating, Restarting and Stopped. This module holds the words of each state and the one
//! built-in document that draws them. The document takes the state's words and buttons as
//! parameters. It runs no script, loads nothing and registers no scheme: a button is a link to a
//! fixed address, and [`navigation`] is the webview's whole navigation rule. A press is refused as
//! a navigation and acted on by the shell, so the page needs no IPC handler.
//!
//! The ground is the workbench ground, the type follows the workbench kit, the motion stops under
//! the system's reduced-motion setting, and a forced-colors block hands every colour to the
//! system, so a state screen reads under Windows High Contrast.
//!
//! Everything here is pure and unit tested.

use crate::webview::GROUND;

/// What the Starting screen says.
pub const STARTING_WORDS: &str = "Starting Ghostlight...";

/// What the Updating screen says.
pub const UPDATING_WORDS: &str = "Updating Ghostlight...";

/// What the Restarting screen says.
pub const RESTARTING_WORDS: &str = "Ghostlight is restarting...";

/// What the Stopped screen says.
pub const STOPPED_WORDS: &str = "Ghostlight stopped unexpectedly.";

/// What the Start button says.
pub const START_LABEL: &str = "Start";

/// What the Diagnostics button says.
pub const DIAGNOSTICS_LABEL: &str = "Diagnostics";

/// The address the Start button points at. Nothing is ever loaded from it: the webview refuses the
/// navigation and the shell acts on it. The `.invalid` name is reserved and never resolves.
pub const START_ADDRESS: &str = "https://ghostlight.invalid/start";

/// The address the Diagnostics button points at. See [`START_ADDRESS`].
pub const DIAGNOSTICS_ADDRESS: &str = "https://ghostlight.invalid/diagnostics";

/// The address a document loaded from a string has in the webview, on Linux and for a blank page.
const BLANK_ADDRESS: &str = "about:blank";

/// The start of the address a document loaded from a string has on Windows.
const DOCUMENT_ADDRESS_PREFIX: &str = "data:text/html";

/// The document, with `{ground}`, `{class}`, `{words}` and `{buttons}` to fill in.
const DOCUMENT: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'">
<title>Ghostlight</title>
<style>
:root { color-scheme: dark; --bg: {ground}; --ink: #f4f4f5; --ink-2: #c9c9d1; --a: #5eead4; --danger: #f87171; }
html, body { height: 100%; margin: 0; }
body {
  display: grid; place-items: center;
  background: var(--bg); color: var(--ink-2);
  font: 400 13px/1.65 'Segoe UI', system-ui, Arial, sans-serif;
}
main { display: grid; justify-items: center; gap: 18px; padding: 24px; text-align: center; }
.mark { width: 10px; height: 10px; border-radius: 50%; background: var(--a); }
.working .mark { animation: breathe 2.4s ease-in-out infinite; }
.stopped .mark { background: var(--danger); }
h1 { margin: 0; color: var(--ink); font-size: 15px; font-weight: 300; letter-spacing: -.01em; }
.buttons { display: flex; flex-wrap: wrap; justify-content: center; gap: 10px; }
a.button {
  display: inline-block; padding: 5px 16px; border: 1px solid rgba(255, 255, 255, .14);
  border-radius: 7px; color: var(--ink-2); text-decoration: none;
}
a.button:hover { border-color: rgba(255, 255, 255, .22); color: var(--ink); background: rgba(255, 255, 255, .035); }
a.button:focus-visible { outline: 2px solid var(--a); outline-offset: 2px; }
@keyframes breathe { 50% { opacity: .3; } }
@media (prefers-reduced-motion: reduce) {
  .working .mark { animation: none; }
}
@media (forced-colors: active) {
  .mark, .stopped .mark { background: Highlight; forced-color-adjust: none; }
  h1 { color: CanvasText; }
  a.button { border-color: ButtonBorder; color: ButtonText; }
  a.button:hover { border-color: Highlight; background: Canvas; color: ButtonText; }
  a.button:focus-visible { outline-color: Highlight; }
}
</style>
</head>
<body class="{class}">
<main role="status" aria-live="polite">
<div class="mark" aria-hidden="true"></div>
<h1>{words}</h1>
<div class="buttons">{buttons}</div>
</main>
</body>
</html>
"#;

/// One of the shell's four states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateScreen {
    /// A person launched Ghostlight and the service is coming up.
    Starting,
    /// The service is being replaced.
    Updating,
    /// The service went away without a goodbye and the shell is bringing it back.
    Restarting,
    /// The service is gone and the shell has stopped trying to start it.
    Stopped,
}

/// A button of a state screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateButton {
    /// Start the service.
    Start,
    /// Show the folder of diagnostics.
    Diagnostics,
}

impl StateButton {
    /// What the button says.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Start => START_LABEL,
            Self::Diagnostics => DIAGNOSTICS_LABEL,
        }
    }

    /// The fixed address the button points at.
    pub const fn address(self) -> &'static str {
        match self {
            Self::Start => START_ADDRESS,
            Self::Diagnostics => DIAGNOSTICS_ADDRESS,
        }
    }
}

impl StateScreen {
    /// The state's words. The tray's tooltip says the same.
    pub const fn words(self) -> &'static str {
        match self {
            Self::Starting => STARTING_WORDS,
            Self::Updating => UPDATING_WORDS,
            Self::Restarting => RESTARTING_WORDS,
            Self::Stopped => STOPPED_WORDS,
        }
    }

    /// The buttons the state offers, in order.
    pub const fn buttons(self) -> &'static [StateButton] {
        match self {
            Self::Stopped => &[StateButton::Start, StateButton::Diagnostics],
            Self::Starting | Self::Updating | Self::Restarting => &[],
        }
    }

    /// The class of the document's body: a state that is working breathes, a stopped one does
    /// not.
    const fn class(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting | Self::Updating | Self::Restarting => "working",
        }
    }
}

/// What the webview does with a navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
    /// The document itself loads.
    Allow,
    /// Anything else is refused.
    Refuse,
    /// A button was pressed: the navigation is refused and the shell acts on the press.
    Press(StateButton),
}

/// The rule for every navigation of a state screen: the document loaded from a string may load, the
/// two button addresses are presses, and nothing else goes anywhere.
pub fn navigation(url: &str) -> Navigation {
    [StateButton::Start, StateButton::Diagnostics]
        .into_iter()
        .find(|button| button.address() == url)
        .map(Navigation::Press)
        .unwrap_or_else(|| {
            if url == BLANK_ADDRESS || url.starts_with(DOCUMENT_ADDRESS_PREFIX) {
                Navigation::Allow
            } else {
                Navigation::Refuse
            }
        })
}

/// The document for `screen`, ready to load from a string.
pub fn document(screen: StateScreen) -> String {
    let buttons: String = screen
        .buttons()
        .iter()
        .map(|button| {
            format!(
                "<a class=\"button\" href=\"{}\">{}</a>",
                button.address(),
                escape(button.label())
            )
        })
        .collect();
    DOCUMENT
        .replace("{ground}", &ground_color())
        .replace("{class}", screen.class())
        .replace("{words}", &escape(screen.words()))
        .replace("{buttons}", &buttons)
}

/// The window ground as a CSS colour.
fn ground_color() -> String {
    let (red, green, blue, _) = GROUND;
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// `text` safe to put in the document as text or as a quoted attribute.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENS: [StateScreen; 4] = [
        StateScreen::Starting,
        StateScreen::Updating,
        StateScreen::Restarting,
        StateScreen::Stopped,
    ];

    #[test]
    fn each_state_says_its_own_words() {
        assert_eq!(StateScreen::Starting.words(), "Starting Ghostlight...");
        assert_eq!(StateScreen::Updating.words(), "Updating Ghostlight...");
        assert_eq!(
            StateScreen::Restarting.words(),
            "Ghostlight is restarting..."
        );
        assert_eq!(
            StateScreen::Stopped.words(),
            "Ghostlight stopped unexpectedly."
        );
    }

    #[test]
    fn only_the_stopped_state_has_buttons_and_they_are_start_and_diagnostics() {
        for screen in SCREENS {
            let expected: &[StateButton] = if screen == StateScreen::Stopped {
                &[StateButton::Start, StateButton::Diagnostics]
            } else {
                &[]
            };
            assert_eq!(screen.buttons(), expected, "{screen:?}");
        }
        assert_eq!(StateButton::Start.label(), "Start");
        assert_eq!(StateButton::Diagnostics.label(), "Diagnostics");
    }

    #[test]
    fn a_document_holds_its_words_and_its_buttons_and_nothing_left_to_fill() {
        for screen in SCREENS {
            let document = document(screen);
            assert!(document.contains(screen.words()), "{screen:?}");
            for button in screen.buttons() {
                assert!(
                    document.contains(&format!(
                        "<a class=\"button\" href=\"{}\">{}</a>",
                        button.address(),
                        button.label()
                    )),
                    "{screen:?} {button:?}"
                );
            }
            if screen.buttons().is_empty() {
                assert!(!document.contains("<a "), "{screen:?}");
            }
            for placeholder in ["{ground}", "{class}", "{words}", "{buttons}"] {
                assert!(!document.contains(placeholder), "{screen:?} {placeholder}");
            }
        }
    }

    #[test]
    fn a_document_has_the_workbench_ground_and_no_script_and_loads_nothing() {
        let document = document(StateScreen::Starting);
        assert!(document.contains("--bg: #0f0e12;"));
        assert!(!document.contains("<script"));
        assert!(!document.contains("src="));
        assert!(!document.contains("@import"));
        assert!(document.contains("default-src 'none'"));
        assert!(document.is_ascii());
    }

    #[test]
    fn a_document_follows_reduced_motion_and_forced_colors() {
        let document = document(StateScreen::Stopped);
        assert!(document.contains("@media (prefers-reduced-motion: reduce)"));
        assert!(document.contains("@media (forced-colors: active)"));
        assert!(document.contains("ButtonText"));
        assert!(document.contains("Highlight"));
    }

    #[test]
    fn a_working_state_breathes_and_a_stopped_one_does_not() {
        assert!(document(StateScreen::Restarting).contains("<body class=\"working\">"));
        assert!(document(StateScreen::Stopped).contains("<body class=\"stopped\">"));
    }

    #[test]
    fn the_document_itself_loads_and_a_button_is_a_press() {
        assert_eq!(navigation("about:blank"), Navigation::Allow);
        assert_eq!(
            navigation("data:text/html;charset=utf-8,%3Chtml%3E"),
            Navigation::Allow
        );
        assert_eq!(
            navigation(START_ADDRESS),
            Navigation::Press(StateButton::Start)
        );
        assert_eq!(
            navigation(DIAGNOSTICS_ADDRESS),
            Navigation::Press(StateButton::Diagnostics)
        );
    }

    #[test]
    fn every_other_address_is_refused() {
        for url in [
            "",
            "http://127.0.0.1:41234/",
            "https://example.com/",
            "https://ghostlight.invalid/",
            "https://ghostlight.invalid/start/",
            "https://ghostlight.invalid/start?x=1",
            "https://ghostlight.invalid/quit",
            "file:///C:/Windows/System32/cmd.exe",
            "javascript:alert(1)",
            "about:srcdoc",
        ] {
            assert_eq!(navigation(url), Navigation::Refuse, "{url}");
        }
    }

    #[test]
    fn text_is_escaped_where_it_enters_the_document() {
        assert_eq!(
            escape("a<b>&\"c\""),
            "a&lt;b&gt;&amp;&quot;c&quot;".to_owned()
        );
    }
}
