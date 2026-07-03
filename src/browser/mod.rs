//! Browser domain plugin -- the tool registry and page-content redaction.
//!
//! This bounded context (see docs/design/ghostlight-service-architecture.md section 3)
//! is the browser-specific plugin over the domain-agnostic [`crate::governance`] core: it
//! owns the tool registry ([`directory`], ADR-0024 Decision 1: the single per-tool
//! authority, absorbing the ADR-0022 Decision 2 action directory's per-action bound
//! capability requirement sets and agent-facing descriptions as its per-tool variants;
//! there are no per-tool code homes, the registry IS the per-tool authority), the
//! secret-value redaction overlay ([`redact`]) applied to `read_page` output, the
//! domain-pattern module ([`pattern`], authored-pattern syntax plus the WHATWG-parser-backed
//! matcher), the host-polarity evaluator ([`polarity`], ADR-0022 Decision 4: per-grant
//! hosts.allow/hosts.deny evaluation over already-normalized hosts, consumed by grant
//! enforcement from s05 on), the sacred never-touch list ([`sacred`], ADR-0018 step 2, always
//! enforced), the URL-to-governing-resource classification ([`resource`], g13: what a URL IS,
//! for the grant enforcement pre/post-dispatch checks), and the tool-advertisement filter
//! ([`advertise`], g14: a visibility optimization over `tools/list`, never a security
//! boundary). `directory` is the sole validity, enforcement, advertisement, and audit
//! authority as of ADR-0024; the earlier per-tool code stubs and the observe/mutate
//! classification table they carried are deleted (ADR-0024 Decision 5).
//!
//! It may depend on the governance core and on std/serde; the governance core must never
//! depend back on this module.

pub mod advertise;
pub mod directory;
pub mod pattern;
pub mod polarity;
pub mod redact;
pub mod resource;
pub mod sacred;
