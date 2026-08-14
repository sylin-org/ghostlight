# Ghostlight 1.0 policy examples

These files use the strict schema-3 policy format decoded by the orchestrator. Set
`GHOSTLIGHT_POLICY_FILE` to a local example. A managed source uses the same manifest after its
signed bundle has been verified.

`developer-unrestricted.json` expresses the all-open authority explicitly. With no configured
policy, Ghostlight remains all-open without loading an example. `developer-observe.json` shows
shadow enforcement: ordinary policy denials are reported while work continues. Loopback and other
permanently protected resources cannot be granted or observed through.

Unknown fields and invalid host patterns fail closed. See
`docs/guides/governance-configuration.md` before adapting a file.
