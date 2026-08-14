# Roadmap

Ghostlight's current development target is the planned 1.0 release. The public 0.8 release remains
the installed baseline until 1.0 completes its release gates.

## 1.0 release completion

- Finish the accepted visible-browser journey matrix across screenshots, semantic input, upload,
  dialogs, governed denial, reconnect, tab-close interlock, tab grouping, and multiple concurrent
  client sessions.
- Produce signed Windows and Linux packages containing the orchestrator/workbench and both
  version-matched connectors.
- Verify clean install, upgrade, harness registration, native messaging, tray, notification,
  headless fallback, and uninstall on each platform.
- Complete live-browser verification on supported Chromium families and publish a matching 1.0
  extension only after service compatibility is proven.
- Reconcile release metadata, compatibility, public status, stores, package registries, and final
  public documentation from the signed artifacts.

The canonical gates are [`docs/1.0/ACCEPTANCE.md`](docs/1.0/ACCEPTANCE.md); mutable evidence is in
[`docs/STATUS.md`](docs/STATUS.md).

## Direction after 1.0

- Add browser families only behind the existing browser port and explicit capability negotiation.
  A new browser must not spread product policy into its adapter.
- Expand the workbench only when a new destination materially improves at-a-glance understanding,
  recovery, or explicit user control. Monitor, MCP integrations, and Status are a deliberate
  ceiling, not a starting point.
- Evolve model-facing jobs in the orchestrator without routine edits to the MCP connector, browser
  connector, shared bridge, or extension.
- Keep policy local, monotonic, inspectable, and payload-free. Ghostlight will not add telemetry,
  activation, update polling, or a hosted control plane.

Historical proposals remain in the ADR, design, research, and business records. They are evidence,
not automatic roadmap commitments. Use
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for a concrete user job;
accepted work receives an ADR or an explicit 1.0 contract amendment.
