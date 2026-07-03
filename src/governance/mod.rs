//! Governance core -- the domain-agnostic policy layer.
//!
//! This bounded context (see docs/design/ghostlight-service-architecture.md section 3)
//! names no browser type. It owns the dispatch seam ([`dispatch`]), the typed config
//! registry ([`config`]), the stable denial id scheme ([`denial`]), the per-call grant
//! enforcement decision core ([`enforcement`]), the deterministic plain-language policy
//! renderer ([`explain`]), the audit-replay policy simulator ([`simulate`]), the embedded
//! policy manifest templates ([`templates`]), the policy manifest ([`manifest`]), the audit
//! flight recorder ([`audit`]), and the policy-decision-point/policy-enforcement-point
//! contract ([`ports`]). The dependency direction is strictly inward: infra and the browser
//! plugin may depend on this module; this module depends only on std and serde (plus
//! `uuid`/`chrono`/`sha2` for audit, manifest identity, and denial ids). A fail-closed
//! arch-test (task A7) enforces that.

pub mod audit;
pub mod config;
pub mod denial;
pub mod dispatch;
pub mod enforcement;
pub mod explain;
pub mod manifest;
pub mod ports;
pub mod simulate;
pub mod templates;
