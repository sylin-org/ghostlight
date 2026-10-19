//! What Ghostlight says when the page refuses a control before acting on it (ADR-0198, amendment
//! of 2026-10-18).
//!
//! A page asset checks a control before it clicks, types into, fills or moves to it, and refuses
//! with a closed code when the control is hidden, disabled, read-only or does not take text. Such
//! a refusal comes before any effect. This module owns that closed vocabulary, the gesture the
//! call would have made, and the one place that turns the pair into a sentence that says nothing
//! happened and a step that would work. The browser layer decodes the page's code into
//! [`PageRefusal`]; the page's own message never reaches the model.

use serde::{Deserialize, Serialize};

use super::Operation;

/// Why the page refused a control before acting on it.
///
/// The words are the page asset's own codes. They are the `reason` fact of the refusal and the
/// `cause` its audit keeps, so a word is not renamed or removed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageRefusal {
    /// The control is not visible: it is hidden, transparent or takes no space on the page.
    NotVisible,
    /// The control is disabled or inside an inert part of the page.
    Disabled,
    /// The field is read-only.
    ReadOnly,
    /// The control does not take typed text.
    NotEditable,
}

impl PageRefusal {
    /// Every page refusal, in declaration order.
    pub const ALL: [Self; 4] = [
        Self::NotVisible,
        Self::Disabled,
        Self::ReadOnly,
        Self::NotEditable,
    ];

    /// The word the page, the facts and the audit use for this refusal.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotVisible => "not_visible",
            Self::Disabled => "disabled",
            Self::ReadOnly => "read_only",
            Self::NotEditable => "not_editable",
        }
    }

    /// What was refused, in plain words.
    const fn clause(self) -> &'static str {
        match self {
            Self::NotVisible => "That target is not visible on the page.",
            Self::Disabled => "That target is disabled.",
            Self::ReadOnly => "That field is read-only.",
            Self::NotEditable => "That target does not take typed text.",
        }
    }

    /// The one step that would work instead. Every tool and field it names is one the catalog
    /// takes.
    const fn step(self) -> &'static str {
        match self {
            Self::NotVisible => {
                "Call browser_find or browser_inspect and use a control that is visible; one in a closed menu or a collapsed section shows after a click opens it."
            }
            Self::Disabled => {
                "Do what the page still needs before it enables this control (browser_inspect shows each control and its state), then repeat the call."
            }
            Self::ReadOnly => {
                "This field does not take input. Call browser_find with scope control to find a field that does."
            }
            Self::NotEditable => {
                "Call browser_find with scope control for the text field itself, then repeat with its target."
            }
        }
    }
}

/// What a call does to the page, so a refusal can say that it did not happen and a browser
/// failure can say what the browser could not do.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageGesture {
    /// A click, a press or a submit.
    Click,
    /// Typing into a field.
    Type,
    /// Filling form fields.
    Fill,
    /// Moving the pointer over a control.
    Hover,
    /// Dragging from one place to another.
    Drag,
    /// Scrolling a control into view.
    Scroll,
    /// Pressing a key in a control.
    Key,
    /// Attaching a file to a file input.
    Upload,
    /// Capturing a control.
    Capture,
    /// Any other call that checks a control first.
    Other,
}

impl PageGesture {
    /// The gesture a decoded operation makes. A flow names none of its own: each step is
    /// refused with its own gesture.
    #[must_use]
    pub const fn of(operation: &Operation) -> Self {
        match operation {
            Operation::Click(_) => Self::Click,
            Operation::TypeText(_) => Self::Type,
            Operation::FillForm(_) => Self::Fill,
            Operation::Hover(_) => Self::Hover,
            Operation::Drag(_) => Self::Drag,
            Operation::ScrollPage(_) => Self::Scroll,
            Operation::PressKey(_) => Self::Key,
            Operation::UploadFiles(_) => Self::Upload,
            Operation::TakeScreenshot(_) => Self::Capture,
            _ => Self::Other,
        }
    }

