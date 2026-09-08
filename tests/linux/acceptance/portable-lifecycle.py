import os
import os,json,pathlib,subprocess,hashlib
area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_AREA']);ctx=json.loads((area/'no-tray/context.json').read_text());env={**os.environ,**ctx['env']};binary=area/'no-tray/bin/ghostlight';home=area/'home';state=area/'no-tray';report={'passed':False,'release_ready':False,'checks':[]}
# The bwrap home mount protects actual client config and the ordinary command symlink.
def cli(*args):
 r=subprocess.run(['bwrap','--bind','/','/','--bind',str(home),str(pathlib.Path.home()),'--',str(binary),*args],env=env,text=True,capture_output=True,timeout=30)
 (state/('lifecycle-'+str(len(report['checks']))+'.log')).write_text(r.stdout+r.stderr)
 assert r.returncode==0,r.stdout+r.stderr
 return r.stdout
client=home/'.claude.json';original={'mcpServers':{'foreign-server':{'command':'/bin/true','args':['retain-this']}}};client.write_text(json.dumps(original,indent=2))
cli('install','--all-browsers','--client','claude-code','--no-open');configured=json.loads(client.read_text());assert configured['mcpServers']['foreign-server']==original['mcpServers']['foreign-server'];assert 'ghostlight' in configured['mcpServers'];report['checks'].append('actual-Claude-config-install-preserves-foreign-server')
# Establish a real saved receipt before removing setup surfaces.
subprocess.run([str(binary),'call','policy_explain','{}'],env=env,check=True,stdout=subprocess.DEVNULL);audit=state/'bin/audit.jsonl';before=audit.read_bytes();assert before
foreign=state/'config/BraveSoftware/Brave-Browser/NativeMessagingHosts/org.sylin.ghostlight.json';foreign_bytes=b'{"name":"org.example.foreign","path":"/bin/true","type":"stdio"}\n';foreign.write_bytes(foreign_bytes)
command=home/'.local/bin/ghostlight';assert command.is_symlink();command.unlink();command.write_text('foreign command must survive\n');command_bytes=command.read_bytes()
cli('uninstall','--all-browsers','--client','claude-code','--no-open');assert json.loads(client.read_text())==original;assert foreign.read_bytes()==foreign_bytes;assert command.read_bytes()==command_bytes;assert audit.read_bytes()==before;assert not (state/'data/applications/org.sylin.ghostlight.desktop').exists();assert not (state/'config/chromium/NativeMessagingHosts/org.sylin.ghostlight.json').exists();report['checks'].append('uninstall-preserves-foreign-manifest-command-client-and-audit')
# Remove only the explicit fixture foreign entries; reinstall the ordinary owned surfaces.
foreign.unlink();command.unlink();cli('install','--all-browsers','--client','claude-code','--no-open');assert (state/'data/applications/org.sylin.ghostlight.desktop').is_file();assert command.is_symlink();assert json.loads(client.read_text())['mcpServers']['foreign-server']==original['mcpServers']['foreign-server'];assert audit.read_bytes()==before;report['checks'].append('portable-reinstall-restores-owned-surfaces-and-retains-audit')
report['passed']=True;report['audit_sha256']=hashlib.sha256(before).hexdigest();(state/'lifecycle.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
