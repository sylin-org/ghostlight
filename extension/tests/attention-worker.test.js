"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const vm = require("node:vm");
const { createHash } = require("node:crypto");
const chunksApi = require("../lib/chunks.js");
const shared = require("../lib/shared.js");
const topologyApi = require("../lib/topology.js");
const nativeInputApi = require("../lib/native-input.js");

function browserFixture({ grouped = true } = {}) {
  const effects = [];
  const windows = new Map([[20,{id:20,focused:true}],[21,{id:21,focused:false}]]);
  const tabs = new Map([
    [1, { id: 1, windowId: 20, active: true, url: "https://example.test/human-draft", status: "complete" }],
    [2, { id: 2, windowId: 20, active: false, url: "https://example.test/owned", status: "complete" }],
    [3, { id: 3, windowId: 21, active: true, url: "https://example.test/duplicate", status: "complete" }]
  ]);
  const groups = grouped ? [
    { id: 10, windowId: 20, title: "Ghostlight - fixture", collapsed: true },
    { id: 11, windowId: 21, title: "Ghostlight - fixture", collapsed: true }
  ] : [];
  let nextId = 4;
  const chrome = {
    storage: { session: { async get() { return {}; }, async set() {} } },
    tabs: {
      async get(id) { return { ...tabs.get(id) }; },
      async query(query) {
        return [...tabs.values()].filter(tab => query.windowId === undefined || tab.windowId === query.windowId);
      },
      async create(options) {
        effects.push(["create_tab", { ...options }]);
        const tab = { id: nextId++, status: "complete", ...options };
        tabs.set(tab.id, tab);
        if (options.active) for (const other of tabs.values()) {
          if (other.windowId === tab.windowId && other.id !== tab.id) other.active = false;
        }
        return { ...tab };
      },
      async update(id, options) {
        effects.push(["update_tab", id, { ...options }]);
        const tab = tabs.get(id);
        Object.assign(tab, options);
        if (options.active) for (const other of tabs.values()) {
          if (other.windowId === tab.windowId && other.id !== tab.id) other.active = false;
        }
        return { ...tab };
      },
      async remove(id) { effects.push(["remove_tab", id]); tabs.delete(id); },
      async move(id, options) { effects.push(["move_tab", id, options]); },
      async group(options) {
        effects.push(["group", options]);
        // Model Chromium's actual default: a new group goes in the focused window.
        const windowId = options.groupId !== undefined ? groups.find(group => group.id === options.groupId).windowId
          : options.createProperties?.windowId ?? [...windows.values()].find(window => window.focused)?.id;
        for(const id of options.tabIds) {
          if(tabs.get(id).windowId!==windowId) effects.push(["implicit_group_move",id,windowId]);
          tabs.get(id).windowId=windowId;
        }
        return options.groupId ?? 12;
      }
    },
    tabGroups: {
      async get(id) { return groups.find(group => group.id === id); },
      async query() { return groups; },
      async update(id, options) { effects.push(["update_group", id, { ...options }]); }
    },
    windows: {
      async get(id) { return { ...windows.get(id) }; },
      async update(id, options) { effects.push(["update_window", id, { ...options }]); return { id, ...options }; },
      async create(options) {
        effects.push(["create_window", { ...options }]);
        const tab = { id: nextId++, windowId: 30, active: true, url: options.url, status: "complete" };
        tabs.set(tab.id, tab);
        windows.set(30,{id:30,focused:options.focused});
        return { id: 30, tabs: [{ ...tab }] };
      }
    }
  };
  const topology = topologyApi.create(chrome, "test-topology");
  return { effects, tabs, windows, chrome, topology };
}

