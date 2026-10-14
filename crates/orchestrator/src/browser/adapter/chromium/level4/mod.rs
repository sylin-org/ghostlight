//! Chromium adapter protocol 4: the service's translation of browser work into frozen
//! primitives.
//!
//! The extension runs a small closed set of primitives and reports facts. The service decides
//! what they mean. This module is the service's side of that contract: the wire vocabulary, the
//! connection that carries it, the world messages its page assets understand, and the frame
//! trees, locators and document scope that keep every call inside the documents the authority
//! admitted.

pub mod connection;
#[cfg(test)]
mod fake;
pub mod locator;
pub mod scope;
pub mod tree;
pub mod wire;
pub mod world;
