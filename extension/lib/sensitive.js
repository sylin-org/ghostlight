// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- policy-free structural classification for credential-class form fields.

(function () {
"use strict";

const SENSITIVE_AUTOCOMPLETE = Object.freeze([
  "current-password", "new-password", "one-time-code",
  "cc-number", "cc-csc", "cc-exp", "cc-exp-month", "cc-exp-year",
]);

const STRUCTURAL_SECRET = /(?:password|passcode|\bpin\b|one[\s_-]*time[\s_-]*(?:code|password)|otp|two[\s_-]*factor(?:[\s_-]*(?:code|token))?|2fa|mfa|verification[\s_-]*(?:code|token)|recovery[\s_-]*(?:code|key)|auth(?:entication|orization)?[\s_-]*(?:code|token)|api[\s_-]*key|client[\s_-]*secret|access[\s_-]*token|refresh[\s_-]*token|bearer[\s_-]*token|session[\s_-]*token|secret[\s_-]*(?:key|token)|private[\s_-]*key)/i;

function isSensitiveField(facts) {
  if (!facts || typeof facts !== "object" || Array.isArray(facts)) return false;
  const type = typeof facts.type === "string" ? facts.type.toLowerCase() : "";
  if (type === "password" || type === "hidden") return true;
  const autocomplete = typeof facts.autocomplete === "string"
    ? facts.autocomplete.toLowerCase()
    : "";
  if (SENSITIVE_AUTOCOMPLETE.some((token) => autocomplete.includes(token))) return true;
  const structural = [facts.label, facts.ariaLabel, facts.placeholder, facts.name, facts.id]
    .filter((value) => typeof value === "string" && value)
    .join(" ");
  return STRUCTURAL_SECRET.test(structural);
}

const api = Object.freeze({ SENSITIVE_AUTOCOMPLETE, isSensitiveField });
if (typeof module !== "undefined" && module.exports) module.exports = api;
else self.GhostlightSensitive = api;
})();