function workerFixture(options) {
  const fixture = browserFixture(options);
  const custody = [];
  const sandbox = {
    ...fixture, shared, BACKGROUND_EDIT_COMMANDS: new Set(["fill","type_text","type_focused"]), NATIVE_SURFACE_COMMANDS: new Set(["activate", "activate_modified", "activate_point", "activate_point_modified", "wheel_at", "hover", "hover_point", "press_key", "drag", "drag_points"]), cancelled: new Set(), navigationWatchers: new Map(), beforeUnloadAcceptors: new Map(),
    retainManagedDebugger: async id => { custody.push(id); },
    syncFormDiagnostics: async () => {}, ensureDebugger: async () => {},
    tabPreservationEnabled: async () => false,
    debuggerLifecycle: { unretain: async id => custody.push(["release", id]) },
    sendDebugger: async () => {}, detachDebugger: async () => {},
    waitForReady: async id => fixture.chrome.tabs.get(id),
    physicalTab: tab => tab,
    documents: { run: async (_tab, _scope, task) => task() }
  };
  sandbox.nativeInput=nativeInputApi.create(fixture.chrome,id=>fixture.topology.workspaceFor(id),()=>shared.attentionProtected("native_input"));
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  vm.createContext(sandbox);
  for (const name of ["dispatch", "openTab", "navigate", "navigateDiscardingBeforeUnload", "resizeWindow"]) {
    const body = source.match(new RegExp(`async function ${name}\\([^]*?\\n}`));
    assert.ok(body, name);
    vm.runInContext(body[0], sandbox);
  }
  let correlation = 0;
  const dispatch = (command, attention = "background") => sandbox.dispatch({
    correlation: `fixture-${++correlation}`, workspace: "workspace", attention, command
  });
  return { ...fixture, sandbox, dispatch, custody };
}

test("attention vocabulary defaults only omitted legacy requests and rejects unknown rules", () => {
  assert.equal(shared.browserAttention(undefined), "foreground");
  assert.equal(shared.browserAttention("background"), "background");
  for (const value of [null, "quiet", true, {}]) {
    assert.throws(() => shared.browserAttention(value), /unsupported browser attention rule/);
  }
});

test("background open preserves human tab, duplicate groups, and collapsed presentation", async () => {
  const { dispatch, tabs, effects, topology } = workerFixture();
  const before = { ...tabs.get(1) };
  const result = await dispatch({ command: "open_tab", url: "https://example.test/agent", reuse: "domain", group_title: "Ghostlight - fixture" });
  assert.equal(result.outcome, "tab_opened");
  assert.equal(result.reused, undefined);
  assert.deepEqual(tabs.get(1), before);
  assert.equal(topology.workspaceFor(1), null);
  assert.equal(topology.workspaceFor(result.tab.id), "workspace");
  assert.equal(tabs.get(3).windowId, 21);
  assert.equal(effects.filter(effect => effect[0] === "group").length, 1);
  assert.equal(effects.find(effect => effect[0] === "create_window")[1].focused, false);
  assert.equal(result.tab.windowId, 30);
  assert.equal(effects.some(effect => effect[0] === "update_group" && [10,11].includes(effect[1])), false);
  assert.equal(effects.some(effect => effect[0] === "move_tab" || effect[0] === "update_window"), false);
  assert.equal(Object.hasOwn(effects.find(effect => effect[0] === "update_group")[2], "collapsed"), false);
});

test("foreground domain reuse preserves the legacy explicit opt-out behavior", async () => {
  const { dispatch, topology, tabs, effects } = workerFixture();
  const result = await dispatch({ command: "open_tab", url: "https://example.test/next", reuse: "domain", group_title: "Ghostlight - fixture" }, "foreground");
  assert.equal(result.reused, true);
  assert.equal(result.tab.id, 1);
  assert.equal(topology.workspaceFor(1), "workspace");
  assert.equal(tabs.get(1).url, "https://example.test/next");
  assert.equal(effects.some(effect => effect[0] === "create_tab" || effect[0] === "create_window"), false);
  assert.equal(effects.some(effect => effect[0] === "update_window"), true);
});

