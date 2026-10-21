"use strict";

// The script effect truth regressions (docs/tasks/security-hardening/h3-script-effect-truth.md),
// through the `evaluate_main` primitive. The script's form is chosen by parsing it before anything
// is sent: a script that does not parse never reaches the page and its receipt is not dispatched.
// Once the evaluation is sent the receipt says it was dispatched, so a script the page threw on, or
// a reply that was lost, leaves its effect uncertain, and nothing is ever sent twice.

const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const Evaluate = require("../lib/evaluate.js");
const { fakeChrome, wire } = require("./p4-fakes.js");
const { execute } = require("./p4-chrome.js");

const TAB = 12;

/** The primitive over a fake browser whose `Runtime.evaluate` is `answer(params)`. */
function setup(answer) {
  const chrome = fakeChrome();
  chrome.handlers["Runtime.evaluate"] = (_target, params) => answer(params);
  const parts = wire(chrome);
  const { evaluateMain } = Evaluate.create({ lifecycle: parts.lifecycle, inventory: parts.inventory, sleep: async () => {} });
  const sent = () => chrome.calls.filter((call) => call.method === "Runtime.evaluate").map((call) => call.params);
  return { evaluateMain, sent };
}

const request = (script, maximum = 1000) => ({ tab_id: TAB, script, max_result_chars: maximum, settle_ms: 0, timeout_ms: 2000 });

/** Answers each evaluation with the next reply, and refuses an extra one. */
function scripted(...replies) {
  return async () => {
    if (replies.length === 0) throw new Error("unexpected extra evaluation");
    return replies.shift();
  };
}

function value(result) {
  return { result: { value: result } };
}

function failure(description, className) {
  return {
    exceptionDetails: {
      text: "Uncaught",
      exception: { className, description }
    }
  };
}

/** Runs each evaluation in one VM context, so a test can count the effects a script had. */
function executing() {
  const context = vm.createContext({ effects: 0 });
  const answer = async (params) => {
    try {
      // Node's VM has no REPL mode. Supply an async context for a leading await expression;
      // the separate Chromium journey verifies the real REPL completion and await behavior.
      const expression = params.expression.startsWith("await ") ? `(async () => ${params.expression})()` : params.expression;
      return value(await vm.runInContext(expression, context));
    } catch (error) {
      return failure(error.stack ?? String(error), error.name);
    }
  };
  return { context, answer };
}

test("an expression evaluates with repl-grade debugger flags and returns its value", async () => {
  const { evaluateMain, sent } = setup(scripted(value(4)));
  const result = await execute(evaluateMain, request("2 + 2"));
  assert.deepEqual(result.ok, { value: "4", truncated: false });
  assert.deepEqual(sent(), [{
    expression: "2 + 2",
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
    replMode: true
  }]);
});

test("a missing result value is returned as null", async () => {
  const { evaluateMain } = setup(scripted({ result: {} }));
  assert.deepEqual((await execute(evaluateMain, request("undefined"))).ok, { value: "null", truncated: false });
});

test("top-level await reaches the page unchanged", async () => {
  const { evaluateMain, sent } = setup(scripted(value("late")));
  const script = "await new Promise((resolve) => setTimeout(() => resolve('late'), 1))";
  assert.equal((await execute(evaluateMain, request(script))).ok.value, "\"late\"");
  assert.equal(sent()[0].expression, script);
});

test("a bare top-level return selects an async function before its only execution", async () => {
  const { evaluateMain, sent } = setup(scripted(value("returned")));
  const result = await execute(evaluateMain, request("return 'returned';"));
  assert.equal(result.ok.value, "\"returned\"");
  assert.deepEqual(sent().map((params) => params.expression), ["await (async () => {\nreturn 'returned';\n})()"]);
});

test("a selected bare-return form reports runtime failure as uncertain", async () => {
  const { evaluateMain } = setup(scripted(failure("ReferenceError: missing is not defined", "ReferenceError")));
  const result = await execute(evaluateMain, request("return missing;"));
  assert.match(result.ok.exception, /ReferenceError: missing is not defined/);
  assert.equal(result.dispatched, true);
});

test("a runtime failure is bounded, useful, and uncertain", async () => {
  const { evaluateMain } = setup(scripted(failure("ReferenceError: missing is not defined", "ReferenceError")));
  const result = await execute(evaluateMain, request("missing()"));
  assert.deepEqual(result.ok, { value: "", truncated: false, exception: "ReferenceError: missing is not defined" });
  assert.equal(result.dispatched, true);
});

test("a pure parse failure refuses decisively without inventing an effect", async () => {
  const { evaluateMain, sent } = setup(scripted());
  const result = await execute(evaluateMain, request("const x = ();"));
  assert.equal(result.failure, "invalid_request");
  assert.match(result.message, /Unexpected token/);
  assert.match(result.message, /\(1:11\)$/, "coordinates refer to the supplied script");
  assert.equal(result.dispatched, false);
  assert.equal(sent().length, 0, "syntax rejection happens before sending any page code");
});

test("failure descriptions are bounded by the caller's budget", async () => {
  const long = `ReferenceError: ${"x".repeat(500)} is not defined`;
  const { evaluateMain } = setup(scripted(failure(long, "ReferenceError")));
  assert.equal((await execute(evaluateMain, request("missing()", 40))).ok.exception, long.slice(0, 40));
});

