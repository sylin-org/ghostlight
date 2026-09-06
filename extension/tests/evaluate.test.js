"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const vm = require("node:vm");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const scriptEvaluator = require("../lib/script-evaluator.js");
const engineApi = require("../lib/engine.js");

function fakeSend(scripted) {
  const calls = [];
  const send = async (method, params) => {
    calls.push({ method, params });
    if (scripted.length === 0) throw new Error("unexpected extra evaluation");
    return scripted.shift();
  };
  return { calls, send };
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

function executingSend() {
  const context = vm.createContext({ effects: 0 });
  const calls = [];
  const send = async (method, params) => {
    assert.equal(method, "Runtime.evaluate");
    calls.push(params.expression);
    try {
      // Node's VM has no REPL mode. Supply an async context for a leading await expression;
      // the separate Chromium journey verifies the real REPL completion and await behavior.
      const expression = params.expression.startsWith("await ") ? `(async () => ${params.expression})()` : params.expression;
      return value(await vm.runInContext(expression, context));
    } catch (error) {
      return failure(error.stack ?? String(error), error.name);
    }
  };
  return { context, calls, send };
}

test("an expression evaluates with repl-grade debugger flags and returns its value", async () => {
  const { calls, send } = fakeSend([value(4)]);
  const result = await scriptEvaluator.evaluate(send, "2 + 2", 1000);
  assert.equal(result, 4);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "Runtime.evaluate");
  assert.deepEqual(calls[0].params, {
    expression: "2 + 2",
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
    replMode: true
  });
});

test("a missing result value is returned as null", async () => {
  const { send } = fakeSend([{ result: {} }]);
  assert.equal(await scriptEvaluator.evaluate(send, "undefined", 1000), null);
});

test("top-level await reaches the evaluator unchanged", async () => {
  const { calls, send } = fakeSend([value("late")]);
  const script = "await new Promise((resolve) => setTimeout(() => resolve('late'), 1))";
  assert.equal(await scriptEvaluator.evaluate(send, script, 1000), "late");
  assert.equal(calls[0].params.expression, script);
});

test("a bare top-level return selects an async function before its only execution", async () => {
  const { calls, send } = fakeSend([value("returned")]);
  const result = await scriptEvaluator.evaluate(send, "return 'returned';", 1000);
  assert.equal(result, "returned");
  assert.equal(calls.length, 1);
  assert.equal(
    calls[0].params.expression,
    "await (async () => {\nreturn 'returned';\n})()"
  );
});

test("a selected bare-return form reports runtime failure as uncertain", async () => {
  const { send } = fakeSend([failure("ReferenceError: missing is not defined", "ReferenceError")]);
  await assert.rejects(
    scriptEvaluator.evaluate(send, "return missing;", 1000),
    (error) => error.effectUnknown === true && error.code === "primitive_failed"
  );
});

test("a runtime failure is bounded, useful, and uncertain", async () => {
  const { send } = fakeSend([failure("ReferenceError: missing is not defined", "ReferenceError")]);
  await assert.rejects(
    scriptEvaluator.evaluate(send, "missing()", 1000),
    (error) => {
      assert.match(error.message, /ReferenceError: missing is not defined/);
      assert.equal(error.effectUnknown, true);
      assert.equal(error.code, "primitive_failed");
      return true;
    }
  );
});

test("a pure parse failure refuses decisively without inventing an effect", async () => {
  const { calls, send } = fakeSend([]);
  await assert.rejects(
    scriptEvaluator.evaluate(send, "const x = ();", 1000),
    (error) => {
      assert.match(error.message, /Unexpected token/);
      assert.match(error.message, /\(1:11\)$/, "coordinates refer to the supplied script");
      assert.equal(error.effectUnknown, false);
      assert.equal(error.code, "invalid_script");
      return true;
    }
  );
  assert.equal(calls.length, 0, "syntax rejection happens before sending any page code");
});

test("failure descriptions are bounded by the caller's budget", async () => {
  const long = `ReferenceError: ${"x".repeat(500)} is not defined`;
  const { send } = fakeSend([failure(long, "ReferenceError")]);
  await assert.rejects(
    scriptEvaluator.evaluate(send, "missing()", 40),
    (error) => error.message.length === 40 && error.message === long.slice(0, 40)
  );
});

