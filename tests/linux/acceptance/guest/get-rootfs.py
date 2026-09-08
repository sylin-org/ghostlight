import os
import urllib.request,json,pathlib,hashlib,tarfile,subprocess,sys
image,tag=sys.argv[1:]
assert image in ('debian','ubuntu'), 'Expected debian or ubuntu'
area=pathlib.Path(os.environ['GHOSTLIGHT_LINUX_GUEST_AREA'])/image
area.mkdir(parents=True,exist_ok=True)
def read(url,headers={}):return urllib.request.urlopen(urllib.request.Request(url,headers=headers),timeout=120).read()
token=json.loads(read('https://auth.docker.io/token?service=registry.docker.io&scope=repository:library/'+image+':pull'))['token']
headers={'Authorization':'Bearer '+token,'Accept':'application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json'}
base='https://registry-1.docker.io/v2/library/'+image
index=json.loads(read(base+'/manifests/'+tag,headers));digest=next(m['digest'] for m in index['manifests'] if m['platform']['architecture']=='amd64' and m['platform']['os']=='linux')
manifest_bytes=read(base+'/manifests/'+digest,headers);assert 'sha256:'+hashlib.sha256(manifest_bytes).hexdigest()==digest
manifest=json.loads(manifest_bytes);(area/'manifest.json').write_bytes(manifest_bytes)
for layer in manifest['layers']:
 digest=layer['digest'];path=area/(digest.split(':')[1]+'.tar.gz')
 if not path.exists():path.write_bytes(read(base+'/blobs/'+digest,headers))
 assert 'sha256:'+hashlib.sha256(path.read_bytes()).hexdigest()==digest
 root=area/'root';root.mkdir(exist_ok=True)
 subprocess.run(['unshare','--user','--map-auto','--map-root-user','tar','-xzf',str(path),'-C',str(root)],check=True)
 print(image,tag,digest,flush=True)
(root/'etc/resolv.conf').unlink(missing_ok=True);(root/'etc/resolv.conf').write_bytes(pathlib.Path('/etc/resolv.conf').read_bytes())
