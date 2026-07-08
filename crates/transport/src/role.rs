// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The process's role marker (ADR-0030 Decision 1 addendum; PINS.md SS8): once this process learns
//! whether it won or lost the ADAPTER/CONTROL endpoint claim, it records that role ONCE, here, and
//! the two seams where a mismatch would mean the SoC boundary already failed elsewhere -- the
//! governance chokepoint (`transport::mcp::server::serve_session`) and the service-spawn path (H6)
//! -- assert against it as their first action. This is a fail-loud backstop, NOT a substitute for
//! the structural separation (the ADAPTER's code never calls governance; the SERVICE's code never
//! calls spawn): it is a no-op (no output, no behavior change) whenever the role is already
//! correct, so it does not touch the all-open byte-identity invariant.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Service,
    Adapter,
}

static ROLE: OnceLock<Role> = OnceLock::new();

/// Record this process's role. Called exactly once, immediately after the ADAPTER/CONTROL
/// endpoint-claim result is known (ADR-0030 Decision 1).
pub fn set_role(role: Role) {
    if ROLE.set(role).is_err() {
        panic!("ghostlight process role decided twice");
    }
}

/// Read this process's role. Panics if [`set_role`] has not run yet.
pub fn role() -> Role {
    *ROLE
        .get()
        .unwrap_or_else(|| panic!("ghostlight process role read before it was decided"))
}

/// Pure assertion: `current` must equal `required`, else panic naming `what` (the caller's own
/// function name) and both roles.
pub fn assert_role(current: Role, required: Role, what: &str) {
    assert!(
        current == required,
        "invariant violated: {what} must only run when this process's role is {required:?}, but it is {current:?}"
    );
}

/// = `assert_role(role(), Role::Service, what)`: call as the first line of the governance
/// chokepoint.
pub fn assert_service_role(what: &str) {
    assert_role(role(), Role::Service, what);
}

/// = `assert_role(role(), Role::Adapter, what)`: call as the first line of the spawn-on-demand
/// function (H6).
pub fn assert_adapter_role(what: &str) {
    assert_role(role(), Role::Adapter, what);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "must only run when this process's role is Service")]
    fn adapter_role_hitting_the_governance_chokepoint_panics() {
        assert_role(Role::Adapter, Role::Service, "test");
    }

    #[test]
    #[should_panic(expected = "must only run when this process's role is Adapter")]
    fn service_role_hitting_spawn_on_demand_panics() {
        assert_role(Role::Service, Role::Adapter, "test");
    }

    #[test]
    fn matching_roles_do_not_panic() {
        assert_role(Role::Service, Role::Service, "test");
        assert_role(Role::Adapter, Role::Adapter, "test");
    }
}
