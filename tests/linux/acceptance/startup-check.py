import os
import pathlib,subprocess,json,os,tarfile,time
area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA'])/'startup';area.mkdir(parents=True,exist_ok=True);binary_dir=area/'bin';binary_dir.mkdir(exist_ok=True)
subprocess.run(['tar','-xzf',os.environ['GHOSTLIGHT_LINUX_PORTABLE_ARCHIVE'],'-C',str(binary_dir),'--strip-components=1'],check=True)
env=os.environ.copy()
for key,folder in [('XDG_CONFIG_HOME','config'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data'),('XDG_STATE_HOME','state')]:env[key]=str(area/folder);(area/folder).mkdir(exist_ok=True)
binary=str(binary_dir/'ghostlight');bad={**env,'GDK_BACKEND':'wayland','WAYLAND_DISPLAY':'ghostlight-test-missing-display','DISPLAY':''};started=time.monotonic()
r=subprocess.run([binary,'open'],env=bad,capture_output=True,text=True,timeout=50);elapsed=time.monotonic()-started;(area/'failed.log').write_text(r.stdout+r.stderr);assert r.returncode!=0
# A subsequent valid startup must recover through the ordinary Open path.
good=subprocess.run([binary,'open'],env=env,capture_output=True,text=True,timeout=50);(area/'recovered.log').write_text(good.stdout+good.stderr);assert good.returncode==0
pid=next(int(p.name) for p in pathlib.Path('/proc').iterdir() if p.name.isdigit() and (p/'exe').exists() and os.path.realpath(p/'exe')==binary)
report={'passed':True,'failed_start_returncode':r.returncode,'failed_start_seconds':round(elapsed,3),'recovered_pid':pid,'scope':'missing Wayland display followed by ordinary valid desktop Open','env':{k:v for k,v in env.items() if k.startswith('XDG_')}};(area/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='env'}))
