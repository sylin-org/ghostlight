"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const api = require("../../crates/orchestrator/src/glass/sensor.js");

function mockDocument() {
  const listeners = [];
  const root = {
    nodeType: 1,
    tagName: "HTML",
    childNodes: [],
    append(...nodes) {
      this.childNodes.push(...nodes);
      for (const listener of listeners) listener();
    }
  };
  return {
    nodeType: 9,
    documentElement: root,
    body: root,
    _listeners: listeners,
    _triggerMutation() {
      for (const listener of listeners) listener();
    }
  };
}

function mockObserverClass(doc) {
  return class MockMutationObserver {
    constructor(callback) {
      this.callback = callback;
      this.active = true;
    }
    observe(_target, _options) {
      doc._listeners.push(this.callback);
    }
    disconnect() {
      this.active = false;
      const index = doc._listeners.indexOf(this.callback);
      if (index !== -1) doc._listeners.splice(index, 1);
    }
  };
}

test("sensor returns immediately on tick 0 when already satisfied", async () => {
  let samples = 0;
  let observersCreated = 0;
  const doc = mockDocument();
  class TrackedObserver extends mockObserverClass(doc) {
    constructor(cb) {
      super(cb);
      observersCreated++;
    }
  }

  const result = await api.settle(
    () => {
      samples++;
      return { text: "Hello world", words: 2 };
    },
    (res) => res.words > 0,
    { document: doc, MutationObserver: TrackedObserver }
  );

  assert.equal(result.words, 2);
  assert.equal(samples, 1, "only initial sample was taken");
  assert.equal(observersCreated, 0, "no MutationObserver was allocated on hot path");
});

test("sensor returns initial candidate immediately if document is unavailable", async () => {
  const result = await api.settle(
    () => ({ words: 0 }),
    (res) => res.words > 0,
    { document: null }
  );
  assert.equal(result.words, 0);
});

test("sensor settles on delayed hydration when content appears", async () => {
  const doc = mockDocument();
  let pageContent = "";

  const promise = api.settle(
    () => ({ text: pageContent, words: pageContent ? pageContent.split(/\s+/).length : 0 }),
    (res) => res.words > 0,
    {
      document: doc,
      MutationObserver: mockObserverClass(doc),
      maxWaitMs: 500,
      quietMs: 30
    }
  );

  // Simulate SPA mounting after 40ms
  setTimeout(() => {
    pageContent = "Rendered Single Page Application";
    doc._triggerMutation();
  }, 40);

  const result = await promise;
  assert.equal(result.words, 4);
  assert.equal(result.text, "Rendered Single Page Application");
  assert.equal(doc._listeners.length, 0, "observer disconnected after resolution");
});

test("sensor multi-stage hydration captures state after quiet window", async () => {
  const doc = mockDocument();
  let pageContent = "";

  const promise = api.settle(
    () => ({ text: pageContent, count: pageContent ? 1 : 0 }),
    (res) => res.count > 0,
    {
      document: doc,
      MutationObserver: mockObserverClass(doc),
      maxWaitMs: 500,
      quietMs: 40
    }
  );

  // Stage 1 at 20ms: initial partial text
  setTimeout(() => {
    pageContent = "Loading";
    doc._triggerMutation();
  }, 20);

  // Stage 2 at 40ms (before quietMs 40ms expires from stage 1): final text arrives
  setTimeout(() => {
    pageContent = "Dashboard ready with 5 accounts";
    doc._triggerMutation();
  }, 40);

  const result = await promise;
  assert.equal(result.text, "Dashboard ready with 5 accounts");
  assert.equal(doc._listeners.length, 0, "observer cleaned up");
});

test("sensor times out cleanly on genuinely empty document", async () => {
  const doc = mockDocument();
  const started = Date.now();

  const result = await api.settle(
    () => ({ words: 0, text: "" }),
    (res) => res.words > 0,
    {
      document: doc,
      MutationObserver: mockObserverClass(doc),
      maxWaitMs: 80,
      quietMs: 20
    }
  );

  const elapsed = Date.now() - started;
  assert.equal(result.words, 0);
  assert.ok(elapsed >= 70, `waited bounded duration (elapsed: ${elapsed}ms)`);
  assert.equal(doc._listeners.length, 0, "observer cleaned up on timeout");
});

