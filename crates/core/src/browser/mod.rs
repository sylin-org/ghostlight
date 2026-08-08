// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Browser-domain classification, resource, and page-content helpers.
//!
//! This bounded context (see docs/design/ghostlight-service-architecture.md section 3)
//! is the browser-specific plugin over the domain-agnostic [`crate::governance`] core: it
//! owns the secret-value redaction overlay ([`redact`]) applied to page output, the
//! domain-pattern module ([`pattern`], authored-pattern syntax plus the WHATWG-parser-backed
//! matcher), the host-polarity evaluator ([`polarity`], ADR-0022 Decision 4: per-grant
//! hosts.allow/hosts.deny evaluation over already-normalized hosts, consumed by grant
//! enforcement from s05 on), the sacred never-touch list ([`sacred`], ADR-0018 step 2, always
//! enforced), the URL-to-governing-resource classification ([`resource`], g13: what a URL IS,
//! for the grant enforcement pre/post-dispatch checks). Canonical operation semantics live in
//! [`crate::operation::registry`]; model-facing declarations live only at the MCP edge.
//!
//! It may depend on the governance core and on std/serde; the governance core must never
//! depend back on this module.

pub mod form_match;
pub mod mechanism;
pub mod pattern;
pub mod polarity;
pub mod redact;
pub mod resource;
pub mod sacred;
