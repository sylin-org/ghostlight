import os
import pathlib,os,json,signal,time,subprocess,gi
area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA']);gi.require_version('Gio','2.0');from gi.repository import Gio,GLib
bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
def call(name,path,interface,method,args=None):return bus.call_sync(name,path,interface,method,args,None,Gio.DBusCallFlags.NONE,10000,None).unpack()
def identity(pid):
 try:
  p=pathlib.Path('/proc')/str(pid);fields=(p/'stat').read_text().split(') ')[-1].split();return {'pid':int(pid),'image':os.readlink(p/'exe'),'start_ticks':fields[19]}
 except (OSError,ValueError):return None
protected=[]
owned=[]
for path in pathlib.Path('/proc').iterdir():
 if path.name.isdigit():
  item=identity(path.name)
  if item and pathlib.Path(item['image']).name in ('ghostlight','ghostlight-mcp-connector') and not item['image'].startswith(str(area)+'/'):protected.append(item)
  if item and item['image'].startswith(str(area)+'/') and '/guest/' not in item['image'] and pathlib.Path(item['image']).name=='ghostlight':owned.append(item)
report={'before':owned,'quit':[],'fallback_cleanup':[]}
items=call('org.kde.StatusNotifierWatcher','/StatusNotifierWatcher','org.freedesktop.DBus.Properties','Get',GLib.Variant('(ss)',('org.kde.StatusNotifierWatcher','RegisteredStatusNotifierItems')))[0]
for item in items:
 name,path=item.split('/',1);path='/'+path
 pid=call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','GetConnectionUnixProcessID',GLib.Variant('(s)',(name,)))[0]
 match=next((p for p in owned if p['pid']==pid),None)
 if not match:continue
 assert identity(pid)==match
 menu=call(name,path,'org.freedesktop.DBus.Properties','Get',GLib.Variant('(ss)',('org.kde.StatusNotifierItem','Menu')))[0]
 layout=call(name,menu,'com.canonical.dbusmenu','GetLayout',GLib.Variant('(iias)',(0,-1,[])))[1]
 def nodes(node):
  yield node
  for child in node[2]:yield from nodes(child)
 quit_item=next(n for n in nodes(layout) if n[1].get('label','').replace('_','').lower() in ('quit','quit ghostlight'))
 call(name,menu,'com.canonical.dbusmenu','Event',GLib.Variant('(isvu)',(quit_item[0],'clicked',GLib.Variant('i',0),0)))
 report['quit'].append(match)
time.sleep(1)
for item in owned:
 if identity(item['pid'])==item:
  # The private session bus may already have exited. This is only the exact authority created
  # for that disposable no-tray test; never fall back to terminating other installations.
  assert item['image']==str(area/'no-tray/bin/ghostlight'),item
  os.kill(item['pid'],signal.SIGTERM);report['fallback_cleanup'].append(item)
time.sleep(.5)
report['remaining']=[p for p in owned if identity(p['pid'])==p]
report['protected_processes']=protected
assert report['remaining']==[]
for item in protected:assert identity(item['pid'])==item, 'Unrelated process identity changed'
(area/'cleanup.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
