"use strict";
const test=require("node:test");
const assert=require("node:assert/strict");
const vm=require("node:vm");
const {readFileSync}=require("node:fs");
const {join}=require("node:path");
const nativeApi=require("../lib/native-input.js");

function fixture(){
  const events={};
  const event=name=>({addListener(fn){events[name]=fn;}});
  const tabs=new Map([[1,{id:1,windowId:10,active:true}],[2,{id:2,windowId:10,active:false}],
    [3,{id:3,windowId:20,active:true}]]);
  const owners=new Map([[1,"first"],[2,"second"]]);
  const windows=new Map([[10,{id:10,focused:false}],[20,{id:20,focused:true}]]);
  const effects=[];
  const chrome={tabs:{
    onActivated:event("activated"),onCreated:event("created"),onRemoved:event("removed"),
    onAttached:event("attached"),onDetached:event("detached"),onMoved:event("moved"),
    async get(id){return {...tabs.get(id)};},
    async query({windowId}){return [...tabs.values()].filter(tab=>tab.windowId===windowId).map(tab=>({...tab}));},
    async update(id,options){
      const tab=tabs.get(id);for(const other of tabs.values())if(other.windowId===tab.windowId)other.active=false;
      Object.assign(tab,options);effects.push(["select",id]);events.activated({windowId:tab.windowId,tabId:id});return {...tab};
    }
  },windows:{onFocusChanged:event("focus"),async get(id){return {...windows.get(id)};}},
  debugger:{async sendCommand(target,method,params){effects.push([method,target.tabId,params.type??params.text]);}}
  };
  const coordinator=nativeApi.create(chrome,id=>owners.get(id)??null,()=>({outcome:"attention_protected",reason:"native_input"}));
  const sandbox={chrome,INPUT_DISPATCH_METHODS:new Set(["Input.dispatchMouseEvent","Input.dispatchKeyEvent","Input.dispatchDragEvent","Input.insertText"]),
    documents:{async input(){},async targetedInput(){}}};vm.createContext(sandbox);
  const source=readFileSync(join(__dirname,"../service-worker.js"),"utf8");
  vm.runInContext(source.match(/async function sendDebugger\([^]*?\n}/)[0],sandbox);
  const packet=(context,id,type)=>sandbox.sendDebugger({tabId:id,nativeContext:context},"Input.dispatchKeyEvent",{type,key:"x"});
  return {tabs,owners,windows,effects,chrome,coordinator,events,packet,sandbox};
}
function deferred(){let resolve;return {promise:new Promise(done=>{resolve=done;}),resolve:value=>resolve(value)};}

test("plural clients in one window keep selection, packets and confirmation inside FIFO",async()=>{
  const f=fixture(),entered=deferred(),confirm=deferred();
  const first=f.coordinator.run(1,{surface:true},async context=>{
    await f.packet(context,1,"keyDown");entered.resolve();await confirm.promise;
    await f.packet(context,1,"keyUp");return {outcome:"key_pressed"};
  });await entered.promise;
  const second=f.coordinator.run(2,{surface:true},async context=>{
    await f.packet(context,2,"keyDown");assert.equal(f.tabs.get(2).active,true);
    await f.packet(context,2,"keyUp");return {outcome:"key_pressed"};
  });await new Promise(done=>setImmediate(done));
  assert.deepEqual(f.effects,[["Input.dispatchKeyEvent",1,"keyDown"]]);
  confirm.resolve();await Promise.all([first,second]);
  assert.deepEqual(f.effects,[["Input.dispatchKeyEvent",1,"keyDown"],["Input.dispatchKeyEvent",1,"keyUp"],
    ["select",2],["Input.dispatchKeyEvent",2,"keyDown"],["Input.dispatchKeyEvent",2,"keyUp"]]);
  assert.equal(f.coordinator.pendingWindows(),0);
});

test("selection completion waits for Chrome's delayed activation event before native packets",async()=>{
  const f=fixture(),activation=deferred();
  f.chrome.tabs.update=async(id,options)=>{
    for(const tab of f.tabs.values())if(tab.windowId===10)tab.active=tab.id===id;
    Object.assign(f.tabs.get(id),options);
    setImmediate(()=>{f.events.activated({windowId:10,tabId:id});activation.resolve();});
    return {...f.tabs.get(id)};
  };
  const result=await f.coordinator.run(2,{surface:true},async context=>{
    await activation.promise;
    await f.packet(context,2,"keyDown");await f.packet(context,2,"keyUp");return "done";
  });
  assert.equal(result,"done");assert.equal(f.coordinator.pendingWindows(),0);
});

test("missing selection event refuses before any native packet",async()=>{
  const f=fixture();
  f.chrome.tabs.update=async id=>{for(const tab of f.tabs.values())if(tab.windowId===10)tab.active=tab.id===id;return {...f.tabs.get(id)};};
  const result=await f.coordinator.run(2,{surface:true},async context=>f.packet(context,2,"keyDown"));
  assert.equal(result.outcome,"attention_protected");assert.deepEqual(f.effects,[]);
});

test("foreground focus or an added human tab before first packet refuses with no effect",async()=>{
  for(const change of ["focus","human"]){
    const f=fixture();const result=await f.coordinator.run(1,{surface:true},async context=>{
      if(change==="focus"){f.windows.get(10).focused=true;f.events.focus(10);}
      else {f.tabs.set(9,{id:9,windowId:10,active:false});f.events.created(f.tabs.get(9));}
      await f.packet(context,1,"keyDown");return {outcome:"key_pressed"};
    });assert.equal(result.outcome,"attention_protected");assert.deepEqual(f.effects,[]);
    assert.equal(f.coordinator.pendingWindows(),0);
  }
});