test("repeated background opens leave the active human tab and group topology unchanged", async () => {
  const { dispatch, effects, tabs } = workerFixture();
  const results = await Promise.all([
    dispatch({ command: "open_tab", url: "https://other.test/one", reuse: "domain", group_title: "Ghostlight - fixture" }),
    dispatch({ command: "open_tab", url: "https://other.test/two", reuse: "domain", group_title: "Ghostlight - fixture" })
  ]);
  assert.notEqual(results[0].tab.id, results[1].tab.id);
  assert.equal(tabs.get(1).active, true);
  assert.equal(effects.filter(effect => effect[0] === "create_tab").every(effect => effect[1].active === false), true);
  assert.equal(effects.filter(effect => effect[0] === "group").length, 2);
  assert.equal(effects.some(effect => effect[0] === "move_tab" || effect[0] === "update_window"), false);
});

test("background first open creates an unfocused work window without changing human attention", async () => {
  const { dispatch, effects, tabs } = workerFixture({ grouped: false });
  const result = await dispatch({ command: "open_tab", url: "https://other.test/", reuse: "domain", group_title: "Ghostlight - fixture" });
  assert.equal(result.tab.windowId, 30);
  assert.equal(effects.find(effect => effect[0] === "create_window")[1].focused, false);
  assert.equal(effects.some(effect => effect[0] === "update_window" || effect[0] === "move_tab"), false);
  assert.equal(tabs.get(1).active, true);
});

test("background navigation and discard-before-unload never activate a tab", async () => {
  for (const command of ["navigate", "navigate_discarding_before_unload"]) {
    const { dispatch, effects, tabs, topology } = workerFixture();
    await topology.remember(2, "workspace");
    const result = await dispatch({ command, tab_id: 2, url: "https://example.test/next" });
    assert.equal(result.outcome, "navigated");
    assert.equal(tabs.get(1).active, true);
    assert.equal(tabs.get(2).active, false);
    assert.equal(tabs.get(2).url, "https://example.test/next");
    assert.equal(Object.hasOwn(effects.find(effect => effect[0] === "update_tab")[2], "active"), false);
  }
});

test("background focus refuses before tab, window, ownership, or debugger effects", async () => {
  const { dispatch, effects, custody, topology } = workerFixture();
  assert.deepEqual(await dispatch({ command: "focus_tab", tab_id: 2 }), { outcome: "attention_protected", reason: "focus" });
  assert.equal(effects.length, 0);
  assert.equal(custody.length, 0);
  assert.equal(topology.workspaceFor(2), null);
});

test("background resize refuses shared human windows before changing geometry", async () => {
  const { dispatch, effects, tabs } = workerFixture();
  assert.deepEqual(await dispatch({ command: "resize_window", tab_id: 2, width: 1000, height: 800 }),
    { outcome: "attention_protected", reason: "shared_window_resize" });
  assert.equal(effects.length, 0);
  assert.equal(tabs.get(1).active, true);
});

test("background resize accepts a dedicated controlled window without focusing it", async () => {
  const { dispatch, effects, topology, windows } = workerFixture();
  windows.get(20).focused=false;
  await topology.remember(1, "another-workspace");
  const result = await dispatch({ command: "resize_window", tab_id: 2, width: 1000, height: 800 });
  assert.equal(result.outcome, "window_resized");
  assert.equal(result.width, 1000);
  assert.equal(result.height, 800);
  assert.deepEqual(effects, [["update_window", 20, { width: 1000, height: 800 }]]);
  assert.deepEqual(Array.from(result.affected_tab_ids), [1, 2]);
});

test("background close protects an active shared window and its unowned successor", async () => {
  const { dispatch, effects, tabs, topology } = workerFixture();
  await topology.remember(1, "workspace");
  assert.deepEqual(await dispatch({ command: "close_tab", tab_id: 1 }),
    { outcome: "attention_protected", reason: "active_tab_close" });
  assert.equal(tabs.has(1), true);
  assert.equal(effects.length, 0);
});

test("background close refuses the final active controlled tab", async () => {
  const { dispatch, effects, tabs, topology } = workerFixture();
  await topology.remember(3, "workspace");
  assert.deepEqual(await dispatch({ command: "close_tab", tab_id: 3 }),
    { outcome: "attention_protected", reason: "active_tab_close" });
  assert.equal(tabs.has(3), true);
  assert.equal(effects.length, 0);
});

