// How the installed Linux journey records and checks one MCP tool call.
//
// The journey owns the MCP request channel, the results report and its save; this module owns what
// one call writes into that report and what it refuses. A reply is recorded before it is checked,
// so a failed call is already on disk when the assertion throws.
import assert from "node:assert/strict";

/**
 * Returns `call(name, args)`, which sends `tools/call` through `request`, appends one invocation to
 * `report.invocations`, calls `save`, and then requires a protocol-clean reply whose structured
 * result succeeded. It resolves to that structured result. The recorded reason is the result's
 * `facts.reason` only when it is a plain snake_case code, so no free text reaches the report.
 */
export function invoker({ request, report, save }) {
  return async function call(name, args) {
    const reply = await request("tools/call", { name, arguments: args });
    const result = reply.result?.structuredContent;
    const reason = result?.facts?.reason;
    report.invocations.push({ tool: name, status: result?.status, effect: result?.effect,
      reason: typeof reason === "string" && /^[a-z_]+$/.test(reason) ? reason : null,
      ...(Number.isInteger(reply.error?.code) ? { protocol_error_code: reply.error.code } : {}) });
    save();
    assert.equal(reply.error, undefined, "Unexpected MCP protocol failure");
    assert.equal(reply.result?.structuredContent?.status, "succeeded", `${name} did not succeed`);
    return reply.result.structuredContent;
  };
}
