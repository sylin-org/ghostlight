// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight -- facade crate. The implementation lives in ghostlight-core (churny brain) and
//! ghostlight-transport (stable substrate); this crate re-exports both under the historical
//! `ghostlight::` paths so integration tests and external references keep compiling, and hosts
//! the `ghostlight` executable (CLI + service).

pub use ghostlight_core::{browser, governance, hub, install, mcp, messages, origin};
pub use ghostlight_transport::error::{Error, Result, ToolError};
pub use ghostlight_transport::init_tracing;
pub use ghostlight_transport::{error, handshake, instance, observability, proc};

/// Historical path continuity (`ghostlight::native::...`).
pub mod native {
    pub use ghostlight_core::messages;
    pub use ghostlight_transport::host;
    /// The two halves of the old ipc module, merged back under the historical path.
    pub mod ipc {
        pub use ghostlight_core::hub::endpoint::*;
        pub use ghostlight_transport::ipc::*;
    }
}

/// Historical path continuity (`ghostlight::transport::...`).
pub mod transport {
    pub use crate::native;
    pub use ghostlight_core::mcp;
    pub use ghostlight_transport::watchdog;
}
