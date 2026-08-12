import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PLUGIN_SCHEMA = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
const MCP_SCHEMA = "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json";
const PLUGIN_NAME_PATTERN = /^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/;
const MAX_PLUGIN_NAME_LENGTH = 64;
const MAX_DISCOVERY_DESCRIPTION_LENGTH = 100;
const EXTENSION_NAMESPACE_PATTERN = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+$/i;
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function readAsciiJson(relativePath) {
  const source = readFileSync(resolve(repositoryRoot, relativePath), "utf8");
  assert.doesNotMatch(source, /[^\x00-\x7f]/, `${relativePath} must remain ASCII`);
  return JSON.parse(source);
}

function sortedKeys(value) {
  return Object.keys(value).sort();
}

function schemaVersion(schema) {
  const match = schema.match(/\/schemas\/([^/]+)\//);
  assert.ok(match, `Schema URL does not contain a version: ${schema}`);
  return match[1];
}

const plugin = readAsciiJson("plugin.json");
const mcp = readAsciiJson("mcp.json");
const cargo = readFileSync(resolve(repositoryRoot, "Cargo.toml"), "utf8");

const workspacePackage = cargo.match(/\[workspace\.package\]([\s\S]*?)(?=\n\[|$)/);
assert.ok(workspacePackage, "Cargo.toml must define [workspace.package]");
const workspaceVersion = workspacePackage[1].match(/^version\s*=\s*"([^"]+)"/m);
assert.ok(workspaceVersion, "[workspace.package] must define version");

const permittedPluginFields = new Set([
  "$schema",
  "name",
  "version",
  "description",
  "author",
  "homepage",
  "repository",
  "license",
  "keywords",
  "extensions"
]);
for (const field of Object.keys(plugin)) {
  assert.ok(permittedPluginFields.has(field), `plugin.json contains unknown field ${field}`);
}

assert.deepEqual(sortedKeys(plugin), [
  "$schema",
  "author",
  "description",
  "homepage",
  "keywords",
  "name",
  "repository",
  "version"
]);
assert.equal(plugin.$schema, PLUGIN_SCHEMA);
assert.equal(plugin.name, "ghostlight");
assert.ok(plugin.name.length >= 1 && plugin.name.length <= MAX_PLUGIN_NAME_LENGTH);
assert.match(plugin.name, PLUGIN_NAME_PATTERN);
assert.equal(plugin.version, workspaceVersion[1]);
assert.equal(plugin.description, "Visible local browser automation in signed-in Chromium, with optional policy and audit.");
assert.ok(plugin.description.length <= MAX_DISCOVERY_DESCRIPTION_LENGTH);
assert.deepEqual(plugin.author, {
  name: "Sylin",
  email: "hello@sylin.org",
  url: "https://sylin.org/"
});
assert.equal(plugin.homepage, "https://sylin.org/ghostlight/");
assert.equal(plugin.repository, "https://github.com/sylin-org/ghostlight");
assert.deepEqual(plugin.keywords, ["browser", "chromium", "local", "mcp"]);
assert.equal(Object.hasOwn(plugin, "license"), false);
assert.equal(Object.hasOwn(plugin, "extensions"), false);
assert.equal(Object.hasOwn(plugin, "skills"), false);
assert.equal(Object.hasOwn(plugin, "mcpServers"), false);
assert.equal(existsSync(resolve(repositoryRoot, "skills")), false);
assert.deepEqual(
  readdirSync(repositoryRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && EXTENSION_NAMESPACE_PATTERN.test(entry.name))
    .map((entry) => entry.name),
  [],
  "MCP-only package must not contain a top-level client extension namespace directory"
);

assert.deepEqual(sortedKeys(mcp), ["$schema", "mcpServers"]);
assert.equal(mcp.$schema, MCP_SCHEMA);
assert.equal(schemaVersion(mcp.$schema), schemaVersion(plugin.$schema));
assert.deepEqual(sortedKeys(mcp.mcpServers), ["ghostlight"]);

const server = mcp.mcpServers.ghostlight;
assert.deepEqual(server, {
  type: "stdio",
  command: "ghostlight-mcp-connector"
});
assert.equal(server.command.includes("/"), false);
assert.equal(server.command.includes("\\"), false);
for (const field of ["args", "env", "cwd", "url", "headers"]) {
  assert.equal(Object.hasOwn(server, field), false, `Ghostlight MCP server must not define ${field}`);
}
assert.notEqual(server.type, "streamable-http");
assert.notEqual(server.type, "sse");

const forbiddenSecretField = /^(?:api[-_]?key|authorization|credential|password|secret|token)$/i;
function assertNoSecretFields(value, location) {
  if (Array.isArray(value)) {
    value.forEach((entry, index) => assertNoSecretFields(entry, `${location}[${index}]`));
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [field, entry] of Object.entries(value)) {
    assert.doesNotMatch(field, forbiddenSecretField, `${location}.${field} is a secret-bearing field`);
    assertNoSecretFields(entry, `${location}.${field}`);
  }
}
assertNoSecretFields(plugin, "plugin.json");
assertNoSecretFields(mcp, "mcp.json");

console.log("agent plugin contract ok: Agent Plugins 1.0.0 MCP-only package");
