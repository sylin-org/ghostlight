// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The outbound zone -- per-capability EXECUTORS that translate a native tool-call into the
//! backend's commands and await its reply.
//!
//! Each capability (the browser today; desktop, shell, filesystem, network later) gets its own
//! executor module here, symmetric with the per-channel ingestors in [`crate::hub::inbound`].
//! The pair forms the matrix: inbound ingestors converge on the governance pipeline, which
//! dispatches a native tool-call to the matching outbound executor. The pipeline knows neither
//! end; the executors know no policy.
//!
//! Today only the browser capability exists ([`browser::Browser`]); it drives the user's own
//! authenticated Chromium session over the extension link.

pub mod browser;
