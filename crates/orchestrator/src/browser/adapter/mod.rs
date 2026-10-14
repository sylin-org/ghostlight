//! The service's browser adapter component.
//!
//! One adapter exists per browser family. The extension's hello names its family and wire
//! protocol, and the service picks the matching [`Adapter`]: the wire level it speaks and the
//! page assets it installs. Everything that runs in a page, and the translation from the
//! service's `Work` to browser primitives, lives under the family module, so a service update
//! ships new page code without an extension update.
//!
//! Chromium is the only family and level 3 is its only level. The vocabulary of that level is
//! re-exported here, so the rest of the service names `crate::browser::adapter::BrowserCommand`
//! and never a level path. A second level changes this file and the family module, not callers.

use std::collections::HashMap;

pub mod chromium;

pub use chromium::level3::*;

/// The service's internal browser work vocabulary.
///
/// The executor, workspace and presentation code describe browser work in this vocabulary and
/// the adapter turns it into frames. Level 3 carries it unchanged, so it is the level 3 command
/// set today and becomes its own type when a second level exists.
pub type Work = BrowserCommand;

/// Browser engine family an adapter serves. A closed set: a new family is a new module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    /// Chromium-family browsers (Chrome, Edge, Brave, and others).
    Chromium,
}

impl Family {
    /// Choose the family from a hello's platform field. An absent field means Chromium, the
    /// only family that predates the field.
    #[must_use]
    pub fn from_platform(platform: Option<BrowserPlatform>) -> Self {
        match platform.unwrap_or_default() {
            BrowserPlatform::Chromium => Self::Chromium,
        }
    }

    /// The platform identity this family reports in browser discovery.
    #[must_use]
    pub const fn platform(self) -> BrowserPlatform {
        match self {
            Self::Chromium => BrowserPlatform::Chromium,
        }
    }
}

/// Wire level of a family: the adapter protocol revision its extension speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    /// Adapter protocol 3, spoken by the published Chromium extension.
    Three,
}

impl Level {
    /// The adapter protocol major this level speaks.
    #[must_use]
    pub const fn major(self) -> u16 {
        match self {
            Self::Three => ADAPTER_PROTOCOL_MAJOR,
        }
    }
}

/// A hello that no adapter of the named family can serve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedHello {
    /// Protocol major the extension offered.
    pub offered: u16,
    /// Protocol major the family's newest level speaks.
    pub required: u16,
}

/// The family and wire level chosen for one browser connection at hello.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Adapter {
    family: Family,
    level: Level,
}

impl Adapter {
    /// Choose the adapter for a hello's platform and protocol major.
    ///
    /// # Errors
    ///
    /// Returns [`UnsupportedHello`] when the family has no level speaking `major`.
    pub fn negotiate(
        platform: Option<BrowserPlatform>,
        major: u16,
    ) -> Result<Self, UnsupportedHello> {
        let family = Family::from_platform(platform);
        let level = match family {
            Family::Chromium => chromium::level_for(major),
        };
        level
            .map(|level| Self { family, level })
            .ok_or(UnsupportedHello {
                offered: major,
                required: match family {
                    Family::Chromium => chromium::NEWEST_LEVEL.major(),
                },
            })
    }

    /// The browser engine family.
    #[must_use]
    pub const fn family(self) -> Family {
        self.family
    }

    /// The wire level this connection speaks.
    #[must_use]
    pub const fn level(self) -> Level {
        self.level
    }

    /// The platform identity reported in browser discovery.
    #[must_use]
    pub const fn platform(self) -> BrowserPlatform {
        self.family.platform()
    }

    /// Whether the adapter must receive this family's page assets before it takes work, given
    /// the capabilities it advertised.
    #[must_use]
    pub fn installs_assets(self, capabilities: &HashMap<String, u16>) -> bool {
        match (self.family, self.level) {
            (Family::Chromium, Level::Three) => chromium::assets::installs(capabilities),
        }
    }

    /// The work that installs this adapter's page assets.
    #[must_use]
    pub fn install_command(self) -> Work {
        match (self.family, self.level) {
            (Family::Chromium, Level::Three) => chromium::assets::install_command(),
        }
    }

    /// Whether an outcome acknowledges the exact assets [`Self::install_command`] sent.
    #[must_use]
    pub fn acknowledged(self, outcome: &BrowserOutcome) -> bool {
        match (self.family, self.level) {
            (Family::Chromium, Level::Three) => chromium::assets::acknowledged(outcome),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hello_without_a_platform_is_chromium_level_three() {
        let adapter = Adapter::negotiate(None, ADAPTER_PROTOCOL_MAJOR).expect("level 3");
        assert_eq!(adapter.family(), Family::Chromium);
        assert_eq!(adapter.platform(), BrowserPlatform::Chromium);
        assert_eq!(
            adapter,
            Adapter::negotiate(Some(BrowserPlatform::Chromium), 3).expect("level 3")
        );
    }

    #[test]
    fn a_protocol_major_without_a_level_is_refused_with_the_required_major() {
        for major in [0, ADAPTER_PROTOCOL_MAJOR - 1, ADAPTER_PROTOCOL_MAJOR + 1] {
            assert_eq!(
                Adapter::negotiate(None, major),
                Err(UnsupportedHello {
                    offered: major,
                    required: ADAPTER_PROTOCOL_MAJOR
                })
            );
        }
    }

    #[test]
    fn assets_install_only_for_an_adapter_that_advertises_the_page_runtime() {
        let adapter = Adapter::negotiate(None, ADAPTER_PROTOCOL_MAJOR).expect("level 3");
        let mut capabilities = HashMap::new();
        assert!(!adapter.installs_assets(&capabilities));
        capabilities.insert(adapter_capability::PAGE_RUNTIME.to_string(), 1);
        assert!(adapter.installs_assets(&capabilities));
    }

    #[test]
    fn the_install_command_is_acknowledged_by_its_own_receipt_only() {
        let adapter = Adapter::negotiate(None, ADAPTER_PROTOCOL_MAJOR).expect("level 3");
        let Work::InstallPageRuntime {
            revision, sha256, ..
        } = adapter.install_command()
        else {
            panic!("level 3 installs through the page-runtime command");
        };
        assert!(adapter.acknowledged(&BrowserOutcome::PageRuntimeInstalled {
            revision,
            sha256: sha256.clone(),
        }));
        assert!(
            !adapter.acknowledged(&BrowserOutcome::PageRuntimeInstalled {
                revision,
                sha256: "0".repeat(64),
            })
        );
    }
}
