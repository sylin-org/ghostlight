import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {randomUUID,createHash} from 'node:crypto';
import {mkdirSync,readFileSync,writeFileSync,existsSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {area,bin,env,product,doctor,cdp,mcp,delay,until} from './lib.mjs';
const output=join(area,`governance-${Date.now()}`);mkdirSync(output);const report={release_ready:false,phases:[],calls:[],passed:false};
const save=()=>writeFileSync(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');
let phase='preflight',client,dev,reply;const clients=[];
const sentinel='LINUX_PRIVATE_'+randomUUID().replaceAll('-','');
assert.ok(env.XDG_STATE_HOME?.startsWith(area+'/context/'),'Run acceptance/start.mjs for this dedicated area first');
const policyPath=join(env.XDG_STATE_HOME,'ghostlight/user-policy.json');assert.ok(policyPath.startsWith(area+'/context/'));mkdirSync(join(env.XDG_STATE_HOME,'ghostlight'),{recursive:true});
const server=createServer((req,res)=>{res.setHeader('content-type','text/html');if(req.url==='/child'){res.end(`<body style="background:#ff00ff">${sentinel}<label>Child draft<input aria-label="Child draft"></label></body>`);return}if(req.url==='/redirect'){res.writeHead(302,{location:`http://127.0.0.1:${server.address().port}/child`});res.end();return}res.end(`<!doctype html><title>Linux governed fixture</title><h1>Permitted parent</h1><button onclick="this.dataset.count=String(Number(this.dataset.count||0)+1)">Count effect</button><label>Ordinary draft<input aria-label="Ordinary draft"></label><label>Password<input aria-label="Password" type="password"></label>${req.url==='/frames'?`<iframe src="http://127.0.0.1:${server.address().port}/child" width="600" height="220"></iframe>`:''}`)});
await new Promise(done=>server.listen(0,'127.0.0.1',done));const base=`http://localhost:${server.address().port}`;
function policy(allowed=['read','action','write','execute'],child=false,mode='permitted_content'){writeFileSync(policyPath,JSON.stringify({schema:3,name:'Dedicated installed Linux acceptance',version:randomUUID(),grants:[{id:'parent',hosts:{allow:child?['localhost','127.0.0.1']:['localhost']},allowed}],config:[{key:'browser.startup',value:'manual',level:'mandatory'},{key:'content.frames.handling',value:mode,level:'mandatory'}]}));}
async function session(){client=await mcp();clients.push(client)}
async function call(name,args={},expected='succeeded'){reply=await client.call(name,args);const r=reply.structuredContent;report.calls.push({tool:name,status:r?.status,effect:r?.effect,reason:r?.facts?.reason,policy_rule:r?.facts?.policy_rule});save();assert.equal(r?.status,expected,JSON.stringify(r));return r}
function pass(facts={}){report.phases.push({name:phase,status:'passed',...facts});save();console.log('PASS',phase)}
async function rawFor(url){const t=await until(async()=>(await dev.send('Target.getTargets')).targetInfos.find(x=>x.type==='page'&&x.url===url),'own fixture target');return dev.attach(t.targetId)}
async function opened(path='/'){const r=await call('browser_navigate',{url:base+path,new_tab:true});await call('browser_wait',{tab:r.facts.tab,condition:'load_ready'});return {tab:r.facts.tab,raw:await rawFor(base+path)}}
try{
 assert.equal(doctor().readiness.state,'ready');dev=await cdp();policy();product(['diagnostics','on']);await session();let {tab,raw}=await opened();
 phase='configured-RAWX-allows-retained-effects';
 await call('browser_read',{tab});await call('browser_click',{tab,selector:{name:'Count effect',role:'button'}});
 assert.equal(await raw("document.querySelector('button').dataset.count"),'1');
 await call('browser_fill_form',{tab,fields:[{selector:{name:'Ordinary draft'},value:sentinel}]});assert.equal(await raw("document.querySelector('input').value"),sentinel);
 const value=await call('browser_execute',{tab,script:'2+3'});assert.equal(value.facts.value,5);pass();
 for(const [allowed,tool,args,probe] of [
  [['action','write','execute'],'browser_read',{tab},null],
  [['read','write','execute'],'browser_click',{tab,selector:{name:'Count effect',role:'button'}},"document.querySelector('button').dataset.count"],
  [['read','action','execute'],'browser_fill_form',{tab,fields:[{selector:{name:'Ordinary draft'},value:'must-not-replace'}]},"document.querySelector('input').value"],
  [['read','action','write'],'browser_execute',{tab,script:"document.title='must-not-run'"},'document.title']]){
  phase='independent-denial-'+tool;const before=probe?await raw(probe):null;policy(allowed);const r=await call(tool,args,'blocked');assert.equal(r.effect,'none');assert.equal(r.facts.policy_rule,'capability');if(probe)assert.equal(await raw(probe),before);pass();
 }
 phase='policy-restoration-keeps-the-original-draft';policy();await call('browser_read',{tab});assert.equal(await raw("document.querySelector('input').value"),sentinel);pass();
 phase='excluded-frame-read-and-script-refusal';await session();({tab,raw}=await opened('/frames'));await until(()=>raw("document.querySelector('iframe').contentWindow.length===0"),'child exists');await delay(500);
 const read=await call('browser_read',{tab});assert.equal(read.facts.coverage.excluded_documents,1);assert.doesNotMatch(JSON.stringify(read),new RegExp(sentinel+'|127\\.0\\.0\\.1'));const title=await raw('document.title');await call('browser_execute',{tab,script:"document.title='must-not-run'"},'blocked');assert.equal(await raw('document.title'),title);pass();
 phase='delivered-screenshot-masks-excluded-pixels';const capture=await call('browser_screenshot',{tab,full_page:true});assert.equal(capture.facts.coverage.masked_regions,1);const image=reply.content.find(x=>x.type==='image');assert.ok(image);writeFileSync(join(output,'masked.jpg'),Buffer.from(image.data,'base64'));
 const pixels=await raw(`(async()=>{const image=new Image();image.src=${JSON.stringify('data:'+image.mimeType+';base64,'+image.data)};await image.decode();const canvas=document.createElement('canvas');canvas.width=image.width;canvas.height=image.height;const ctx=canvas.getContext('2d');ctx.drawImage(image,0,0);const rect=document.querySelector('iframe').getBoundingClientRect(),scale=image.width/document.documentElement.scrollWidth;return [0.2,0.8].flatMap(x=>[0.2,0.8].map(y=>[...ctx.getImageData(Math.round((rect.left+scrollX+rect.width*x)*scale),Math.round((rect.top+scrollY+rect.height*y)*scale),1,1).data]));})()`);
 for(const pixel of pixels)for(const [i,value]of [32,36,43,255].entries())assert.ok(Math.abs(pixel[i]-value)<=5,JSON.stringify(pixel));assert.equal(await raw("getComputedStyle(document.querySelector('iframe')).visibility"),'visible');pass({samples:pixels.length});
 phase='allowed-frame-content-and-stale-document-rejection';policy(undefined,true);const permitted=await call('browser_read',{tab});assert.match(JSON.stringify(permitted),new RegExp(sentinel));const found=await call('browser_find',{tab,text:'Child draft',scope:'control'});report.find_shape=Object.keys(found.facts);save();const target=found.facts.matches[0].target;
 await raw(`document.querySelector('iframe').src='http://127.0.0.1:${server.address().port}/child?replacement'`);await delay(500);const stale=await client.call('browser_fill_form',{tab,fields:[{target,value:'must-not-fill'}]});assert.notEqual(stale.structuredContent.status,'succeeded');assert.equal(stale.structuredContent.effect,'none');pass();
 phase='strict-frame-coverage-refuses-incomplete-page';policy(undefined,false,'complete_page');await session();tab=(await call('browser_navigate',{url:base+'/frames',new_tab:true})).facts.tab;await delay(500);const strict=await call('browser_read',{tab},'blocked');assert.equal(strict.facts.coverage.excluded_documents,1);pass();
 phase='redirect-rechecks-the-committed-host';policy();await session();const redirect=await call('browser_navigate',{url:base+'/redirect',new_tab:true},'blocked');assert.equal(redirect.facts.reason,'host_denied');assert.doesNotMatch(JSON.stringify(redirect),new RegExp(sentinel));pass();
 phase='popup-pause-resume-and-terminal-stop';policy();await session();({tab,raw}=await opened('/controls'));
 const popup=await dev.send('Target.createTarget',{url:'chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/popup.html'});const human=await dev.attach(popup.targetId);await until(()=>human("!!document.querySelector('#toggle')&&!document.querySelector('#toggle').disabled"),'popup connected');
 await human("document.querySelector('#toggle').click()");await until(()=>human("document.querySelector('#status').textContent.includes('PAUSED')"),'pause visible');await call('browser_click',{tab,selector:{name:'Count effect',role:'button'}},'blocked');assert.equal(await raw("document.querySelector('button').dataset.count"),undefined);
 await human("document.querySelector('#toggle').click()");await until(()=>human("document.querySelector('#status').textContent.includes('allowed')"),'resume visible');await call('browser_click',{tab,selector:{name:'Count effect',role:'button'}});assert.equal(await raw("document.querySelector('button').dataset.count"),'1');
 await human("document.querySelector('#session-button').click()");await until(()=>human("document.querySelector('#session-button').dataset.intent==='start_session'"),'stop visible');await call('browser_click',{tab,selector:{name:'Count effect',role:'button'}},'blocked');assert.equal(await raw("document.querySelector('button').dataset.count"),'1');
 await human("document.querySelector('#session-button').click()");await until(()=>human("document.querySelector('#session-button').dataset.intent==='end_session'"),'new session visible');await call('browser_read',{tab});assert.equal(await raw("document.querySelector('button').dataset.count"),'1');pass();
 phase='preserve-tabs-keeps-local-evidence';await call('browser_tabs',{action:'close',tab},'blocked');assert.equal(await raw('document.title'),'Linux governed fixture');pass();
 phase='installed-audit-and-process-logs-exclude-sentinel';await delay(2200);const audit=readFileSync(join(bin,'audit.jsonl'),'utf8');assert.ok(audit.length>0);assert.ok(!audit.includes(sentinel));const logs=product(['diagnostics','show','--last','10m','--json']);assert.ok(logs.length>0);assert.ok(!logs.includes(sentinel));pass({audit_bytes:Buffer.byteLength(audit),diagnostic_bytes:Buffer.byteLength(logs)});
 report.passed=true;
}catch(error){report.phases.push({name:phase,status:'failed',reason:error.message});console.error(error);process.exitCode=1}finally{for(const c of clients)c.close();dev?.close();server.closeAllConnections();server.close();product(['diagnostics','off']);if(existsSync(policyPath))writeFileSync(join(output,'policy.json'),readFileSync(policyPath));report.finished_at=new Date().toISOString();save();console.log(output)}
