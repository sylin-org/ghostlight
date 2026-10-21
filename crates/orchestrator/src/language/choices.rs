//! The closed choices a call may name, each decoded once into a type (ADR-0203 Decision 14).
//!
//! Every vocabulary here is advertised by the catalog in the order its words are listed, decoded
//! with the same refusal words the decoder has always used ("`field` must be one of ...: got
//! `value`"), and matched by the executor as a type, so a new word fails to compile wherever it
//! is not handled.

use serde::de::{Deserialize, Deserializer, Error as _};

/// Define one closed vocabulary: the enum, its words in catalog order, and its decoding.
macro_rules! closed_choice {
    (
        $(#[$meta:meta])*
        $name:ident ($field:literal) {
            $($(#[$variant_meta:meta])* $variant:ident => $word:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            /// Every word, in the order the catalog advertises them.
            pub const WORDS: &'static [&'static str] = &[$($word),+];

            /// The word a call names this choice by.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $word,)+
                }
            }

            /// The choice a call names, or the refusal that lists the words.
            pub(crate) fn named(name: &str) -> Result<Self, String> {
                match name {
                    $($word => Ok(Self::$variant),)+
                    _ => Err(format!(
                        "{} must be one of {}: got `{name}`",
                        $field,
                        Self::WORDS.join(", ")
                    )),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let name = String::deserialize(deserializer)?;
                Self::named(&name).map_err(D::Error::custom)
            }
        }
    };
}

closed_choice! {
    /// What an inspection lists.
    InspectScope("scope") {
        /// The controls an agent can act on.
        Controls => "controls",
        /// The page's structural landmarks and headings.
        Structure => "structure",
        /// Controls and structure together.
        All => "all",
        /// One bounded document tree.
        Document => "document",
    }
}

closed_choice! {
    /// What a find searches.
    FindScope("scope") {
        /// Controls and text alike.
        Any => "any",
        /// Controls only.
        Control => "control",
        /// Text only.
        Text => "text",
    }
}

closed_choice! {
    /// Which pointer button a click presses.
    PointerButton("button") {
        /// The primary button.
        Primary => "primary",
        /// The middle button.
        Middle => "middle",
        /// The secondary button.
        Secondary => "secondary",
    }
}

closed_choice! {
    /// Which way a scroll goes.
    ScrollDirection("direction") {
        /// Up.
        Up => "up",
        /// Down.
        Down => "down",
        /// Left.
        Left => "left",
        /// Right.
        Right => "right",
    }
}

closed_choice! {
    /// How far a page scroll goes.
    ScrollAmount("amount") {
        /// A small step.
        Small => "small",
        /// A medium step.
        Medium => "medium",
        /// A large step.
        Large => "large",
        /// One viewport.
        Page => "page",
    }
}

closed_choice! {
    /// Which way a history step goes.
    HistoryDirection("action") {
        /// To the previous page.
        Back => "back",
        /// To the next page.
        Forward => "forward",
    }
}

closed_choice! {
    /// Which browser diagnostics a read returns.
    DiagnosticSource("source") {
        /// Console and network.
        Both => "both",
        /// The console only.
        Console => "console",
        /// The network only.
        Network => "network",
    }
}

closed_choice! {
    /// How much a diagnostics read returns.
    DiagnosticDetail("detail") {
        /// Warnings, errors, failed requests and HTTP failures.
        Problems => "problems",
        /// Every entry.
        All => "all",
    }
}

closed_choice! {
    /// What a flow does at a failed step.
    OnError("on_error") {
        /// Stop the flow.
        Stop => "stop",
        /// Run the remaining steps.
        Continue => "continue",
    }
}

closed_choice! {
    /// What a dialog call does.
    DialogAction("action") {
        /// Read the open dialog.
        Status => "status",
        /// Accept it, as OK does.
        Accept => "accept",
        /// Dismiss it.
        Dismiss => "dismiss",
        /// Answer a prompt with text.
        Respond => "respond",
    }
}

closed_choice! {
    /// What a recording call does.
    RecordAction("action") {
        /// Start recording a tab.
        Start => "start",
        /// Read the recording's state.
        Status => "status",
        /// Stop and keep the frames.
        Stop => "stop",
        /// Save the recording.
        Save => "save",
        /// Discard it.
        Discard => "discard",
    }
}
