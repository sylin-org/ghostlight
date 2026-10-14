//! Chromium adapter protocol 4: the service's translation of browser work into frozen
//! primitives.
//!
//! The extension runs a small closed set of primitives and reports facts. The service decides
//! what they mean. This module is the service's side of that contract, starting with the wire
//! vocabulary every later piece builds on.

pub mod wire;