test("moving target into human window during snapshot cannot select it",async()=>{
  const f=fixture();const original=f.chrome.tabs.query;let queries=0;
  f.chrome.tabs.query=async args=>{const result=await original(args);if(++queries===2){
    f.tabs.get(2).windowId=20;f.events.detached(2,{oldWindowId:10});f.events.attached(2,{newWindowId:20});
  }return result;};
  const result=await f.coordinator.run(2,{surface:true},async context=>{await f.packet(context,2,"keyDown");});
  assert.equal(result.outcome,"attention_protected");assert.deepEqual(f.effects,[]);
});

test("takeover after a packet remains unknown, suppresses later input and permits held-key cleanup",async()=>{
  const f=fixture();await assert.rejects(f.coordinator.run(1,{surface:true},async context=>{
    await f.packet(context,1,"keyDown");f.windows.get(10).focused=true;f.events.focus(10);
    try{await f.packet(context,1,"keyDown");}finally{await f.packet(context,1,"keyUp");}
  }),error=>error.effectUnknown===true);
  assert.deepEqual(f.effects,[["Input.dispatchKeyEvent",1,"keyDown"],["Input.dispatchKeyEvent",1,"keyUp"]]);
  assert.equal(f.coordinator.pendingWindows(),0);
});

test("held mouse cleanup bypasses stale document admission while final confirmation remains unknown",async()=>{
  const f=fixture();await assert.rejects(f.coordinator.run(1,{surface:true},async context=>{
    await f.chrome.debugger.sendCommand({tabId:1},"probe",{});
    await context.beforePacket("Input.dispatchMouseEvent",{type:"mousePressed"});
    f.events.focus(10);f.windows.get(10).focused=true;
    assert.equal(await context.beforePacket("Input.dispatchMouseEvent",{type:"mouseReleased"}),true);
    return {outcome:"activated"};
  }),error=>error.effectUnknown===true);
});

test("background editing protects a revealed target before selection, and between edits",async()=>{
  const f=fixture();f.windows.get(10).focused=true;
  let entered=false;
  const refused=await f.coordinator.run(1,{surface:false},async()=>{entered=true;});
  assert.equal(refused.outcome,"attention_protected");assert.equal(entered,false);
  const inactive=await f.coordinator.run(2,{surface:false},async context=>{
    await context.check();await context.beforePacket("Input.insertText",{text:"draft"});
    f.tabs.get(2).active=true;f.tabs.get(1).active=false;f.events.activated({windowId:10,tabId:2});
    await context.beforePacket("Input.insertText",{text:"must not follow"});
  }).catch(error=>error);
  assert.equal(inactive.effectUnknown,true);assert.equal(f.coordinator.pendingWindows(),0);
});

test("failed first command unlocks the window for later work without replay",async()=>{
  const f=fixture();await assert.rejects(f.coordinator.run(1,{surface:true},async()=>{throw new Error("lost reply");}),/lost reply/);
  assert.equal(await f.coordinator.run(2,{surface:true},async()=>"later"),"later");
  assert.deepEqual(f.effects,[["select",2]]);assert.equal(f.coordinator.pendingWindows(),0);
});


test("actual transport fences human focus or movement during document admission",async()=>{
  for(const change of ["focus","move"]){
    const f=fixture();
    f.sandbox.documents.input=async()=>{
      if(change==="focus"){f.windows.get(10).focused=true;f.events.focus(10);}
      else {f.tabs.get(1).windowId=20;f.events.detached(1,{oldWindowId:10});f.events.attached(1,{newWindowId:20});}
    };
    const result=await f.coordinator.run(1,{surface:true},async context=>{await f.packet(context,1,"keyDown");});
    assert.equal(result.outcome,"attention_protected");assert.deepEqual(f.effects,[]);
  }
});

test("actual transport marks a later admission takeover unknown and only releases the held key",async()=>{
  const f=fixture();let admitted=0;
  f.sandbox.documents.input=async()=>{if(++admitted===2){f.windows.get(10).focused=true;f.events.focus(10);}};
  await assert.rejects(f.coordinator.run(1,{surface:true},async context=>{
    await f.packet(context,1,"keyDown");
    try{await f.packet(context,1,"keyDown");}finally{
      f.sandbox.documents.input=async()=>{throw new Error("stale document must not block cleanup");};
      await f.packet(context,1,"keyUp");
    }
  }),error=>error.effectUnknown===true);
  assert.deepEqual(f.effects,[["Input.dispatchKeyEvent",1,"keyDown"],["Input.dispatchKeyEvent",1,"keyUp"]]);
});

test("a mouse release without a held button cannot bypass changed-window refusal",async()=>{
  const f=fixture();await assert.rejects(f.coordinator.run(1,{surface:true},async context=>{
    await context.beforePacket("Input.dispatchMouseEvent",{type:"mouseMoved"});
    f.windows.get(10).focused=true;f.events.focus(10);
    await context.beforePacket("Input.dispatchMouseEvent",{type:"mouseReleased"});
  }),error=>error.effectUnknown===true);
});


test("custody invalidation stops a later background edit without replay",async()=>{
  const f=fixture();await assert.rejects(f.coordinator.run(2,{surface:false},async context=>{
    await context.beforePacket("Input.insertText",{text:"first"});f.owners.clear();
    await context.beforePacket("Input.insertText",{text:"must not follow"});
  }),error=>error.effectUnknown===true);
});
