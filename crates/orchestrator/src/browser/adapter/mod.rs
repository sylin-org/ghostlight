//! The service's browser adapter component.
//!
//! One adapter exists per browser family. The extension's hello names its family and wire
//! protocol, and the service picks the matching [`Adapter`]: the wire level it speaks and the
//! page assets it ships. Everything that runs in a page, and the translation from the service's
//! `Work` to browser primitives, lives under the family module, so a service update ships new page
//! code without an extension update.
//!
//! Chromium is the only family. It speaks level 3, the published extension's protocol, and level
//! 4. The level-3 vocabulary is re-exported here, so the rest of the service names
//! `crate::browser::adapter::BrowserCommand` and never a level path.

use std::collections::HashMap;

use serde::Deserialize as _;
use serde_json::Value;

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

    /// The protocol major of this family's newest level that names its protocol in `field`.
    fn newest_in(self, field: &str) -> u16 {
        let levels = match self {
            Self::Chromium => chromium::LEVELS,
        };
        levels
            .iter()
            .filter(|level| level.hello_field() == field)
            .map(|level| level.major())
            .max()
            .unwrap_or_default()
    }

    /// The platform identity this family reports in browser discovery.
    #[must_use]
    pub const fn platform(self) -> BrowserPlatform {
        match self {
            Self::Chromium => BrowserPlatform::Chromium,
        }
    }
}

/// The hello field in which a level-3 extension names its protocol.
const MAJOR_FIELD: &str = "major";
/// The hello field in which a level-4 extension names its protocol.
const PROTOCOL_FIELD: &str = "protocol";
/// The hello field naming the extension's platform.
const PLATFORM_FIELD: &str = "platform";
/// The frame field naming a frame's kind.
const KIND_FIELD: &str = "kind";
/// The kind of the first frame on every browser connection.
const HELLO_KIND: &str = "hello";

/// Wire level of a family: the adapter protocol revision its extension speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    /// Adapter protocol 3, spoken by the published Chromium extension.
    Three,
    /// Adapter protocol 4: frozen primitives, with page assets shipped in `hello_accepted`.
    Four,
}

impl Level {
    /// The adapter protocol major this level speaks.
    #[must_use]
    pub const fn major(self) -> u16 {
        match self {
            Self::Three => ADAPTER_PROTOCOL_MAJOR,
            Self::Four => chromium::level4::wire::PROTOCOL,
        }
    }