for (const errorExpression of [
  "new Error('Illegal return statement')",
  "new SyntaxError('runtime-thrown exception')",
  "new Error('SyntaxError: forged prefix')",
  "'Illegal return statement'"
]) {
  test(`runtime ${errorExpression} neither replays effects nor claims no effects`, async () => {
    const { context, answer } = executing();
    const { evaluateMain, sent } = setup(answer);
    const result = await execute(evaluateMain, request(`effects += 1; throw ${errorExpression};`));
    assert.equal(context.effects, 1, "the requested effect happens only once");
    assert.equal(sent().length, 1, "an exception must not trigger another execution");
    assert.equal(result.dispatched, true, "runtime effects cannot be ruled out");
    assert.equal(typeof result.ok.exception, "string");
  });
}

test("nested function returns and return-like text keep the original REPL expression", async () => {
  for (const script of [
    "'return'; /* return 4 */ /return/.test('return')",
    "function answer() { return 4; } answer()",
    "(() => { return 4; })()",
    "({ get answer() { return 4; } }).answer",
    "new class { answer() { return 4; } }().answer()",
    "`return ${(() => { return 4; })()}`"
  ]) {
    const { answer } = executing();
    const { evaluateMain, sent } = setup(answer);
    await execute(evaluateMain, request(script));
    assert.deepEqual(sent().map((params) => params.expression), [script]);
  }
});

test("a return inside top-level control flow and await selects one effectful function", async () => {
  const { context, answer } = executing();
  const { evaluateMain, sent } = setup(answer);
  const result = await execute(evaluateMain,
    request("await Promise.resolve(); effects += 1; if (effects) { return { answer: effects }; }"));
  assert.equal(result.ok.value, "{\"answer\":1}");
  assert.equal(context.effects, 1);
  assert.equal(sent().length, 1);
});

test("a bare return with a later runtime syntax exception remains uncertain", async () => {
  const { context, answer } = executing();
  const { evaluateMain, sent } = setup(answer);
  const result = await execute(evaluateMain,
    request("effects += 1; await Promise.resolve(); throw new SyntaxError('Illegal return statement'); return 4;"));
  assert.equal(result.dispatched, true);
  assert.equal(typeof result.ok.exception, "string");
  assert.equal(context.effects, 1);
  assert.equal(sent().length, 1);
});

test("invalid source including wrapper escapes cannot dispatch earlier valid effects", async () => {
  for (const script of ["effects += 1; const x = ();", "} effects += 1; { return 4;",
    "})(), effects += 1, await (async () => { return 4;", "'use strict'; with ({}) { return 4; }"]) {
    const { context, answer } = executing();
    const { evaluateMain, sent } = setup(answer);
    const result = await execute(evaluateMain, request(script));
    assert.equal(result.failure, "invalid_request", script);
    assert.equal(result.dispatched, false, script);
    assert.equal(context.effects, 0);
    assert.equal(sent().length, 0);
  }
});

test("REPL resource declarations and hashbangs remain valid during form selection", async () => {
  for (const script of ["using resource = { [Symbol.dispose]() {} }; 4",
    "await using resource = { async [Symbol.asyncDispose]() {} }; 4", "#!/usr/bin/env node\n4"]) {
    const { evaluateMain, sent } = setup(scripted(value(4)));
    assert.equal((await execute(evaluateMain, request(script))).ok.value, "4");
    assert.equal(sent()[0].expression, script);
  }
  const { answer } = executing();
  const { evaluateMain, sent } = setup(answer);
  assert.equal((await execute(evaluateMain, request("#!/usr/bin/env node\nreturn 4;"))).ok.value, "4");
  assert.equal(sent().length, 1);
});

test("a lost evaluation reply is uncertain and is never retried", async () => {
  let attempts = 0;
  const { evaluateMain } = setup(async () => {
    attempts += 1;
    throw new Error("connection lost");
  });
  const result = await execute(evaluateMain, request("effects += 1"));
  assert.equal(result.ok, undefined);
  assert.equal(result.dispatched, true);
  assert.equal(attempts, 1);
});

test("the actual worker import list loads its packaged parser before the evaluator", async () => {
  const extension = join(__dirname, "..");
  const context = vm.createContext({ setTimeout, clearTimeout });
  context.importScripts = (...paths) => {
    for (const path of paths) vm.runInContext(readFileSync(join(extension, path), "utf8"), context, { filename: path });
  };
  const imports = readFileSync(join(extension, "worker.js"), "utf8").match(/importScripts\([^)]*\);/);
  assert.ok(imports, "worker.js imports its scripts in one call");
  vm.runInContext(imports[0], context);
  assert.equal(context.acorn.version, "8.18.0");
  const expressions = [];
  const lifecycle = {
    cdp: async (_target, _method, params) => {
      expressions.push(params.expression);
      return value(4);
    },
    acquire: async () => {},
    release: async () => {},
    currentDialog: () => null
  };
  const inventory = { snapshot: async () => ({}), checkGuard: () => {} };
  const { evaluateMain } = context.GhostlightEvaluate.create({ lifecycle, inventory });
  const ok = await evaluateMain.run(request("return 4;"), { dispatched: false, watch: () => {} });
  assert.equal(ok.value, "4");
  assert.deepEqual(expressions, ["await (async () => {\nreturn 4;\n})()"], "the parser chose the function form");
});