test("sensor is immune to continuous mutator and terminates at timeout", async () => {
  const doc = mockDocument();
  let ticker = 0;

  // Infinite ticker mutating every 10ms
  const interval = setInterval(() => {
    ticker++;
    doc._triggerMutation();
  }, 10);

  try {
    const result = await api.settle(
      () => ({ matches: [] }),
      (res) => res.matches.length > 0,
      {
        document: doc,
        MutationObserver: mockObserverClass(doc),
        maxWaitMs: 90,
        quietMs: 20
      }
    );

    assert.equal(result.matches.length, 0);
    assert.equal(doc._listeners.length, 0, "observer disconnected despite continuous ticker");
  } finally {
    clearInterval(interval);
  }
});

test("sensor is immune to continuous mutator and resolves when satisfied", async () => {
  const doc = mockDocument();
  let ticker = 0;
  let items = [];

  // Infinite ticker mutating every 10ms
  const interval = setInterval(() => {
    ticker++;
    doc._triggerMutation();
  }, 10);

  // At 30ms, the desired item appears
  setTimeout(() => {
    items = ["Target Control"];
    doc._triggerMutation();
  }, 30);

  try {
    const result = await api.settle(
      () => ({ items }),
      (res) => res.items.length > 0,
      {
        document: doc,
        MutationObserver: mockObserverClass(doc),
        maxWaitMs: 300,
        quietMs: 30
      }
    );

    assert.deepEqual(result.items, ["Target Control"]);
    assert.equal(doc._listeners.length, 0, "observer disconnected cleanly");
  } finally {
    clearInterval(interval);
  }
});

test("sensor propagates sample error and cleans up resources", async () => {
  const doc = mockDocument();
  let calls = 0;

  const promise = api.settle(
    () => {
      calls++;
      if (calls > 1) throw new Error("stale DOM pointer");
      return { words: 0 };
    },
    (res) => res.words > 0,
    {
      document: doc,
      MutationObserver: mockObserverClass(doc),
      maxWaitMs: 200,
      quietMs: 20
    }
  );

  setTimeout(() => {
    doc._triggerMutation();
  }, 20);

  await assert.rejects(promise, { message: "stale DOM pointer" });
  assert.equal(doc._listeners.length, 0, "observer cleaned up on error");
});

test("sensor falls back to interval polling when MutationObserver is absent", async () => {
  const doc = mockDocument();
  let content = "";

  const promise = api.settle(
    () => ({ text: content, count: content ? 1 : 0 }),
    (res) => res.count > 0,
    {
      document: doc,
      MutationObserver: null,
      maxWaitMs: 300,
      quietMs: 20
    }
  );

  setTimeout(() => {
    content = "polled content";
  }, 60);

  const result = await promise;
  assert.equal(result.text, "polled content");
});

test("settleVisual settles on static document with quiet window", async () => {
  const doc = mockDocument();
  doc.getAnimations = () => [];
  const result = await api.settleVisual(doc.documentElement, {
    document: doc,
    maxWaitMs: 300,
    quietMs: 40
  });
  assert.equal(result.settled, true);
  assert.ok(result.elapsed_ms >= 30, `expected at least 30ms quiet window, got ${result.elapsed_ms}`);
});

test("settleVisual waits for running finite animation to finish", async () => {
  const doc = mockDocument();
  let animationRunning = true;
  doc.getAnimations = () => {
    return animationRunning ? [{ playState: "running", effect: { getTiming: () => ({ iterations: 1 }) } }] : [];
  };

  setTimeout(() => {
    animationRunning = false;
  }, 50);

  const result = await api.settleVisual(doc.documentElement, {
    document: doc,
    maxWaitMs: 500,
    quietMs: 30
  });

  assert.equal(result.settled, true);
  assert.ok(result.elapsed_ms >= 70, `expected settle after animation ends, got ${result.elapsed_ms}`);
});

test("settleVisual ignores infinite animations (spinners)", async () => {
  const doc = mockDocument();
  doc.getAnimations = () => [
    { playState: "running", effect: { getTiming: () => ({ iterations: Infinity }) } }
  ];

  const result = await api.settleVisual(doc.documentElement, {
    document: doc,
    maxWaitMs: 300,
    quietMs: 40
  });

  assert.equal(result.settled, true, "infinite animations must not block visual settle");
});