    /// The hello field in which an extension speaking this level names its protocol.
    const fn hello_field(self) -> &'static str {
        match self {
            Self::Three => MAJOR_FIELD,
            Self::Four => PROTOCOL_FIELD,
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

/// Why a first frame chose no adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelloRefusal {
    /// The frame is not a hello this service can read.
    Malformed,
    /// The hello names a protocol no level of its family speaks.
    Unsupported(UnsupportedHello),
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

    /// Choose the adapter for a first frame read as JSON, before it is decoded for its level.
    ///
    /// Each level names its protocol in its own hello field: level 3 in `major`, level 4 in
    /// `protocol`. A hello that names an unsupported protocol in a field is told the newest
    /// protocol its family speaks in that field, so a published level-3 extension still hears the
    /// major it can act on.
    ///
    /// # Errors
    ///
    /// Returns [`HelloRefusal::Malformed`] when the frame is not a readable hello, and
    /// [`HelloRefusal::Unsupported`] when no level of its family speaks the named protocol.
    pub fn select(hello: &Value) -> Result<Self, HelloRefusal> {
        if hello.get(KIND_FIELD).and_then(Value::as_str) != Some(HELLO_KIND) {
            return Err(HelloRefusal::Malformed);
        }
        let platform = match hello.get(PLATFORM_FIELD) {
            None | Some(Value::Null) => None,
            Some(platform) => {
                Some(BrowserPlatform::deserialize(platform).map_err(|_| HelloRefusal::Malformed)?)
            }
        };
        let family = Family::from_platform(platform);
        for field in [PROTOCOL_FIELD, MAJOR_FIELD] {
            let Some(offered) = hello.get(field) else {
                continue;
            };
            let offered = offered
                .as_u64()
                .and_then(|offered| u16::try_from(offered).ok())
                .ok_or(HelloRefusal::Malformed)?;
            return match Self::negotiate(platform, offered) {
                Ok(adapter) if adapter.level.hello_field() == field => Ok(adapter),
                _ => Err(HelloRefusal::Unsupported(UnsupportedHello {
                    offered,
                    required: family.newest_in(field),
                })),
            };
        }
        Err(HelloRefusal::Malformed)
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

    /// The work that installs this adapter's page assets before it takes work, given the
    /// capabilities it advertised, or `None` when nothing must be installed first.
    ///
    /// Level 3 installs its bundle with a correlated command. Level 4 ships its assets in
    /// `hello_accepted`, so it never needs one.
    #[must_use]
    pub fn install_command(self, capabilities: &HashMap<String, u16>) -> Option<Work> {
        match (self.family, self.level) {
            (Family::Chromium, Level::Three) => {
                chromium::assets::installs(capabilities).then(chromium::assets::install_command)
            }
            (Family::Chromium, Level::Four) => None,
        }
    }

    /// Whether an outcome acknowledges the exact assets [`Self::install_command`] sent.
    #[must_use]
    pub fn acknowledged(self, outcome: &BrowserOutcome) -> bool {
        match (self.family, self.level) {
            (Family::Chromium, Level::Three) => chromium::assets::acknowledged(outcome),
            (Family::Chromium, Level::Four) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    fn a_protocol_major_without_a_level_is_refused_with_the_newest_major() {
        for major in [
            0,
            ADAPTER_PROTOCOL_MAJOR - 1,
            chromium::level4::wire::PROTOCOL + 1,
        ] {
            assert_eq!(
                Adapter::negotiate(None, major),
                Err(UnsupportedHello {
                    offered: major,
                    required: chromium::level4::wire::PROTOCOL
                })
            );
        }
    }

    #[test]
    fn each_level_is_selected_by_its_own_hello_field() {
        let three = Adapter::select(&json!({"kind": "hello", "major": 3})).expect("level 3");
        assert_eq!(three.level(), Level::Three);
        let four = Adapter::select(&json!({
            "kind": "hello",
            "protocol": 4,
            "platform": "ghostlight/chromium"
        }))
        .expect("level 4");
        assert_eq!(four.level(), Level::Four);
        assert_eq!(four.platform(), BrowserPlatform::Chromium);
    }

    #[test]
    fn a_hello_naming_a_protocol_in_another_level_field_is_unsupported() {
        for (hello, offered, required) in [
            (json!({"kind": "hello", "major": 4}), 4, 3),
            (json!({"kind": "hello", "major": 2}), 2, 3),
            (json!({"kind": "hello", "protocol": 3}), 3, 4),
            (json!({"kind": "hello", "protocol": 5}), 5, 4),
        ] {
            assert_eq!(
                Adapter::select(&hello),
                Err(HelloRefusal::Unsupported(UnsupportedHello {
                    offered,
                    required
                })),
                "{hello}"
            );
        }
    }

    #[test]
    fn a_frame_that_is_not_a_readable_hello_is_malformed() {
        for frame in [
            json!({"kind": "receipt", "protocol": 4}),
            json!({"kind": "hello"}),
            json!({"kind": "hello", "protocol": "4"}),
            json!({"kind": "hello", "protocol": 70000}),
            json!({"kind": "hello", "protocol": 4, "platform": "ghostlight/gecko"}),
            json!([]),
        ] {
            assert_eq!(
                Adapter::select(&frame),
                Err(HelloRefusal::Malformed),
                "{frame}"
            );
        }
    }

    #[test]
    fn assets_install_only_for_a_level_three_adapter_that_advertises_the_page_runtime() {
        let adapter = Adapter::negotiate(None, ADAPTER_PROTOCOL_MAJOR).expect("level 3");
        let mut capabilities = HashMap::new();
        assert!(adapter.install_command(&capabilities).is_none());
        capabilities.insert(adapter_capability::PAGE_RUNTIME.to_string(), 1);
        assert!(adapter.install_command(&capabilities).is_some());
        let four = Adapter::negotiate(None, chromium::level4::wire::PROTOCOL).expect("level 4");
        assert!(four.install_command(&capabilities).is_none());
    }

    #[test]
    fn the_install_command_is_acknowledged_by_its_own_receipt_only() {
        let adapter = Adapter::negotiate(None, ADAPTER_PROTOCOL_MAJOR).expect("level 3");
        let capabilities = HashMap::from([(adapter_capability::PAGE_RUNTIME.to_string(), 1)]);
        let Some(Work::InstallPageRuntime {
            revision, sha256, ..
        }) = adapter.install_command(&capabilities)
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
