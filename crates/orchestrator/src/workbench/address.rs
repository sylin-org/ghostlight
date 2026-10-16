//! The places the Ghostlight page can be opened at (ADR-0199, amendment of 2026-10-15).
//!
//! Every tab of the page, every section of its Settings tab, and its setup view has an address:
//! the text after the `#` of the page's own address, such as `settings/diagnostics`. The page keeps the list in
//! `ui/lib/words.js` (`ADDRESSES`); [`PageAddress`] is the same list for the service, so a request
//! to show the window at a section is checked here before the shell is told, and the shell never
//! decides what an address means. A test reads the page's list and proves the two are equal.

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The page's list of addresses, as the page writes it.
    const PAGE_WORDS: &str = include_str!("../../ui/lib/words.js");

    /// The line where the page's list starts.
    const LIST_START: &str = "const ADDRESSES = Object.freeze([";

    /// The page's addresses, read from its source in order.
    fn page_addresses() -> Vec<String> {
        let start = PAGE_WORDS
            .find(LIST_START)
            .expect("the page lists its addresses")
            + LIST_START.len();
        let list = &PAGE_WORDS[start..];
        let list = &list[..list.find("])").expect("the list ends")];
        list.split(',')
            .map(|entry| entry.trim().trim_matches('"').to_owned())
            .filter(|entry| !entry.is_empty())
            .collect()
    }

    #[test]
    fn the_service_lists_the_addresses_the_page_lists() {
        let ours: Vec<String> = PageAddress::ALL
            .iter()
            .map(|address| address.as_str().to_owned())
            .collect();
        assert_eq!(ours, page_addresses());
    }

    #[test]
    fn an_address_is_its_wire_text() {
        for address in PageAddress::ALL {
            assert_eq!(PageAddress::parse(address.as_str()), Some(address));
            assert_eq!(
                serde_json::to_value(address).unwrap(),
                serde_json::Value::from(address.as_str())
            );
            assert_eq!(
                serde_json::from_value::<PageAddress>(address.as_str().into()).unwrap(),
                address
            );
            assert_eq!(address.to_string(), address.as_str());
        }
    }

    #[test]
    fn text_that_is_not_an_address_is_not_one() {
        for text in [
            "",
            "#monitor",
            "Monitor",
            "settings/",
            "settings/nowhere",
            "settings/diagnostics/x",
            "nowhere",
            "monitor\"; alert(1); \"",
        ] {
            assert_eq!(PageAddress::parse(text), None, "{text}");
            assert!(serde_json::from_value::<PageAddress>(text.into()).is_err());
        }
    }

    #[test]
    fn every_address_is_safe_to_put_in_a_script() {
        for address in PageAddress::ALL {
            assert!(address.as_str().len() <= 48);
            assert!(address
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || matches!(byte, b'/' | b'-')));
        }
    }
}
