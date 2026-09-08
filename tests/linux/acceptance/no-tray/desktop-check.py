import os
import pathlib,subprocess,json,time,concurrent.futures,os
root=pathlib.Path.cwd(); area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA'])/'no-tray';binary=area/'bin/ghostlight'
context=json.loads((area/'context.json').read_text());product_env={**os.environ,**context['env']}
def command(args):return subprocess.check_output(args,text=True,timeout=30,env=product_env if args[0]==str(binary) else None).strip()
def observed():
 result=[]
 for p in pathlib.Path('/proc').iterdir():
  if not p.name.isdigit():continue
  try:
   if os.readlink(p/'exe')==str(binary):result.append(int(p.name))
  except OSError:pass
 assert len(result)==1,result
 return result[0]
pid=observed(); results=[]; sequence=0
def script(body):
 global sequence
 sequence+=1;name=f'ghostlight-acceptance-{sequence}';path=area/f'kwin-{sequence}.js'
 path.write_text(f'const windows=workspace.windowList().filter(w=>w.pid==={pid});\n'+body)
 slot=command(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.loadScript',str(path),name])
 try:command(['qdbus6','org.kde.KWin',f'/Scripting/Script{slot}','org.kde.kwin.Script.run'])
 finally:command(['qdbus6','org.kde.KWin','/Scripting','org.kde.kwin.Scripting.unloadScript',name])
def windows():
 label=f'GHOSTLIGHT_NATIVE_{os.getpid()}_{sequence+1} '
 script('print('+json.dumps(label)+'+JSON.stringify(windows.map(w=>({pid:w.pid,caption:w.caption,minimized:w.minimized,active:workspace.activeWindow===w,id:String(w.internalId),width:w.width,height:w.height}))));')
 value=command(['journalctl','--user','--since','1 minute ago','--grep',label.strip(),'--no-pager','-o','cat'])
 return json.loads(value.split(label)[-1])
def opened():
 command([str(binary),'open']);time.sleep(.4);w=windows();assert len(w)==1 and w[0]['caption']=='Ghostlight' and w[0]['active'] and not w[0]['minimized'],w;assert observed()==pid;return w[0]
first=opened();results.append({'phase':'open-focuses-one-workbench','window':first})
script('if(windows.length!==1)throw new Error("not one window");windows[0].minimized=true;')
time.sleep(.2);assert windows()[0]['minimized'];replacement=opened();assert replacement['id']!=first['id'];results.append({'phase':'minimize-open-reconstructs','window':replacement})
script('if(windows.length!==1)throw new Error("not one window");windows[0].closeWindow();')
time.sleep(.3);assert windows()==[];assert observed()==pid;replacement=opened();results.append({'phase':'close-keeps-authority-and-open-recreates','window':replacement})
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:list(pool.map(lambda _:command([str(binary),'open']),range(8)))
time.sleep(.5);samples=[]
for i in range(10):
 w=windows();assert len(w)==1 and w[0]['active'] and not w[0]['minimized'],w;samples.append(w[0]);time.sleep(.05)
results.append({'phase':'eight-concurrent-open-requests','samples':samples})

# Renderer fault injection belongs only to this exact test authority's descendant tree.
def identity(process):
 try:
  fields=(pathlib.Path('/proc')/str(process)/'stat').read_text().split(') ')[-1].split()
  return {'pid':int(process),'image':os.readlink(f'/proc/{process}/exe'),'parent':int(fields[1]),'start':fields[19]}
 except (OSError,ValueError):return None
all_processes={int(p.name):identity(p.name) for p in pathlib.Path('/proc').iterdir() if p.name.isdigit()}
def owned(item):
 for _ in range(20):
  if not item:return False
  if item['pid']==pid:return True
  item=all_processes.get(item['parent'])
 return False
renderers=[i for i in all_processes.values() if i and pathlib.Path(i['image']).name=='WebKitWebProcess' and owned(i)]
assert len(renderers)==1,renderers
renderer=renderers[0];assert identity(renderer['pid'])==renderer;os.kill(renderer['pid'],9)
for _ in range(100):
 if windows()==[]:break
 time.sleep(.1)
assert windows()==[],windows();assert observed()==pid
recovered=opened();results.append({'phase':'renderer-loss-discards-only-view-and-open-recovers','renderer':renderer,'window':recovered})
# Install Applications and CLI surfaces into this disposable home, with the private desktop bus.
subprocess.run(['bwrap','--bind','/','/','--bind',str(pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA'])/'home'),str(pathlib.Path.home()),'--',str(binary),'install','--all-browsers','--no-clients','--no-open'],env=product_env,check=True,stdout=subprocess.DEVNULL)
entry=area/'data/applications/org.sylin.ghostlight.desktop';assert entry.is_file()
script('if(windows.length!==1)throw new Error("not one window");windows[0].closeWindow();');time.sleep(.3);assert windows()==[]
subprocess.run(['gio','launch',str(entry)],env=product_env,check=True,timeout=30);time.sleep(.5)
w=windows();assert len(w)==1 and w[0]['active'] and not w[0]['minimized'],w;assert observed()==pid
results.append({'phase':'Applications-opens-same-authority-without-a-tray-host','window':w[0]})
report={'passed':True,'pid':pid,'desktop':'KDE Wayland with a private session bus and no StatusNotifierWatcher','scope':'visible compositor windows of dedicated portable authority','checks':results}
(area/'desktop.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
