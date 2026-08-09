// SPDX-License-Identifier: Apache-2.0 OR MIT
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const content = fs.readFileSync(path.join(root, "extension/content.js"), "utf8");
const sensitive = require(path.join(root, "extension/lib/sensitive.js"));

test("form inspection reports structural sensitivity without reading field values", () => {
  assert.match(content, /sensitive:\s*sensitive\(el\)/);
  assert.match(content, /GhostlightSensitive\.isSensitiveField/);
  const start = content.indexOf("function readControl(el)");
  const end = content.indexOf("function submitKind(el)", start);
  assert.ok(start >= 0 && end > start, "readControl source is bounded");
  assert.doesNotMatch(content.slice(start, end), /\bvalue\s*:/);
});

test("credential classifier covers platform tokens and structural secret labels", () => {
  for (const facts of [
    { type: "password" },
    { autocomplete: "username one-time-code" },
    { label: "API key" },
    { name: "client_secret" },
    { id: "mfa-code" },
    { placeholder: "Verification token" },
    { ariaLabel: "PIN" },
    { label: "Bearer token" },
    { name: "session_token" },
    { label: "Recovery key" },
  ]) {
    assert.equal(sensitive.isSensitiveField(facts), true, JSON.stringify(facts));
  }
  for (const facts of [
    null,
    { type: "text", label: "Display name" },
    { name: "token_budget" },
  ]) {
    assert.equal(sensitive.isSensitiveField(facts), false, JSON.stringify(facts));
  }
});

test("strict form writes revalidate connectivity eligibility type and sensitivity before mutation", () => {
  const start = content.indexOf("function setFormValue(ref, value, rejectSensitive, expectedType)");
  const end = content.indexOf("function setFiles(ref, files)", start);
  assert.ok(start >= 0 && end > start, "setFormValue source is bounded");
  const source = content.slice(start, end);
  for (const fact of [
    "!el.isConnected",
    "!target.isConnected",
    "target.disabled",
    "target.readOnly",
    "controlType(target) !== expectedType",
    "sensitive(target)",
  ]) {
    assert.ok(source.includes(fact), `missing strict revalidation fact: ${fact}`);
    assert.ok(
      source.indexOf(fact) < source.indexOf("scrollIntoView"),
      `${fact} must be checked before page mutation`,
    );
  }
});

test("semantic action resolution always returns a boolean sensitivity fact for settable targets", () => {
  const assignments = content.match(/summary\.sensitive\s*=\s*sensitive\(input\);/g) || [];
  assert.equal(assignments.length, 2, "ref and semantic resolution both stamp sensitivity");
  assert.match(content, /const input = innerInput\(el\) \|\| el;/);
  assert.match(content, /const input = \(el && innerInput\(el\)\) \|\| el;/);
});

test("detached refs fail before semantic resolution coordinates scroll or mutation", () => {
  const start = content.indexOf("function deref(ref)");
  const end = content.indexOf("function staleRefMessage(ref)", start);
  assert.ok(start >= 0 && end > start, "deref source is bounded");
  const source = content.slice(start, end);
  assert.match(source, /!el \|\| !el\.isConnected/);
  assert.ok(source.indexOf("!el.isConnected") < source.indexOf("return el"));
  assert.match(content, /function refCoordinates\(ref\) \{\s*const el = deref\(ref\);/);
  assert.match(content, /function resolveActionable\(target\)[\s\S]*const el = deref\(target\.ref\);/);
});

test("form inspection exposes submit eligibility for immediate revalidation", () => {
  assert.match(content, /submits\.push\(\{[\s\S]*disabled: !!el\.disabled,/);
});
