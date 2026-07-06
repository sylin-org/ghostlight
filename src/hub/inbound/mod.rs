// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The inbound zone -- per-channel INGESTORS that translate a wire/transport into a native
//! tool-call and converge on the governance pipeline ([`serve_session`]).
//!
//! Each channel (the named-pipe/UDS listener thin MCP adapters dial into today, plus the local
//! HTTP/WS adapter a web MCP client drives) gets its own module here, symmetric with the
//! per-capability executors in [`crate::hub::outbound`]. The pair forms the matrix: inbound
//! ingestors converge on the pipeline, which dispatches a native tool-call to the matching
//! outbound executor. The pipeline knows neither end; the ingestors know no policy.
//!
//! [`serve_session`]: crate::transport::mcp::server::serve_session

pub mod web;
