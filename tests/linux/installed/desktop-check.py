import os
import pathlib,subprocess,json,time,concurrent.futures,os
root=pathlib.Path.cwd(); area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_INSTALLED_AREA']);binary=area/'bin/ghostlight'
def command(args):return subprocess.check_output(args,text=True,timeout=20).strip()
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
report={'passed':True,'pid':pid,'desktop':'KDE Wayland','scope':'visible compositor windows of isolated installed authority','checks':results}
(area/'desktop.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
