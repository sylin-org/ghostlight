//! Test-only checks of what a model actually sees (ADR-0196 Decision 13).
//!
//! These tests read the advertised catalog and the authored next steps the way a strict client or
//! a model would: a schema that rejects its own examples, or a recovery hint that names a field a
//! tool lacks, fails here.
//!
//! - `schema` is a small dependency-free JSON Schema checker for the keywords the catalog uses.
//! - `catalog` validates every example and a list of known-good calls against each tool's own
//!   advertised input schema, and decodes them through the real decoder.
//! - `golden` compares or rewrites the pinned model-surface files (catalog and results).
//! - `hints` checks that every next step naming a tool asks only for fields that tool accepts.

mod catalog;
pub(crate) mod golden;
mod hints;
mod schema;