    /// The sentence that says the gesture did not happen. It is true for every page refusal,
    /// because the page refuses before its first change.
    const fn nothing_done(self) -> &'static str {
        match self {
            Self::Click => "Nothing was clicked.",
            Self::Type => "Nothing was typed.",
            Self::Fill => "No field was filled.",
            Self::Hover => "The pointer was not moved.",
            Self::Drag => "Nothing was dragged.",
            Self::Scroll => "Nothing was scrolled.",
            Self::Key => "No key was pressed.",
            Self::Upload => "No file was attached.",
            Self::Capture => "Nothing was captured.",
            Self::Other => "Nothing was done on the page.",
        }
    }

    /// The sentence history keeps when the browser could not make the gesture. It names the kind
    /// of act only, never the page's text or the browser's error.
    #[must_use]
    pub const fn could_not(self) -> &'static str {
        match self {
            Self::Click => "The browser could not click that target.",
            Self::Type => "The browser could not type into that field.",
            Self::Fill => "The browser could not fill those fields.",
            Self::Hover => "The browser could not move the pointer there.",
            Self::Drag => "The browser could not drag that.",
            Self::Scroll => "The browser could not scroll there.",
            Self::Key => "The browser could not press that key.",
            Self::Upload => "The browser could not attach that file.",
            Self::Capture => "The browser could not capture that.",
            Self::Other => "The browser could not complete this operation.",
        }
    }
}

/// The summary of a page refusal: what was refused, then that nothing happened.
#[must_use]
pub fn summary(refusal: PageRefusal, gesture: PageGesture) -> String {
    format!("{} {}", refusal.clause(), gesture.nothing_done())
}

/// The one step a page refusal offers.
#[must_use]
pub fn next_steps(refusal: PageRefusal) -> Vec<String> {
    vec![refusal.step().into()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_refusal_word_round_trips_as_the_page_spells_it() {
        for refusal in PageRefusal::ALL {
            let word = serde_json::to_value(refusal).unwrap();
            assert_eq!(word, refusal.as_str());
            assert_eq!(
                serde_json::from_value::<PageRefusal>(word).unwrap(),
                refusal
            );
        }
    }

    #[test]
    fn a_hidden_target_says_nothing_was_clicked_and_where_a_visible_one_is() {
        assert_eq!(
            summary(PageRefusal::NotVisible, PageGesture::Click),
            "That target is not visible on the page. Nothing was clicked."
        );
        let steps = next_steps(PageRefusal::NotVisible);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].contains("browser_find or browser_inspect"));
    }

    #[test]
    fn a_disabled_target_points_at_what_the_page_still_needs() {
        assert_eq!(
            summary(PageRefusal::Disabled, PageGesture::Click),
            "That target is disabled. Nothing was clicked."
        );
        assert!(next_steps(PageRefusal::Disabled)[0].contains("browser_inspect"));
    }

    #[test]
    fn a_read_only_field_says_nothing_was_typed_and_finds_one_that_takes_input() {
        assert_eq!(
            summary(PageRefusal::ReadOnly, PageGesture::Type),
            "That field is read-only. Nothing was typed."
        );
        assert_eq!(
            summary(PageRefusal::ReadOnly, PageGesture::Fill),
            "That field is read-only. No field was filled."
        );
        assert!(next_steps(PageRefusal::ReadOnly)[0].contains("browser_find with scope control"));
    }

    #[test]
    fn a_target_that_takes_no_text_finds_the_text_field() {
        assert_eq!(
            summary(PageRefusal::NotEditable, PageGesture::Type),
            "That target does not take typed text. Nothing was typed."
        );
        assert!(next_steps(PageRefusal::NotEditable)[0].contains("browser_find with scope control"));
    }

    #[test]
    fn no_sentence_carries_the_page_internal_words() {
        let gestures = [
            PageGesture::Click,
            PageGesture::Type,
            PageGesture::Fill,
            PageGesture::Hover,
            PageGesture::Drag,
            PageGesture::Scroll,
            PageGesture::Key,
            PageGesture::Upload,
            PageGesture::Capture,
            PageGesture::Other,
        ];
        for refusal in PageRefusal::ALL {
            for gesture in gestures {
                let text = format!(
                    "{} {}",
                    summary(refusal, gesture),
                    next_steps(refusal).join(" ")
                );
                assert!(text.is_ascii(), "{text}");
                let words = text.replace("browser_", "");
                for internal in ["activate", " box", "zero-size", "refused this job", "_"] {
                    assert!(!words.contains(internal), "{internal}: {text}");
                }
            }
        }
    }
}
