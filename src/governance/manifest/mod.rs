//! Policy manifest -- the org policy / grants document (ADR-0020). Domain-agnostic core:
//! generic over any policy doc, names no browser type.
//!
//! [`identity`] (ADR-0020 commitment 5) computes name, version, and a content hash so every
//! logged decision is attributable to the exact policy version that made it. [`document`]
//! (G12) is the full schema-2 manifest: format types, parsing, and validation, reusing
//! `identity`'s `canonical_hash` for its own hash step. [`source`] (G12) resolves WHERE the
//! active manifest comes from (org policy file, `--manifest`/`BROWSER_MCP_MANIFEST`, or none =
//! all-open) and orchestrates loading it. Grant EVALUATION (matching, enforcement) is G13's.

pub mod document;
pub mod identity;
pub mod source;
