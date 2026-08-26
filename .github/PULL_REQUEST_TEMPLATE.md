## Problem

What user, agent, operator, or maintainer problem does this change address?

## Change

What changed, and why is this the smallest coherent approach?

## Boundaries

- [ ] The 13 trained tool schemas remain byte-stable, or this is additive growth permitted by the capability registry.
- [ ] The extension remains policy-free; classification, authorization, and audit remain in the service.
- [ ] No telemetry, activation dependency, or other phone-home behavior was added.
- [ ] The Apache-2.0 OR MIT license covering the whole repository is preserved; contributions carry DCO sign-off.
- [ ] Public claims describe behavior that ships now.

## Validation

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --locked --no-fail-fast --workspace`
- [ ] Relevant extension or launcher tests were run.
- [ ] Documentation, ADR, Trust Center, and live-browser evidence were updated where needed.
- [ ] Commits carry DCO sign-off.

