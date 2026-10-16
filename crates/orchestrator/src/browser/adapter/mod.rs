//! The service's browser adapter component.
//!
//! One adapter exists per browser family. Everything that runs in a page, and the translation
//! from the service's work vocabulary to browser primitives, lives under the family module, so a
//! service update ships new page code without an extension update.
//!
//! A new browser family is a new module beside `chromium`; no family enum names them.
//! Chromium is the only family, and it speaks adapter protocol 4. The work vocabulary is
//! re-exported here, so the rest of the service names `crate::browser::adapter::BrowserCommand`
//! and never a protocol path.

pub mod chromium;
pub mod documents;
pub mod work;

pub use work::*;
