# Roadmap

Ghostlight 2.0 is the current development target. The public 1.3.14 release remains the installed
baseline until 2.0 passes its release checks and is published.

## 2.0 release

- Build the release candidate, prove the install from its bundle on Windows and Linux, and run a
  1.x upgrade on another machine while the store still serves 1.x. Nothing is tagged before
  `v2.0.0`.
- Submit the 2.0 extension for staged store review with the 2.0 privacy text already public.
- Cut over in one sitting: the release, the npm launcher, the short install commands, the
  extension, the public state, the MCP Registry and WinGet.

The steps are in [`docs/RELEASE.md`](docs/RELEASE.md), the checks in
[`docs/STATUS.md`](docs/STATUS.md), and the acceptance contract in
[`docs/1.0/ACCEPTANCE.md`](docs/1.0/ACCEPTANCE.md).

## 2.x

2.0 settles the seams: the three executables, the relay and adapter protocols, the policy schema,
the agent entries and the install home. 2.x rebuilds the internals lean behind them. A seam stays
stable by default and changes only when a separation-of-concerns analysis shows a meaningful
architecture change needs it.

- Rebuild the orchestrator's internals behind the existing seams: the installer's agent catalog as
  data, commands inside the running service, and the install folded into one command.
- Evolve model-facing jobs in the orchestrator without routine edits to the edge, the relay or the
  extension.
- Expand the workbench only when a new destination materially improves at-a-glance understanding,
  recovery, or explicit user control.
- Add browser families only behind the existing browser port and explicit capability negotiation.
  A new browser must not spread product policy into its adapter.
- Keep policy local, monotonic, inspectable, and payload-free. Ghostlight will not add telemetry,
  activation, update polling, or a hosted control plane.

## Later

- macOS, once the Windows and Linux line is settled.

Historical proposals remain in the ADR, design, research, and business records. They are evidence,
not automatic roadmap commitments. Use
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for a concrete user job;
accepted work receives an ADR or an explicit contract amendment.
