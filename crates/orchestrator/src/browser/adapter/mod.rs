//! The service's browser adapter component.
//!
//! One adapter exists per browser family. Everything that runs in a page, and the translation
//! from the service's work vocabulary to browser primitives, lives under the family module, so a
//! service update ships new page code without an extension update.
//!
//! Chromium is the only family, and it speaks adapter protocol 4. The work vocabulary is
//! re-exported here, so the rest of the service names `crate::browser::adapter::BrowserCommand`
//! and never a protocol path.

pub mod chromium;
pub mod documents;
pub mod work;

pub use work::*;

/// Browser engine family an adapter serves. A closed set: a new family is a new module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    /// Chromium-family browsers (Chrome, Edge, Brave, and others).
    Chromium,
}

impl Family {
    /// The family a hello's platform field names, or `None` for a platform no adapter serves.
    /// A hello without a platform comes from a Chromium extension.
    #[must_use]
    pub fn from_platform(platform: Option<&str>) -> Option<Self> {
        match platform {
            None => Some(Self::Chromium),
            Some(platform) if platform == chromium::protocol4::wire::PLATFORM => {
                Some(Self::Chromium)
            }
            Some(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hello_names_chromium_or_no_family() {
        assert_eq!(Family::from_platform(None), Some(Family::Chromium));
        assert_eq!(
            Family::from_platform(Some("ghostlight/chromium")),
            Some(Family::Chromium)
        );
        assert_eq!(Family::from_platform(Some("ghostlight/gecko")), None);
    }
}
