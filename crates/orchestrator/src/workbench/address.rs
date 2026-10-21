//! The places the Ghostlight page can be opened at (ADR-0199, amendment of 2026-10-15).
//!
//! Every tab of the page, every section of its Settings tab, and its setup view has an address:
//! the text after the `#` of the page's own address, such as `settings/diagnostics`. The page keeps the list in
//! `ui/lib/words.js` (`ADDRESSES`); [`PageAddress`] is the same list for the service, so a request
//! to show the window at a section is checked here before the shell is told, and the shell never
//! decides what an address means. A test in `surface_tests.rs` reads the page's list and proves
//! the two are equal.

use std::fmt;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// One place the page can be opened at.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PageAddress {
    /// The At a glance tab.
    Monitor,
    /// The Integrations tab.
    Integrations,
    /// The Status tab.
    Status,
    /// The Policy tab.
    Policy,
    /// The Settings tab, at the section it showed last.
    Settings,
    /// Settings, This screen.
    SettingsScreen,
    /// Settings, Sharing.
    SettingsSharing,
    /// Settings, Browser work.
    SettingsBrowserWork,
    /// Settings, Notifications.
    SettingsNotifications,
    /// Settings, Diagnostics.
    SettingsDiagnostics,
    /// The About tab.
    About,
    /// The setup view, which stands in for the tabs while setup is under way.
    Setup,
}

impl PageAddress {
    /// Every address, in the order the page lists them.
    pub const ALL: [Self; 12] = [
        Self::Monitor,
        Self::Integrations,
        Self::Status,
        Self::Policy,
        Self::Settings,
        Self::SettingsScreen,
        Self::SettingsSharing,
        Self::SettingsBrowserWork,
        Self::SettingsNotifications,
        Self::SettingsDiagnostics,
        Self::About,
        Self::Setup,
    ];

    /// The address as the page writes it after the `#`, and as the wire carries it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Monitor => "monitor",
            Self::Integrations => "integrations",
            Self::Status => "status",
            Self::Policy => "policy",
            Self::Settings => "settings",
            Self::SettingsScreen => "settings/screen",
            Self::SettingsSharing => "settings/sharing",
            Self::SettingsBrowserWork => "settings/browser-work",
            Self::SettingsNotifications => "settings/notifications",
            Self::SettingsDiagnostics => "settings/diagnostics",
            Self::About => "about",
            Self::Setup => "setup",
        }
    }

    /// The address named `text` (without a leading `#`), or `None` for text that is not one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|address| address.as_str() == text)
    }
}

impl fmt::Display for PageAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for PageAddress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PageAddress {
    /// Reads an address; text that is not one fails, so a request naming it is a bad request.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| de::Error::custom("not an address of the page"))
    }
}