test("attention-preserved workspace release still drops ownership and debugger custody", async () => {
  const { dispatch, effects, tabs, topology, custody } = workerFixture();
  await topology.remember(3, "workspace");
  assert.deepEqual(await dispatch({ command: "close_tab", tab_id: 3, released: true }),
    { outcome: "attention_protected", reason: "active_tab_close" });
  assert.equal(tabs.has(3), true);
  assert.equal(topology.workspaceFor(3), null);
  assert.deepEqual(custody, [["release", 3]]);
  assert.equal(effects.length, 0);
});

test("background close accepts inactive controlled tabs and active owned-only neighbors", async () => {
  for (const target of [1, 2]) {
    const { dispatch, effects, tabs, topology, windows } = workerFixture();
    windows.get(20).focused=false;
    await topology.remember(1, "workspace");
    await topology.remember(2, "other-workspace");
    const result = await dispatch({ command: "close_tab", tab_id: target });
    assert.equal(result.outcome, "tab_closed");
    assert.equal(tabs.has(target), false);
    assert.deepEqual(effects, [["remove_tab", target]]);
  }
});

test("an explicit foreground reveal activates and focuses the exact tab", async () => {
  const { dispatch, effects, tabs } = workerFixture();
  const result = await dispatch({ command: "focus_tab", tab_id: 2 }, "foreground");
  assert.equal(result.outcome, "tab_focused");
  assert.equal(result.active, true);
  assert.equal(result.window_focused, true);
  assert.equal(tabs.get(1).active, false);
  assert.deepEqual(effects.map(effect => effect[0]), ["update_tab", "update_window"]);
});

test("foreground and background dispatch keep independent rules through document wrappers", async () => {
  const { dispatch, tabs } = workerFixture();
  const primitive = { command: "focus_tab", tab_id: 2 };
  const results = await Promise.allSettled([
    dispatch({ command: "in_documents", primitive, scope: {} }),
    dispatch(primitive, "foreground")
  ]);
  assert.equal(results[0].status, "fulfilled");
  assert.deepEqual(results[0].value, { outcome: "attention_protected", reason: "focus" });
  assert.equal(results[1].status, "fulfilled");
  assert.equal(tabs.get(2).active, true);
});

test("background open keeps an already-created effect unknown when grouping fails", async () => {
  const { dispatch, chrome, effects } = workerFixture();
  chrome.tabs.group = async () => { throw new Error("grouping failed after create"); };
  await assert.rejects(dispatch({ command: "open_tab", url: "https://other.test/", group_title: "Ghostlight - fixture" }),
    error => error.effectUnknown === true);
  assert.equal(effects.find(effect => effect[0] === "create_window")[1].focused, false);
});

test("production capability declaration includes the executed request envelope rule", () => {
  const source = readFileSync(join(__dirname, "../service-worker.js"), "utf8");
  const requestHandlers = source.match(/const REQUEST_HANDLERS = Object\.freeze\((\[[^]*?\n\])\);/);
  assert.ok(requestHandlers);
  const handlers = vm.runInNewContext(requestHandlers[1]);
  assert.deepEqual(shared.adapterCapabilities({}, handlers), [{ name: "browser_attention", revision: 1 }]);
  assert.match(source, /shared\.adapterCapabilities\(COMMAND_HANDLERS, \[\.\.\.PASSIVE_HANDLERS, \.\.\.REQUEST_HANDLERS\]\)/);
});


