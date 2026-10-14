//! Test-only checks of what a model actually sees (ADR-0196 Decision 13).
//!
//! The gate used to measure code, not the model's view: catalog schemas that rejected their own
//! examples and recovery hints that named fields a tool lacks both passed it. These tests read
//! the advertised catalog and the authored next steps the way a strict client or a model would.
//!
//! - `schema` is a small dependency-free JSON Schema checker for the keywords the catalog uses.
//! - `catalog` validates every example and a list of known-good calls against each tool's own
//!   advertised input schema, and decodes them through the real decoder.
//! - `hints` checks that every next step naming a tool asks only for fields that tool accepts.

mod catalog;
mod hints;
mod schema;
