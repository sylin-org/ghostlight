// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The `inbound.pipe` owner-only local listener lifecycle and policy gate.
//!
//! Platform-specific binding, same-user peer credentials, role admission, and anti-squat proof
//! live in [`crate::hub::endpoint`]. This module keeps one concrete job: take the listener already
//! claimed by the composition root, honor `inbound.pipe.enabled`, and run the bridge/control
//! accept loop for the service lifetime.

use crate::hub::endpoint as ipc;
use crate::hub::ServiceContext;

/// The live `inbound.pipe` listener. The composition root claims it as the process-level
/// single-instance guard before constructing this value.
pub struct PipeTransport {
    /// `Some` until `run` is called (which moves the listener into the spawned task).
    listener: Option<ipc::AdapterListener>,
}

impl PipeTransport {
    /// Take ownership of the already-claimed local bridge/control listener.
    pub fn new(listener: ipc::AdapterListener) -> Self {
        Self {
            listener: Some(listener),
        }
    }

    /// Run the bridge/control accept loop for the service lifetime.
    ///
    /// A policy-disabled listener logs and returns without serving. Each admitted peer otherwise
    /// receives a clone of the shared service context inside [`ipc::serve_adapters`].
    pub async fn run(self, ctx: ServiceContext) {
        let enabled = {
            let resolution = ctx.store.current_resolution();
            let resolved = resolution
                .get(crate::governance::config::INBOUND_PIPE_ENABLED)
                .expect("registered key resolves");
            resolved.value.as_bool().unwrap_or(true)
        };
        if !enabled {
            tracing::info!(
                "local bridge/control listener disabled by policy (inbound.pipe.enabled = false); \
                 not serving"
            );
            return;
        }

        let Some(listener) = self.listener else {
            tracing::error!("inbound.pipe transport has no listener");
            return;
        };
        tracing::info!("inbound.pipe bridge/control listener active");
        if let Err(e) = ipc::serve_adapters(ctx, listener).await {
            tracing::error!(error = %e, "inbound.pipe endpoint failed");
        }
    }
}
