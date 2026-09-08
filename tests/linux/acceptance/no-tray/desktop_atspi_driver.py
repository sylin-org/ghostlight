import os
import pathlib,json,os,subprocess,gi,time
area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA'])/'no-tray';c=json.loads((area/'context.json').read_text());os.environ.update(c['env']);subprocess.run([str(area/'bin/ghostlight'),'open'],check=True,timeout=30)
gi.require_version('Atspi','2.0');from gi.repository import Atspi
time.sleep(1);root=Atspi.get_desktop(0)
def walk(a,d=0):
 if d>8:return
 print(' '*d,a.get_role_name(),repr(a.get_name())[:120],flush=True)
 for i in range(min(a.get_child_count(),120)):walk(a.get_child_at_index(i),d+1)
for i in range(root.get_child_count()):
 a=root.get_child_at_index(i)
 if a.get_process_id()==c['pid']:walk(a)
