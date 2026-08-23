"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const scriptEvaluator = require("../lib/script-evaluator.js");

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

test("a bare top-level return retries exactly once inside an async function", async () => {
  const { calls, send } = fakeSend([
    failure("SyntaxError: Illegal return statement", "SyntaxError"),
    value("returned")
  ]);
  const result = await scriptEvaluator.evaluate(send, "return 'returned';", 1000);
  assert.equal(result, "returned");
  assert.equal(calls.length, 2);
  assert.equal(
    calls[1].params.expression,
    "(async () => {\nreturn 'returned';\n})()"
  );
});

test("a bare-return retry that still fails reports the wrapped failure truthfully", async () => {
  const { send } = fakeSend([
    failure("SyntaxError: Illegal return statement", "SyntaxError"),
    failure("ReferenceError: missing is not defined", "ReferenceError")
  ]);
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
  const { calls, send } = fakeSend([failure("SyntaxError: Unexpected token ')'", "SyntaxError")]);
  await assert.rejects(
    scriptEvaluator.evaluate(send, "const x = ();", 1000),
    (error) => {
      assert.match(error.message, /Unexpected token/);
      assert.equal(error.effectUnknown, false);
      assert.equal(error.code, "invalid_script");
      return true;
    }
  );
  assert.equal(calls.length, 1, "only the bare-return case may retry");
});

test("failure descriptions are bounded by the caller's budget", async () => {
  const long = `ReferenceError: ${"x".repeat(500)} is not defined`;
  const { send } = fakeSend([failure(long, "ReferenceError")]);
  await assert.rejects(
    scriptEvaluator.evaluate(send, "missing()", 40),
    (error) => error.message.length === 40 && error.message === long.slice(0, 40)
  );
});
