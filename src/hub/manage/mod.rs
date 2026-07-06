// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The manage zone -- the OPERATOR SURFACE: observe runtime state, diagnose the chain, and
//! administer policy. Multiple deliveries (the loopback HTTP UI today; the `doctor`/`status` CLI
//! and the instrumentation sink fold in later). It is a peer top-level to [`crate::hub::inbound`]
//! and [`crate::hub::outbound`], NOT under `inbound/`: it does NOT ingest tool calls, never flows
//! through `serve_session`, and reads the `ConfigStore` / audit / live state directly.
//!
//! The management plane is PERMANENTLY loopback. There is no legitimate case for administering
//! Ghostlight remotely -- remote policy changes to a service driving an authenticated browser
//! session is a security non-starter. `manage.web.from` is locked to `localhost` (the validator
//! rejects any other member), and the router additionally hard-codes a loopback check on the
//! peer address. An org layer can disable the plane (`manage.web.enabled = false`); it can never
//! widen it.

pub mod web;
