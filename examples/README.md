# Ghostlight 1.0 policy examples

These files use the exact flat version-1 policy schema decoded by the orchestrator. Set
`GHOSTLIGHT_POLICY_FILE` to a local example, or provision a `managed: true` example through
`GHOSTLIGHT_MANAGED_AUTHORITY_FILE` after choosing a real future expiry.

The filenames preserve the project's earlier scenario vocabulary. Ghostlight 1.0 has no observe
mode: `developer-observe.json` is therefore a restrictive read-only evaluation boundary, not a
shadow-enforcement mode. Loopback cannot be granted by policy.

Unknown fields fail closed. See `docs/guides/governance-configuration.md` before adapting a file.