test("verified chunk delivery retains background protection and foreground reveal semantics", async () => {
  for (const attention of ["background", "foreground"]) {
    const { sandbox, effects, custody } = workerFixture();
    const request = { kind: "request", request: { correlation: `chunked-${attention}`, workspace: "workspace",
      attention, command: { command: "focus_tab", tab_id: 2 } } };
    const bytes = Buffer.from(JSON.stringify(request));
    const digest = createHash("sha256").update(bytes).digest("hex");
    const chunkSize = Math.ceil(bytes.length / 2);
    const frames = [0, 1].map(index => ({ kind: "command_chunk", transfer_id: `transfer-${attention}`,
      correlation: request.request.correlation, index, count: 2, total_bytes: bytes.length,
      sha256: digest, data: bytes.subarray(index * chunkSize, (index + 1) * chunkSize).toString("base64") }));
    const chunks = chunksApi.create({
      decodeBase64: value => new Uint8Array(Buffer.from(value, "base64")),
      decodeUtf8: value => new TextDecoder("utf-8", { fatal: true }).decode(value),
      sha256Hex: async value => createHash("sha256").update(value).digest("hex"),
      setTimer: () => 0, clearTimer: () => {}
    });
    const outcome = await new Promise((resolve, reject) => {
      for (const frame of frames) chunks.accept(frame,
        delivered => sandbox.dispatch(delivered.request).then(resolve, reject),
        (_correlation, reason) => reject(new Error(reason)));
    });
    if (attention === "background") {
      assert.deepEqual(outcome, { outcome: "attention_protected", reason: "focus" });
      assert.equal(effects.length, 0);
    } else {
      assert.equal(outcome.outcome, "tab_focused");
      assert.deepEqual(effects.map(effect => effect[0]), ["update_tab", "update_window"]);
    }
    assert.equal(custody.length, 0);
    assert.equal(chunks.stats().completed, 1);
  }
});

test("an unsupported packet attention rule fails before custody or physical effects", async () => {
  const { dispatch, effects, custody } = workerFixture();
  await assert.rejects(dispatch({ command: "focus_tab", tab_id: 2 }, "unknown"), /unsupported browser attention rule/);
  assert.equal(effects.length, 0);
  assert.equal(custody.length, 0);
});


test("background native packets refuse inactive shared or focused windows before custody or page effects", async () => {
  for (const name of ["activate", "activate_modified", "activate_point", "activate_point_modified", "wheel_at", "hover", "hover_point", "press_key", "drag", "drag_points"]) {
    const { dispatch, sandbox, custody, effects } = workerFixture();
    sandbox.pressKey = sandbox.activate = sandbox.activatePoint = sandbox.wheelAt = sandbox.hoverLocator
      = sandbox.hoverPoint = sandbox.dragLocators = sandbox.dragPoints = async () => { throw new Error("native effect forbidden"); };
    const result = await dispatch({command:name,tab_id:2});
    assert.equal(result.outcome,"attention_protected"); assert.equal(result.reason,"native_input");
    assert.deepEqual(custody,[]); assert.deepEqual(effects,[]);
  }
});

test("background native preparation selects only an owned tab in an unfocused own-only window", async () => {
  const { dispatch, sandbox, tabs, windows, topology, custody, effects } = workerFixture();
  windows.get(20).focused=false;
  await topology.remember(1,"other-workspace");
  sandbox.pressKey=async()=>({outcome:"key_pressed"});
  const result=await dispatch({command:"press_key",tab_id:2});
  assert.equal(result.outcome,"key_pressed"); assert.equal(tabs.get(2).active,true);
  assert.equal(tabs.get(1).active,false); assert.equal(windows.get(20).focused,false);
  assert.deepEqual(effects,[["update_tab",2,{active:true}]]); assert.deepEqual(custody,[2]);
});

test("already active background native target proceeds without changing selection or window focus", async () => {
  const { dispatch, sandbox, effects }=workerFixture();
  sandbox.pressKey=async()=>({outcome:"key_pressed"});
  const result=await dispatch({command:"press_key",tab_id:3});
  assert.equal(result.outcome,"key_pressed"); assert.deepEqual(effects,[]);
});

test("foreground native requests preserve ordinary key dispatch without quiet preparation", async () => {
  const {dispatch,sandbox,effects}=workerFixture();
  sandbox.pressKey=async()=>({outcome:"key_pressed"});
  assert.equal((await dispatch({command:"press_key",tab_id:2},"foreground")).outcome,"key_pressed");
  assert.deepEqual(effects,[]);
});


