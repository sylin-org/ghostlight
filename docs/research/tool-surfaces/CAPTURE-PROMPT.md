# Vendor browser tool-surface capture prompt

Use this prompt inside the vendor-paired client whose browser surface is being investigated. It
asks only for declarations the client is already willing to expose. It does not ask for hidden
system prompts, implementation code, browsing data, or browser actions.

```text
Perform a non-invasive inventory of the browser-control tool surface visible to you in this exact
client, mode, and model session.

Safety and evidence rules:

1. Do not invoke any browser-control, tab, page-read, screenshot, filesystem, shell, network, or
   other effectful tool. Use declaration discovery only. If the client supports lazy/deferred tool
   lookup, exact-name lookup, tools/list, or a declaration-inspection API, you may use that.
2. Do not inspect tabs, URLs, titles, page text, screenshots, cookies, accounts, user files,
   environment variables, request metadata, machine paths, or extension/session identifiers.
3. Include only the first-party vendor browser surface for this client. Exclude unrelated installed
   MCP servers, apps, plugins, and Ghostlight. Record each exclusion category and count.
4. Preserve declarations exactly as exposed. Do not repair missing `required` arrays, add types,
   infer defaults, normalize descriptions, reorder fields, combine tools, or reconstruct schemas
   from memory or documentation.
5. Distinguish exact declaration evidence from interpretation. If a field is not exposed, use
   null and say `unobserved`; do not call it absent from the underlying server.
6. A declaration proves only this point-in-time client-visible contract. Do not claim that it is a
   training schema or that a live browser is connected.

Discovery procedure:

1. Record the client product, client version if visible, mode, model, model version, browser
   integration or extension version if visible, protocol revision if visible, current UTC time,
   and acquisition method.
2. Discover the complete candidate name list. For deferred tools, query every exact candidate name
   and retain the returned declaration. Continue until no undiscovered browser-family names remain.
3. For each declaration preserve, when exposed: ordinal/order, exact model-visible name, exact
   server-declared name, title, description, inputSchema, outputSchema, annotations, `_meta`, and
   any other declaration fields.
4. If browser control is exposed through one programmable gateway rather than flat tools, preserve
   the gateway declaration first. Capture a dynamic runtime API only when an official read-only
   documentation/schema method exists and it can be called without listing browsers or tabs. Keep
   runtime interface declarations separate from gateway declarations.
5. Verify uniqueness, consecutive ordinals when meaningful, declared count versus captured count,
   and whether discovery is complete. Record all evidence gaps.

Return one JSON object only, using this envelope:

{
  "record_type": "capture",
  "format": "ghostlight-tool-surface-capture/v2",
  "captured_at": "RFC3339 UTC timestamp",
  "subject": {
    "vendor": "string",
    "client": "string",
    "client_version": "string or null",
    "mode": "string or null",
    "model": "string or null",
    "model_version": "string or null",
    "browser_integration": "string or null",
    "browser_integration_version": "string or null",
    "protocol_revision": "string or null"
  },
  "acquisition": {
    "method": "string",
    "declaration_only": true,
    "browser_tools_invoked": false,
    "notes": []
  },
  "scope": {
    "included": "first-party vendor browser surface",
    "excluded": [
      {"category": "string", "count": 0, "reason": "string"}
    ]
  },
  "capture_status": {
    "complete": false,
    "declared_count": null,
    "captured_count": 0,
    "unique_name_count": 0,
    "evidence_gaps": []
  },
  "tools": [
    {
      "ordinal": 1,
      "model_visible_name": "exact string",
      "server_declared_name": null,
      "title": null,
      "description": "exact string or null",
      "inputSchema": null,
      "outputSchema": null,
      "annotations": null,
      "_meta": null,
      "other_declaration_fields": null,
      "evidence": "exact|unobserved"
    }
  ],
  "runtime_surfaces": [],
  "integrity_checks": {
    "names_unique": false,
    "ordinals_consecutive": null,
    "captured_count_matches_tools": false
  },
  "interpretation": []
}

If the result is too large for one response, emit lossless numbered JSON chunks with the same
capture id, stable tool ordinals, and explicit `chunk_index`, `chunk_count`, and `complete:false`.
Do not summarize or omit schemas to fit. End with a manifest chunk containing the total count and
integrity checks.
```