for (const errorExpression of [
  "new Error('Illegal return statement')",
  "new SyntaxError('runtime-thrown exception')",
  "new Error('SyntaxError: forged prefix')",
  "'Illegal return statement'"
]) {
  test(`runtime ${errorExpression} neither replays effects nor claims no effects`, async () => {
    const { context, calls, send } = executingSend();
    let reported;
    try {
      await scriptEvaluator.evaluate(send, `effects += 1; throw ${errorExpression};`, 1000);
    } catch (error) { reported = error; }
    assert.equal(context.effects, 1, "the requested effect happens only once");
    assert.equal(calls.length, 1, "an exception must not trigger another execution");
    assert.equal(reported?.effectUnknown, true, "runtime effects cannot be ruled out");
    assert.equal(reported?.code, "primitive_failed");
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
    const { calls, send } = executingSend();
    await scriptEvaluator.evaluate(send, script, 1000);
    assert.deepEqual(calls, [script]);
  }
});

test("a return inside top-level control flow and await selects one effectful function", async () => {
  const { context, calls, send } = executingSend();
  const result = await scriptEvaluator.evaluate(send,
    "await Promise.resolve(); effects += 1; if (effects) { return { answer: effects }; }", 1000);
  assert.equal(result.answer, 1);
  assert.equal(context.effects, 1);
  assert.equal(calls.length, 1);
});

test("a bare return with a later runtime syntax exception remains uncertain", async () => {
  const { context, calls, send } = executingSend();
  await assert.rejects(scriptEvaluator.evaluate(send,
    "effects += 1; await Promise.resolve(); throw new SyntaxError('Illegal return statement'); return 4;", 1000),
  (error) => error.effectUnknown === true);
  assert.equal(context.effects, 1);
  assert.equal(calls.length, 1);
});

test("invalid source including wrapper escapes cannot dispatch earlier valid effects", async () => {
  for (const script of ["effects += 1; const x = ();", "} effects += 1; { return 4;",
    "})(), effects += 1, await (async () => { return 4;", "'use strict'; with ({}) { return 4; }"]) {
    const { context, calls, send } = executingSend();
    await assert.rejects(scriptEvaluator.evaluate(send, script, 1000),
      (error) => error.code === "invalid_script" && error.effectUnknown === false);
    assert.equal(context.effects, 0);
    assert.equal(calls.length, 0);
  }
});

test("REPL resource declarations and hashbangs remain valid during form selection", async () => {
  for (const script of ["using resource = { [Symbol.dispose]() {} }; 4",
    "await using resource = { async [Symbol.asyncDispose]() {} }; 4", "#!/usr/bin/env node\n4"]) {
    const { calls, send } = fakeSend([value(4)]);
    assert.equal(await scriptEvaluator.evaluate(send, script, 1000), 4);
    assert.equal(calls[0].params.expression, script);
  }
  const { calls, send } = executingSend();
  assert.equal(await scriptEvaluator.evaluate(send, "#!/usr/bin/env node\nreturn 4;", 1000), 4);
  assert.equal(calls.length, 1);
});

test("a lost evaluation reply is uncertain and is never retried", async () => {
  let attempts = 0;
  await assert.rejects(scriptEvaluator.evaluate(async () => {
    attempts += 1;
    throw new Error("connection lost");
  }, "effects += 1", 1000), (error) => error.effectUnknown === true);
  assert.equal(attempts, 1);
});

test("a runtime syntax exception stays non-replayable after operation-engine recovery", async () => {
  const { context, send } = executingSend();
  let stored;
  const persistence = { load: async () => stored, save: async (next) => { stored = structuredClone(next); } };
  const engine = engineApi.create(persistence);
  await engine.activate("service_script_test");
  const operation = () => scriptEvaluator.evaluate(send, "effects += 1; throw new SyntaxError('runtime')", 1000);
  await assert.rejects(engine.execute("script_test", operation));
  assert.equal(stored.records[0].phase, "uncertain");
  const recovered = engineApi.create(persistence);
  await recovered.activate("service_script_test");
  await assert.rejects(recovered.execute("script_test", operation),
    (error) => error.code === "operation_result_unavailable" && error.effectUnknown === true);
  assert.equal(context.effects, 1);
});

test("the actual worker import list loads its packaged parser before the evaluator", async () => {
  const extension = join(__dirname, "..");
  const context = vm.createContext({});
  context.importScripts = (...paths) => {
    for (const path of paths) vm.runInContext(readFileSync(join(extension, path), "utf8"), context, { filename: path });
  };
  vm.runInContext(readFileSync(join(extension, "service-worker.js"), "utf8").split("\n")[0], context);
  assert.equal(context.acorn.version, "8.18.0");
  const { calls, send } = executingSend();
  assert.equal(await context.GhostlightScriptEvaluator.evaluate(send, "return 4;", 1000), 4);
  assert.equal(calls.length, 1);
});
