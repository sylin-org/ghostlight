import os
import pathlib,subprocess,json,time,os
root=pathlib.Path(os.environ['HOME']);signal=pathlib.Path(os.environ['GHOSTLIGHT_UPGRADE_FIXTURE']);log=open(root/'authority.log','w');authority=subprocess.Popen(['/usr/bin/ghostlight'],stdout=log,stderr=log)
deadline=time.time()+30
while True:
 assert authority.poll() is None,'Direct authority exited before MCP startup'
 status=json.loads(subprocess.check_output(['/usr/bin/ghostlight','status','--json'],text=True))
 if status.get('running'):break
 assert time.time()<deadline,'Authority startup timeout';time.sleep(.1)
mcp=subprocess.Popen(['/usr/bin/ghostlight-mcp-connector'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=open(root/'mcp.log','w'),text=True,bufsize=1);sequence=0

def request(method,params):
 global sequence
 sequence+=1;mcp.stdin.write(json.dumps({'jsonrpc':'2.0','id':sequence,'method':method,'params':params})+'\n');mcp.stdin.flush()
 for line in mcp.stdout:
  r=json.loads(line)
  if r.get('id')==sequence:return r
 raise RuntimeError('MCP connection ended')
init=request('initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'Debian package upgrade','version':'1'}});assert init['result']['serverInfo']['version']==os.environ['GHOSTLIGHT_LINUX_PREVIOUS_VERSION'],init
mcp.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');mcp.stdin.flush()
r=request('tools/call',{'name':'policy_explain','arguments':{}});assert r.get('result',{}).get('structuredContent',{}).get('status')=='succeeded',r.get('error',{})
(signal/'ready').write_text('ready');deadline=time.time()+90
while not (signal/'upgraded').exists():
 assert time.time()<deadline,'Package upgrade timeout';time.sleep(.1)
status=json.loads(subprocess.check_output(['/usr/bin/ghostlight','status','--json'],text=True));report={'before_version':os.environ['GHOSTLIGHT_LINUX_PREVIOUS_VERSION'],'after_package_status':status,'retained_mcp_pid':mcp.pid}
r=request('tools/call',{'name':'policy_explain','arguments':{}});assert r.get('result',{}).get('structuredContent',{}).get('status')=='succeeded',r.get('error',{})
# Explicitly stop this test's old authority once. The same initialized MCP process demand-starts
# the new installed image; it must not need a replacement transport or replay any prior request.
authority.terminate();authority.wait(timeout=15)
subprocess.run(['/usr/bin/ghostlight','open'],check=True,timeout=30)
deadline=time.time()+30
while True:
 state=json.loads(subprocess.check_output(['/usr/bin/ghostlight','status','--json'],text=True))
 if state.get('running') and state.get('version')==os.environ['GHOSTLIGHT_LINUX_VERSION']:break
 assert time.time()<deadline,'Updated authority startup timeout';time.sleep(.1)
deadline=time.time()+30
while True:
 catalog=request('tools/list',{})
 if len(catalog.get('result',{}).get('tools',[]))==23:break
 assert time.time()<deadline,'Retained MCP catalog did not recover';time.sleep(.1)
r=request('tools/call',{'name':'policy_explain','arguments':{}});assert r.get('result',{}).get('structuredContent',{}).get('status')=='succeeded',r.get('error',{})
status=json.loads(subprocess.check_output(['/usr/bin/ghostlight','status','--json'],text=True));report['after_explicit_authority_restart_status']=status
runtime=json.loads((root/'.cache/ghostlight/ghostlight-runtime.json').read_text());assert runtime['service_version']==os.environ['GHOSTLIGHT_LINUX_VERSION'],runtime['service_version']
report['passed']=True;report['scope']='package upgrade and retained initialized MCP; no browser';(signal/'results.json').write_text(json.dumps(report,indent=2)+'\n');mcp.stdin.close();mcp.wait(timeout=15)