test("settleVisual times out when geometry continually shifts", async () => {
  const doc = mockDocument();
  doc.getAnimations = () => [];
  let y = 0;
  const target = {
    getBoundingClientRect() {
      y += 10;
      return { x: 0, y, width: 100, height: 100 };
    }
  };

  const result = await api.settleVisual(target, {
    document: doc,
    maxWaitMs: 120,
    quietMs: 50
  });

  assert.equal(result.settled, false, "shifting geometry should fail to settle within maxWaitMs");
  assert.ok(result.elapsed_ms >= 100, `expected timeout after ~120ms, got ${result.elapsed_ms}`);
});

test("settleVisual stress: staggered finite animations alongside infinite spinners", async () => {
  const doc = mockDocument();
  const start = Date.now();
  // 20 finite animations ending at various times up to 80ms, plus 5 permanent spinners
  doc.getAnimations = () => {
    const elapsed = Date.now() - start;
    const anims = [
      // 5 infinite spinners
      { playState: "running", effect: { getTiming: () => ({ iterations: Infinity }) } },
      { playState: "running", effect: { getTiming: () => ({ iterations: Infinity }) } },
      { playState: "running", effect: { getTiming: () => ({ iterations: Infinity }) } },
      { playState: "running", effect: { getTiming: () => ({ iterations: Infinity }) } },
      { playState: "running", effect: { getTiming: () => ({ iterations: Infinity }) } }
    ];
    for (let i = 1; i <= 20; i++) {
      const finishTime = i * 4; // 4ms to 80ms
      if (elapsed < finishTime) {
        anims.push({
          playState: "running",
          effect: { getTiming: () => ({ iterations: 1, duration: finishTime }) }
        });
      }
    }
    return anims;
  };

  const result = await api.settleVisual(doc.documentElement, {
    document: doc,
    maxWaitMs: 600,
    quietMs: 40
  });

  assert.equal(result.settled, true, "must settle after last finite animation ends");
  assert.ok(result.elapsed_ms >= 110, `expected settle after ~80ms animations + 40ms quiet, got ${result.elapsed_ms}`);
});

test("settleVisual stress: oscillating geometry that stabilizes before deadline", async () => {
  const doc = mockDocument();
  doc.getAnimations = () => [];
  const start = Date.now();
  let toggle = 0;
  const target = {
    getBoundingClientRect() {
      const elapsed = Date.now() - start;
      // Oscillate for the first 60ms, then lock position
      if (elapsed < 60) {
        toggle = 100 - toggle;
        return { x: toggle, y: 50, width: 200, height: 100 };
      }
      return { x: 50, y: 50, width: 200, height: 100 };
    }
  };

  const result = await api.settleVisual(target, {
    document: doc,
    maxWaitMs: 500,
    quietMs: 40
  });

  assert.equal(result.settled, true, "must settle after geometry stabilizes");
  assert.ok(result.elapsed_ms >= 90, `expected settle after 60ms shifts + 40ms quiet, got ${result.elapsed_ms}`);
});

test("settleVisual stress: concurrent multi-target settlement", async () => {
  const doc = mockDocument();
  doc.getAnimations = () => [];

  // Launch 15 concurrent settleVisual instances on distinct elements with varying quiet windows
  const tasks = Array.from({ length: 15 }, (_, i) => {
    let callCount = 0;
    const target = {
      getBoundingClientRect() {
        callCount++;
        // Jitter first 2 calls
        const offset = callCount <= 2 ? i * 2 : 0;
        return { x: offset, y: 10, width: 100, height: 50 };
      }
    };
    return api.settleVisual(target, {
      document: doc,
      maxWaitMs: 400,
      quietMs: 25 + (i % 5) * 5
    });
  });

  const results = await Promise.all(tasks);
  assert.equal(results.length, 15);
  for (const [index, res] of results.entries()) {
    assert.equal(res.settled, true, `concurrent task ${index} must settle`);
    assert.ok(res.elapsed_ms > 0, `task ${index} elapsed_ms must be positive`);
  }
});


