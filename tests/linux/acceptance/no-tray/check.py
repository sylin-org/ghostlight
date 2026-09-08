import os
import os,pathlib,subprocess,json,time,gi
area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA'])/'no-tray'
for name,folder in [('XDG_CONFIG_HOME','config'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data'),('XDG_STATE_HOME','state')]:
 (area/folder).mkdir(parents=True,exist_ok=True);os.environ[name]=str(area/folder)
os.environ['GTK_MODULES']='atk-bridge';os.environ['NO_AT_BRIDGE']='0'
gi.require_version('Gio','2.0');from gi.repository import Gio,GLib
bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
bus.call_sync('org.a11y.Bus','/org/a11y/bus','org.freedesktop.DBus.Properties','Set',GLib.Variant('(ssv)',('org.a11y.Status','IsEnabled',GLib.Variant('b',True))),None,Gio.DBusCallFlags.NONE,10000,None)
gi.require_version('Atspi','2.0');from gi.repository import Atspi
binary=str(area/'bin/ghostlight')
subprocess.run([binary,'open'],check=True,timeout=30)
pid=next(int(p.name) for p in pathlib.Path('/proc').iterdir() if p.name.isdigit() and (p/'exe').exists() and os.path.realpath(p/'exe')==binary)
print('test PID',pid,flush=True)
root=Atspi.get_desktop(0)
def apps():return [root.get_child_at_index(i) for i in range(root.get_child_count())]
app=None
for _ in range(50):
 found=[a for a in apps() if a.get_process_id()==pid]
 if found:app=found[0];break
 time.sleep(.2)
print('accessible',bool(app),flush=True)
def walk(a,d=0):
 if d>8:return
 print(' '*d,a.get_role_name(),repr(a.get_name())[:100],flush=True)
 for i in range(min(a.get_child_count(),100)):walk(a.get_child_at_index(i),d+1)
if app:walk(app)
(area/'context.json').write_text(json.dumps({'pid':pid,'env':{k:v for k,v in os.environ.items() if k.startswith(('XDG_','DBUS_','AT_SPI_','GTK_','NO_AT'))}},indent=2))
print('CONTEXT READY',flush=True)
for _ in range(900):
 if (area/'stop').exists():break
 time.sleep(1)
