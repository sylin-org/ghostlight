// The host grammar the policy editor speaks back to a person.
//
// Ghostlight's suffix wildcard covers subdomains only. That is its own choice and it differs from
// what Chrome's blocklist grammar and the nearest comparable products do, so the editor states
// what a pattern matches instead of expecting anyone to carry the rule in their head.
//
// These assertions exist because the readback is a claim about what the production matcher will
// do. If the two ever disagree, the window is lying politely, which is worse than saying nothing.
// The Rust side is pinned by governance::effective::tests::suffix_wildcards_cover_subdomains_and_
// never_the_bare_host and by manifest::tests::host_patterns_are_exact_suffix_or_universal.
//
// Run with: node tests/policy-grammar.mjs
import { createRequire } from "node:module";
import { join, resolve } from "node:path";

const require = createRequire(import.meta.url);
const repository = resolve(import.meta.dirname, "..");
const words = require(join(repository, "crates", "orchestrator", "ui", "lib", "words.js"));

const checks = [];
const check = (what, ok, detail = "") => checks.push([what, ok, detail]);

/* ---------------------------- accepted patterns --------------------------- */

for (const valid of ["*", "example.com", "*.example.com", "a-b.example"]) {
  check(`accepts ${valid}`, words.validHostPattern(valid));
}
for (const invalid of ["", "https://example.com", "example.com/path", "foo.*.example.com", ".example.com", "*.example.com:443"]) {
  check(`refuses ${invalid || "(empty)"}`, !words.validHostPattern(invalid));
}

/* ------------------------------ what it says ------------------------------ */

check("the universal pattern is named plainly",
  words.hostReadback("*") === "any website",
  words.hostReadback("*"));

check("a suffix wildcard says it excludes the bare host",
  words.hostReadback("*.example.com") === "anything under example.com, but not example.com itself",
  words.hostReadback("*.example.com"));

check("an exact host says it excludes subdomains",
  words.hostReadback("example.com") === "example.com exactly, and none of its subdomains",
  words.hostReadback("example.com"));

check("an unusable pattern says so rather than guessing",
  words.hostReadback("foo.*.example.com").includes("not a site Ghostlight can match"),
  words.hostReadback("foo.*.example.com"));

check("nothing typed reads back as nothing", words.hostReadback("  ") === "");

/* --------------------------- what covers what ----------------------------- */

check("the universal pattern covers everything", words.patternCovers("*", "anything.test"));
check("a suffix wildcard covers its subdomains", words.patternCovers("*.example.com", "a.example.com"));
check("a suffix wildcard covers a narrower wildcard", words.patternCovers("*.example.com", "*.a.example.com"));
check("a suffix wildcard does not cover the bare host", !words.patternCovers("*.example.com", "example.com"));
check("an exact host does not cover its subdomains", !words.patternCovers("example.com", "a.example.com"));
check("coverage ignores case", words.patternCovers("example.com", "EXAMPLE.com"));

/* ------------------------------ the labels -------------------------------- */

check("every capability has a plain label",
  words.CAPABILITY_ORDER.every((capability) => typeof words.CAPABILITY_WORDS[capability] === "string"),
  JSON.stringify(words.CAPABILITY_WORDS));

check("no label uses the policy word for itself",
  words.CAPABILITY_ORDER.every((capability) => !words.CAPABILITY_WORDS[capability].includes(capability)),
  JSON.stringify(words.CAPABILITY_WORDS));

check("the capability order matches the orchestrator's",
  words.CAPABILITY_ORDER.join(",") === "read,action,write,execute",
  words.CAPABILITY_ORDER.join(","));

const startup = words.SETTING_GROUPS.flatMap((group) => group.items)
  .find((item) => item.key === "browser.startup");
check("browser startup is a closed choice",
  startup?.kind === "choice"
    && startup.choices.map((choice) => choice.value).join(",") === "on_demand,manual",
  JSON.stringify(startup));
check("browser startup values read as choices rather than raw keys",
  words.settingWords("browser.startup", "manual") ===
    "When no browser is connected: I will start it myself",
  words.settingWords("browser.startup", "manual"));

let failed = 0;
for (const [what, ok, detail] of checks) {
  if (!ok) failed += 1;
  console.log(`${ok ? "PASS" : "FAIL"}  ${what}${ok || !detail ? "" : `\n        ${detail}`}`);
}
console.log(`\npolicy grammar ${failed ? `FAILED (${failed})` : "ok: the readback matches what the matcher does"}`);
process.exit(failed ? 1 : 0);
