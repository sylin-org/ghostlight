import {join} from 'node:path';
import { readFileSync } from 'node:fs';
const [port,endpoint]=readFileSync(join(process.env.GHOSTLIGHT_LINUX_INSTALLED_AREA,'context/config/chromium/DevToolsActivePort'),'utf8').trim().split('\n');
const ws=new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
let id=0;const pending=new Map();
ws.onmessage=({data})=>{const m=JSON.parse(data);const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(new Error(JSON.stringify(m.error))):p.resolve(m.result)}};
function send(method,params={},sessionId){return new Promise((resolve,reject)=>{const n=++id;pending.set(n,{resolve,reject});ws.send(JSON.stringify({id:n,method,params,...(sessionId?{sessionId}:{})}))})}
try {
const {targetInfos}=await send('Target.getTargets');
const worker=targetInfos.find(t=>t.type==='service_worker'&&t.url==='chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/service-worker.js');
if(!worker)throw new Error('No acceptance adapter worker');
const {sessionId}=await send('Target.attachToTarget',{targetId:worker.targetId,flatten:true});
const r=await send('Runtime.evaluate',{expression:process.argv[2]||'JSON.stringify(uiSnapshot())',awaitPromise:true,returnByValue:true},sessionId);
if(r.exceptionDetails)throw new Error(JSON.stringify(r.exceptionDetails));
console.log(r.result.value);
}finally{ws.close()}
