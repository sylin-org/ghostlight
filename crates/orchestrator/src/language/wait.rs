//! The closed wait vocabulary: the conditions `browser_wait` and an action's `expect` name, the
//! subset the page evaluates, and the rules each condition carries (ADR-0203 Decision 8).
//!
//! The decoder turns a condition's name into a [`WaitCondition`] once, with the curated refusal
//! words, and everything after it matches the type. The page receives [`PageCondition::as_str`],
//! so the words on the wire are the catalog's.

use serde::de::{Deserialize, Deserializer, Error as _};

/// The condition `layout_stable` became `visual_settle`; a call that still names it is told so.
const RETIRED_LAYOUT_STABLE: &str = "layout_stable";

/// The refusal for an `expect` that names a condition it does not take.
pub(super) const EXPECT_REFUSAL: &str =
    "expect supports load_ready, url_contains, text_present, or text_absent";

/// One condition a wait can wait for.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WaitCondition {
    /// The top document finished loading.
    LoadReady,
    /// The top document's address contains the value.
    UrlContains,
    /// The value is in the visible text.
    TextPresent,
    /// The value is gone from the visible text.
    TextAbsent,
    /// The target exists.
    TargetPresent,
    /// The target is gone.
    TargetAbsent,
    /// The page, or the target, stopped moving and repainting.
    VisualSettle,
    /// A semantic selector matches a control.
    SelectorPresent,
    /// A number of milliseconds passed.
    Duration,
}

impl WaitCondition {
    /// Every condition, in the catalog's order.
    pub const ALL: [Self; 9] = [
        Self::LoadReady,
        Self::UrlContains,
        Self::TextPresent,
        Self::TextAbsent,
        Self::TargetPresent,
        Self::TargetAbsent,
        Self::VisualSettle,
        Self::SelectorPresent,
        Self::Duration,
    ];

    /// The condition's name, as the catalog advertises it and the page receives it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadReady => "load_ready",
            Self::UrlContains => "url_contains",
            Self::TextPresent => "text_present",
            Self::TextAbsent => "text_absent",
            Self::TargetPresent => "target_present",
            Self::TargetAbsent => "target_absent",
            Self::VisualSettle => "visual_settle",
            Self::SelectorPresent => "selector_present",
            Self::Duration => "duration",
        }
    }

    /// Whether a satisfied wait also settles the layout when the call does not say: on for
    /// readiness, a target appearing, a selector match and a duration; off for text, the address
    /// and a target leaving.
    #[must_use]
    pub const fn settles_by_default(self) -> bool {
        matches!(
            self,
            Self::LoadReady | Self::TargetPresent | Self::SelectorPresent | Self::Duration
        )
    }

    /// Whether this wait settles the layout once its condition holds, given the call's
    /// `visual_settle`. A visual-settle wait is the settling itself, so it never settles again.
    #[must_use]
    pub fn settles(self, requested: Option<bool>) -> bool {
        match self {
            Self::VisualSettle => false,
            _ if self.settles_by_default() => requested != Some(false),
            _ => requested == Some(true),
        }
    }

    /// The condition the page evaluates, or none for the two the service answers itself: a
    /// selector match is polled through the semantic query, and a duration is a sleep.
    #[must_use]
    pub const fn page(self) -> Option<PageCondition> {
        Some(match self {
            Self::LoadReady => PageCondition::LoadReady,
            Self::UrlContains => PageCondition::UrlContains,
            Self::TextPresent => PageCondition::TextPresent,
            Self::TextAbsent => PageCondition::TextAbsent,
            Self::TargetPresent => PageCondition::TargetPresent,
            Self::TargetAbsent => PageCondition::TargetAbsent,
            Self::VisualSettle => PageCondition::VisualSettle,
            Self::SelectorPresent | Self::Duration => return None,
        })
    }

    /// The condition a call names, or the refusal that says which names exist.
    pub(super) fn named(name: &str) -> Result<Self, String> {
        if name == RETIRED_LAYOUT_STABLE {
            return Err(
                "condition layout_stable was merged into visual_settle; use condition visual_settle"
                    .into(),
            );
        }
        Self::ALL
            .into_iter()
            .find(|condition| condition.as_str() == name)
            .ok_or_else(|| {
                // The refusal has never listed selector_present, which a wait names together
                // with its selector.
                let names: Vec<&str> = Self::ALL
                    .into_iter()
                    .filter(|condition| *condition != Self::SelectorPresent)
                    .map(Self::as_str)
                    .collect();
                format!(
                    "condition must be one of {}: got `{name}`",
                    names.join(", ")
                )
            })
    }
}

/// A condition the page evaluates: what `BrowserCommand::Observe` carries.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageCondition {
    /// The top document finished loading.
    LoadReady,
    /// The top document's address contains the value.
    UrlContains,
    /// Some readable document shows the value.
    TextPresent,
    /// No readable document shows the value.
    TextAbsent,
    /// The locator's target is in the page.
    TargetPresent,
    /// The locator's target is gone.
    TargetAbsent,
    /// The page, or the locator's target, stopped moving and repainting.
    VisualSettle,
}

impl PageCondition {
    /// The conditions an action's `expect` may name, in the catalog's order.
    pub const EXPECTABLE: [Self; 4] = [
        Self::LoadReady,
        Self::UrlContains,
        Self::TextPresent,
        Self::TextAbsent,
    ];

    /// The condition's name, as the page receives it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadReady => WaitCondition::LoadReady.as_str(),
            Self::UrlContains => WaitCondition::UrlContains.as_str(),
            Self::TextPresent => WaitCondition::TextPresent.as_str(),
            Self::TextAbsent => WaitCondition::TextAbsent.as_str(),
            Self::TargetPresent => WaitCondition::TargetPresent.as_str(),
            Self::TargetAbsent => WaitCondition::TargetAbsent.as_str(),
            Self::VisualSettle => WaitCondition::VisualSettle.as_str(),
        }
    }

    /// Whether the condition holds when something is gone rather than there.
    #[must_use]
    pub const fn tests_absence(self) -> bool {
        matches!(self, Self::TextAbsent | Self::TargetAbsent)
    }

    /// Whether the condition reads every readable document of the page rather than one.
    #[must_use]
    pub const fn reads_whole_page(self) -> bool {
        matches!(self, Self::TextPresent | Self::TextAbsent)
    }

    /// The condition an `expect` names, or the refusal that says which names it takes.
    fn expected(name: &str) -> Result<Self, String> {
        Self::EXPECTABLE
            .into_iter()
            .find(|condition| condition.as_str() == name)
            .ok_or_else(|| EXPECT_REFUSAL.into())
    }
}

/// Decode a wait's `condition` with the curated refusal words.
pub(super) fn decode_condition<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<WaitCondition, D::Error> {
    let name = String::deserialize(deserializer)?;
    WaitCondition::named(&name).map_err(D::Error::custom)
}

/// Decode an `expect`'s `condition` with the curated refusal words.
pub(super) fn decode_expected<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<PageCondition, D::Error> {
    let name = String::deserialize(deserializer)?;
    PageCondition::expected(&name).map_err(D::Error::custom)
}
