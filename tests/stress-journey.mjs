// Live Chromium stress tests for the visual settlement sensor (ADR-0173).
// Exercises GhostlightSensor.settleVisual against real Chromium rendering, layout,
// CSS animations, infinite spinners, and concurrent multi-target workloads.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readDevToolsPort, removeBrowserScratch, waitForChromiumExit } from "./lib/chromium.mjs";

const root = resolve(import.meta.dirname, "..");
const scratchRoot = join(root, ".tmp");
mkdirSync(scratchRoot, { recursive: true });
const scratch = mkdtempSync(join(scratchRoot, "frame-browser-stress-"));
const browser = process.env.GHOSTLIGHT_TEST_BROWSER || join(root, ".tmp/chrome-testing/chrome-win64/chrome.exe");

console.log("Starting Chromium for Testing at:", browser);
const child = spawn(
  browser,
  [
    "--remote-debugging-port=0",
    `--user-data-dir=${scratch}`,
    "--no-first-run",
    "--no-default-browser-check",
    "about:blank"
  ],
  { windowsHide: true, stdio: "ignore" }
);
child.on("error", (error) => {
  child.startError = error;
});

let socket;
let send;

try {
  const [port, endpoint] = await readDevToolsPort(scratch, child);
  console.log(`Connected to DevTools on port ${port}`);

  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((resolveOpen, reject) => {
    socket.addEventListener("open", resolveOpen, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });

  let next = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const response = JSON.parse(event.data);
    const request = pending.get(response.id);
    if (!request) return;
    pending.delete(response.id);
    clearTimeout(request.timer);
    if (response.error) request.reject(new Error(JSON.stringify(response.error)));
    else request.resolve(response.result);
  });

  send = (method, params = {}, sessionId) =>
    new Promise((resolveCall, reject) => {
      const id = ++next;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`Timeout: ${method}`));
      }, 15000);
      pending.set(id, { resolve: resolveCall, reject, timer });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });

  const { targetInfos } = await send("Target.getTargets");
  const target = targetInfos.find((info) => info.type === "page");
  assert.ok(target, "Must find initial page target");

  const { sessionId } = await send("Target.attachToTarget", {
    targetId: target.targetId,
    flatten: true
  });

  const evaluate = async (expression) => {
    const response = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sessionId);
    assert.equal(response.exceptionDetails, undefined, `Evaluation error: ${JSON.stringify(response.exceptionDetails)}`);
    return response.result.value;
  };

  // Inject sensor.js into the real Chromium page
  const sensorSource = readFileSync(join(root, "crates/orchestrator/src/page_runtime/sensor.js"), "utf8");
  await evaluate(sensorSource);

  const hasSensor = await evaluate("typeof GhostlightSensor !== 'undefined' && typeof GhostlightSensor.settleVisual === 'function'");
  assert.equal(hasSensor, true, "GhostlightSensor.settleVisual must be injected and available in real Chromium");
  console.log("PASS: GhostlightSensor injected into real Chromium.");

  // Test 1: Static page visual settle
  console.log("Running Test 1: Static document settlement in real Chromium...");
  await evaluate(`
    document.body.innerHTML = '<div id="content" style="padding:20px;font-size:16px;">Static Content Ready</div>';
  `);
  const staticResult = await evaluate(`
    GhostlightSensor.settleVisual(document.documentElement, { maxWaitMs: 500, quietMs: 40 })
  `);
  assert.equal(staticResult.settled, true, "Static document must settle");
  assert.ok(staticResult.elapsed_ms >= 30, `Elapsed ms must cover quiet window: got ${staticResult.elapsed_ms}`);
  console.log(`PASS: Static document settled in ${staticResult.elapsed_ms}ms.`);

  // Test 2: Real CSS finite keyframe animation
  console.log("Running Test 2: Finite CSS animation settlement...");
  await evaluate(`
    const style = document.createElement('style');
    style.textContent = \`
      @keyframes slideIn {
        from { transform: translateX(0px); opacity: 0.2; }
        to { transform: translateX(150px); opacity: 1; }
      }
      .animating {
        width: 100px;
        height: 50px;
        background: blue;
        animation: slideIn 120ms ease-out forwards;
      }
    \`;
    document.head.appendChild(style);
    document.body.innerHTML = '<div id="box" class="animating">Box</div>';
  `);
  const animResult = await evaluate(`
    GhostlightSensor.settleVisual(document.getElementById("box"), { maxWaitMs: 1500, quietMs: 40 })
  `);
  assert.equal(animResult.settled, true, "Finite animation must settle after completing");
  assert.ok(animResult.elapsed_ms >= 140, `Must wait for 120ms animation + 40ms quiet window: got ${animResult.elapsed_ms}`);
  console.log(`PASS: Finite animation settled in ${animResult.elapsed_ms}ms (animation ended cleanly).`);

  // Test 3: Real CSS infinite spinner immunity
  console.log("Running Test 3: Infinite animation spinner immunity...");
  await evaluate(`
    const spinnerStyle = document.createElement('style');
    spinnerStyle.textContent = \`
      @keyframes spin {
        from { transform: rotate(0deg); }
        to { transform: rotate(360deg); }
      }
      .spinner {
        width: 30px;
        height: 30px;
        border: 4px solid red;
        border-top-color: transparent;
        border-radius: 50%;
        animation: spin 300ms linear infinite;
      }
    \`;
    document.head.appendChild(spinnerStyle);
    document.body.innerHTML = '<div class="spinner"></div><div id="text">Loaded Content</div>';
  `);
  const spinnerResult = await evaluate(`
    GhostlightSensor.settleVisual(document.documentElement, { maxWaitMs: 500, quietMs: 40 })
  `);
  assert.equal(spinnerResult.settled, true, "Infinite spinner must NOT deadlock visual settle");
  assert.ok(spinnerResult.elapsed_ms < 400, `Must settle promptly without waiting for infinite spinner: got ${spinnerResult.elapsed_ms}ms`);
  console.log(`PASS: Infinite spinner bypassed successfully in ${spinnerResult.elapsed_ms}ms.`);

  // Test 4: Continuous layout shifting timeout
  console.log("Running Test 4: Continuous layout shift timeout...");
  await evaluate(`
    document.body.innerHTML = '<div id="shifter" style="width:50px;height:50px;background:green;"></div>';
    let width = 50;
    window._shiftInterval = setInterval(() => {
      width = (width + 5) % 300;
      const el = document.getElementById("shifter");
      if (el) el.style.width = width + "px";
    }, 15);
  `);
  const timeoutResult = await evaluate(`
    GhostlightSensor.settleVisual(document.getElementById("shifter"), { maxWaitMs: 150, quietMs: 50 })
  `);
  assert.equal(timeoutResult.settled, false, "Continuously shifting element must report settled: false");
  assert.ok(timeoutResult.elapsed_ms >= 120, `Elapsed ms must reach timeout budget: got ${timeoutResult.elapsed_ms}`);
  await evaluate("clearInterval(window._shiftInterval)");
  console.log(`PASS: Shifting element correctly timed out at ${timeoutResult.elapsed_ms}ms.`);

  // Test 5: Multi-element concurrent visual settle stress
  console.log("Running Test 5: Multi-element concurrent visual settle stress (20 parallel targets)...");
  await evaluate(`
    document.body.innerHTML = '';
    const styleMulti = document.createElement('style');
    styleMulti.textContent = \`
      @keyframes pulse {
        0% { transform: scale(1); }
        50% { transform: scale(1.1); }
        100% { transform: scale(1); }
      }
    \`;
    document.head.appendChild(styleMulti);
    for (let i = 0; i < 20; i++) {
      const el = document.createElement('div');
      el.id = 'target-' + i;
      el.textContent = 'Target ' + i;
      const duration = 40 + (i * 5); // 40ms to 135ms
      el.style.animation = 'pulse ' + duration + 'ms ease-out forwards';
      document.body.appendChild(el);
    }
  `);

  const multiResult = await evaluate(`
    Promise.all(Array.from({ length: 20 }, (_, i) => {
      const el = document.getElementById('target-' + i);
      return GhostlightSensor.settleVisual(el, { maxWaitMs: 800, quietMs: 35 });
    }))
  `);
  assert.equal(multiResult.length, 20, "All 20 concurrent targets must return results");
  for (let i = 0; i < 20; i++) {
    assert.equal(multiResult[i].settled, true, `Target ${i} must settle cleanly`);
    assert.ok(multiResult[i].elapsed_ms > 0, `Target ${i} elapsed time must be positive`);
  }
  console.log(`PASS: All 20 concurrent visual settlements completed successfully.`);

  console.log("\nALL REAL CHROMIUM STRESS TESTS PASSED CLEANLY!");
} finally {
  if (socket) {
    try {
      socket.close();
    } catch (_) {}
  }
  if (child && child.pid) {
    try {
      child.kill();
      await waitForChromiumExit(child, 5000);
    } catch (_) {}
  }
  try {
    await removeBrowserScratch(scratch, scratchRoot, "frame-browser-", { timeoutMs: 5000 });
  } catch (_) {}
}