test("background open reuses current own-only group but treats copied group labels as presentation", async () => {
  const { dispatch, topology, effects }=workerFixture();
  await topology.remember(3,"other-workspace");
  const result=await dispatch({command:"open_tab",url:"https://example.test/new",group_title:"Ghostlight - fixture"});
  assert.equal(result.tab.windowId,21);
  assert.equal(effects.some(effect=>effect[0]==="create_window"),false);
  assert.equal(effects.find(effect=>effect[0]==="create_tab")[1].active,false);
  assert.equal(effects.some(effect=>effect[0]==="update_group" && effect[1]===10),false);
});

test("epoch custody reset preserves groups titles and physical tabs while forgetting every owner", async () => {
  const {topology,tabs,effects}=browserFixture();
  await topology.remember(1,"first"); await topology.remember(2,"second");
  await topology.forgetAll();
  assert.equal(topology.workspaceFor(1),null);assert.equal(topology.workspaceFor(2),null);
  assert.equal(tabs.size,3);assert.deepEqual(effects,[]);
});


test("background open skips focused own-only exact-title window for another unfocused own-only match", async()=>{
  const {dispatch,topology,effects}=workerFixture();
  for(const id of [1,2,3])await topology.remember(id,"current-authority");
  const result=await dispatch({command:"open_tab",url:"https://example.test/fresh",group_title:"Ghostlight - fixture"});
  assert.equal(result.tab.windowId,21);
  assert.equal(effects.some(effect=>effect[0]==="create_window"),false);
  assert.equal(effects.some(effect=>effect[0]==="update_group" && effect[1]===10),false);
});


test("background shell effects protect a focused own-only window while release still drops custody",async()=>{
  for(const command of ["resize_window","close_tab"]){
    const {dispatch,topology,tabs,effects,custody}=workerFixture();
    await topology.remember(1,"workspace");await topology.remember(2,"other-workspace");
    const result=await dispatch({command,tab_id:1,width:1000,height:800,released:command==="close_tab"});
    assert.equal(result.outcome,"attention_protected");
    assert.equal(result.reason,command==="close_tab"?"active_tab_close":"shared_window_resize");
    assert.deepEqual(effects,[]);assert.equal(tabs.has(1),true);
    if(command==="close_tab"){assert.equal(topology.workspaceFor(1),null);assert.deepEqual(custody,[["release",1]]);}
  }
});

test("background shell guards recheck target placement after window observation",async()=>{
  for(const command of ["resize_window","close_tab"]){
    const {dispatch,chrome,topology,tabs,effects,windows}=workerFixture();
    windows.get(20).focused=false;
    await topology.remember(1,"workspace");await topology.remember(2,"other-workspace");
    const get=chrome.tabs.get;let reads=0;
    chrome.tabs.get=async id=>{if(++reads===2)tabs.get(id).windowId=21;return get(id);};
    const result=await dispatch({command,tab_id:1,width:1000,height:800});
    assert.equal(result.outcome,"attention_protected");assert.deepEqual(effects,[]);
  }
});


test("new quiet group explicitly stays in its created work window instead of Chromium current window",async()=>{
  const {dispatch,effects,tabs}=workerFixture({grouped:false});
  const result=await dispatch({command:"open_tab",url:"https://example.test/fixture",group_title:"Ghostlight - fixture"});
  assert.equal(result.tab.windowId,30);assert.equal(tabs.get(result.tab.id).windowId,30);
  const grouped=effects.find(effect=>effect[0]==="group")[1];
  assert.deepEqual(grouped.createProperties,{windowId:30});
  assert.equal(effects.some(effect=>effect[0]==="implicit_group_move"),false);
});

test("person moving the new tab during quiet placement reads keeps actual placement and skips grouping",async()=>{
  const {dispatch,chrome,tabs,effects}=workerFixture({grouped:false});
  const get=chrome.tabs.get;let reads=0;
  chrome.tabs.get=async id=>{if(++reads===2 && tabs.get(id).windowId===30)tabs.get(id).windowId=20;return get(id);};
  const result=await dispatch({command:"open_tab",url:"https://example.test/moved",group_title:"Ghostlight - fixture"});
  assert.equal(result.tab.windowId,20);assert.equal(tabs.get(result.tab.id).windowId,20);
  assert.equal(effects.some(effect=>effect[0]==="group"||effect[0]==="implicit_group_move"),false);
});
