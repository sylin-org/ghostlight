// Disposable Windows install proof through unchanged connectNative and the native Workbench.
// Only the adapter host name changes. All binaries, profiles, manifests and user state are owned
// fixtures. No production registration, existing browser or persistent preference is changed.
// Suite opt-in: GHOSTLIGHT_TEST_NATIVE_INSTALLATION=1 node tests/hardening-suite.mjs --lane=all
// Or run this file with GHOSTLIGHT_BIN_DIR pointing at the exact freshly built siblings.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createServer as createSocketServer } from "node:net";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fixtureRenderingArguments, readDevToolsPort, waitForChromiumExit } from "./lib/chromium.mjs";

assert.equal(process.platform, "win32", "This journey requires an interactive Windows desktop.");
assert.equal(process.argv.length, 2, "Usage: node tests/quiet-native-installed-journey.mjs");
const repository = resolve(import.meta.dirname, "..");
const sourceBin = resolve(process.env.GHOSTLIGHT_BIN_DIR || join(repository, ".target-ghostlight-1.0/debug"));
const browser = resolve(process.env.GHOSTLIGHT_TEST_BROWSER || join(repository, ".tmp/chrome-testing/chrome-win64/chrome.exe"));
assert.ok(existsSync(browser), "Set GHOSTLIGHT_TEST_BROWSER to Chrome for Testing with unpacked extensions.");
const tempRoot = resolve(process.env.TEMP || process.env.TMP);
const scratch = mkdtempSync(join(tempRoot, "ghostlight-quiet-native-"));
const bin = join(scratch, "bin"), user = join(scratch, "user"), extension = join(scratch, "extension");
for (const path of [bin, user, join(user, "AppData/Roaming"), join(user, "AppData/Local")]) mkdirSync(path, { recursive: true });
mkdirSync(join(repository, ".tmp"), { recursive: true });
const evidence = join(repository, ".tmp", `quiet-native-installed-${new Date().toISOString().replace(/[:.]/g, "-")}-${process.pid}.json`);
const runtime = join(user, ".ghostlight/ghostlight-runtime.json");
const host = `org.sylin.ghostlight.quiet_${randomUUID().replaceAll("-", "")}`;
const manifest = join(scratch, "native-host.json"), registryScript = join(scratch, "owned-registry.ps1");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const report = { started_at: new Date().toISOString(), passed: false, failure: "run_incomplete",
  transport: "Chrome connectNative -> registered copied browser connector -> ordinary service discovery -> MCP and CLI",
  workbench: "Production Tauri WebView2 and production Show tab click handler",
  limitations: ["Disposable local installation; not the existing user install or store packaging", "Typing and operator clicks use CDP, not physical input",
    "Chrome retains document.hasFocus after native input even without emulation; cleanup uses an inactive never-native sentinel plus native-page visibility and debugger detachment"],
  browser_fixture_arguments: fixtureRenderingArguments(),
  revision: execFileSync("git", ["rev-parse", "HEAD"], { cwd: repository, windowsHide: true, encoding: "utf8" }).trim(),
  host, scratch, runtime, runtime_override: false, binaries: {}, checks: [], receipts: [], observations: [], artifacts: [] };
const save = () => writeFileSync(evidence, JSON.stringify(report, null, 2) + "\n");
const sourceFingerprint = () => {
  const paths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
    "crates", "extension", "tests", "Cargo.toml", "Cargo.lock"], { cwd: repository, windowsHide: true, encoding: "utf8" })
    .split("\0").filter(Boolean).sort();
  return hash(paths.map(path => `${path}\0${existsSync(join(repository, path)) ? hash(readFileSync(join(repository, path))) : "deleted"}\n`).join(""));
};
const check = name => { report.checks.push(name); save(); console.log(`PASS native: ${name}`); };
save();
const children = [], sockets = [];
const environment = { ...process.env, USERPROFILE: user, APPDATA: join(user, "AppData/Roaming"),
  LOCALAPPDATA: join(user, "AppData/Local"), WEBVIEW2_USER_DATA_FOLDER: join(scratch, "webview"),
  GHOSTLIGHT_AUDIT_FILE: join(scratch, "audit.jsonl"), GHOSTLIGHT_POLICY_FILE: join(scratch, "policy.json"),
  GHOSTLIGHT_DIAGNOSTICS_DIR: join(scratch, "diagnostics"), GHOSTLIGHT_NATIVE_HOST_DIR: join(scratch, "native-host-inspect") };
delete environment.GHOSTLIGHT_RUNTIME_FILE;
const executable = name => join(bin, name + ".exe");
const delay = ms => new Promise(done => setTimeout(done, ms));
async function until(fn, label, timeout = 30000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await fn(); if (value) return value; await delay(50); }
  throw new Error(`Timed out: ${label}`);
}
function start(path, args = []) {
  const child = spawn(path, args, { env: environment, windowsHide: true, stdio: ["pipe", "pipe", "pipe"] });
  child.stderr.on("data", data => { child.logs = ((child.logs || "") + data).slice(-12000); });
  child.on("error", error => { child.startError = error; }); children.push(child); return child;
}
function channel(write) {
  let next = 0; const pending = new Map();
  return { send(method, params = {}, sessionId) { const id = ++next;
    return new Promise((done, reject) => { const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 45000);
      pending.set(id, { done, reject, timer }); write({ id, method, params, ...(sessionId ? { sessionId } : {}) }); }); },
  receive(message) { const item = pending.get(message.id); if (!item) return; pending.delete(message.id); clearTimeout(item.timer);
    if (message.error) item.reject(new Error(JSON.stringify(message.error))); else item.done(message.result); } };
}
async function connect(url) {
  const socket = new WebSocket(url); sockets.push(socket);
  await new Promise((done, reject) => { socket.onopen = done; socket.onerror = reject; });
  const cdp = channel(message => socket.send(JSON.stringify(message)));
  socket.onmessage = ({ data }) => cdp.receive(JSON.parse(data)); return cdp;
}
async function evaluate(cdp, expression, sessionId) {
  const result = await cdp.send("Runtime.evaluate", { expression: `await (${expression})`, returnByValue: true, awaitPromise: true, replMode: true }, sessionId);
  assert.equal(result.exceptionDetails, undefined, JSON.stringify(result.exceptionDetails)); return result.result.value;
}
// The helper refuses pre-existing names and removes only our exact one-value, no-child key.
// It snapshots both Windows registry views and both hives for existing production identities.
writeFileSync(registryScript, `param([ValidateSet('snapshot','register','cleanup','processes','ancestry')][string]$Action,[string]$HostName,[string]$ManifestPath,[string]$ManifestHash,[string]$BinPath,[int]$ChildId,[string]$BrowserPath,[string]$ProfilePath)
$ErrorActionPreference='Stop'
$unique='Software\\Google\\Chrome\\NativeMessagingHosts\\'+$HostName
if($HostName -notmatch '^org\\.sylin\\.ghostlight\\.quiet_[a-f0-9]{32}$'){throw 'Invalid owned host name'}
if($Action -eq 'processes') {
  @(Get-CimInstance Win32_Process -Filter "Name = 'ghostlight.exe' OR Name = 'ghostlight-browser-connector.exe'" | Where-Object {$_.ExecutablePath -and [IO.Path]::GetDirectoryName($_.ExecutablePath) -eq $BinPath} | ForEach-Object {@{id=$_.ProcessId;parent=$_.ParentProcessId;path=$_.ExecutablePath}}) | ConvertTo-Json -Depth 5 -Compress
  exit
}
if($Action -eq 'ancestry') {
  $chain=@();$node=Get-CimInstance Win32_Process -Filter "ProcessId = $ChildId"
  if(-not $node -or $node.ExecutablePath -cne (Join-Path $BinPath 'ghostlight-browser-connector.exe')){throw 'Native child identity changed'}
  for($step=0;$step -lt 8 -and $node;$step++){
    $ownedChrome=$node.ExecutablePath -eq $BrowserPath -and $node.CommandLine -and $node.CommandLine.Contains($ProfilePath)
    $chain+=@{id=$node.ProcessId;parent=$node.ParentProcessId;path=$node.ExecutablePath;owned_profile=[bool]$ownedChrome}
    if($ownedChrome){break};$node=Get-CimInstance Win32_Process -Filter ('ProcessId = '+$node.ParentProcessId)
  };@{chain=$chain}|ConvertTo-Json -Depth 5 -Compress;exit
}
$hives=@([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)
$views=@([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)
if($Action -eq 'snapshot') {
  $records=@();$vendors=@('Google\\Chrome','Microsoft\\Edge','BraveSoftware\\Brave-Browser','Chromium','Mozilla')
  foreach($hive in $hives){foreach($view in $views){$root=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
    try{foreach($vendor in $vendors){foreach($prefix in @('Software\\','Software\\Ghostlight\\Isolated\\')){
      $name=$prefix+$vendor+'\\NativeMessagingHosts\\org.sylin.ghostlight';$key=$root.OpenSubKey($name)
      $values=@();$children=@();if($key){try{foreach($valueName in ($key.GetValueNames()|Sort-Object)){
        $value=$key.GetValue($valueName,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        $fileHash=$null;if($value -is [string] -and [IO.File]::Exists($value)){$fileHash=(Get-FileHash -LiteralPath $value -Algorithm SHA256).Hash}
        $values+=@{name=$valueName;kind=$key.GetValueKind($valueName).ToString();value=$value;manifest_sha256=$fileHash}
      };$children=@($key.GetSubKeyNames()|Sort-Object)}finally{$key.Dispose()}}
      $records+=@{hive=$hive.ToString();view=$view.ToString();key=$name;exists=[bool]$key;values=$values;children=$children}
    }}}finally{$root.Dispose()}
  }};$records|ConvertTo-Json -Depth 8 -Compress;exit
}
if($Action -eq 'register') {
  foreach($hive in $hives){foreach($view in $views){$root=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view);try{$key=$root.OpenSubKey($unique);if($key){$key.Dispose();throw 'Owned host name already exists'}}finally{$root.Dispose()}}}
  if((Get-FileHash -LiteralPath $ManifestPath -Algorithm SHA256).Hash -ne $ManifestHash){throw 'Manifest identity changed'}
  $root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryView]::Registry64)
  try{$key=$root.CreateSubKey($unique);try{$key.SetValue('',$ManifestPath,[Microsoft.Win32.RegistryValueKind]::String)}finally{$key.Dispose()}}finally{$root.Dispose()}
  @{registered=$true;key=$unique;manifest=$ManifestPath}|ConvertTo-Json -Compress;exit
}
$root=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryView]::Registry64)
try{$key=$root.OpenSubKey($unique);if(-not $key){throw 'Owned registration disappeared before cleanup'}
  try{if($key.SubKeyCount -ne 0 -or $key.ValueCount -ne 1 -or $key.GetValueNames()[0] -ne '' -or $key.GetValueKind('') -ne [Microsoft.Win32.RegistryValueKind]::String -or $key.GetValue('') -cne $ManifestPath){throw 'Registration ownership changed; refusing cleanup'}
    if((Get-FileHash -LiteralPath $ManifestPath -Algorithm SHA256).Hash -ne $ManifestHash){throw 'Manifest ownership changed; refusing cleanup'}
  }finally{$key.Dispose()};$root.DeleteSubKey($unique,$true)
  if($root.OpenSubKey($unique)){throw 'Owned registration still present'}
}finally{$root.Dispose()};@{removed=$true;key=$unique}|ConvertTo-Json -Compress
`);
function registry(action, childId = 0, profilePath = "") {
  const result = execFileSync("pwsh", ["-NoProfile", "-NonInteractive", "-File", registryScript,
    "-Action", action, "-HostName", host, "-ManifestPath", manifest, "-ManifestHash", report.manifest_sha256 || "", "-BinPath", bin,
    "-ChildId", String(childId), "-BrowserPath", browser, "-ProfilePath", profilePath],
  { encoding: "utf8", windowsHide: true }); return result.trim() ? JSON.parse(result) : [];
}
const server = createServer((_request, response) => {
  response.setHeader("content-type", "text/html; charset=utf-8");
  response.end(`<!doctype html><title>Disposable installed quiet fixture</title><h1>Human work</h1>
    <label>Human draft<textarea id="human" aria-label="Human draft"></textarea></label>
    <label>Agent draft<input id="agent" aria-label="Agent draft"></label>
    <script>window.inputs=[];human.addEventListener('input',event=>inputs.push(event.isTrusted));</script>`);
});
let registered = false, chromium, authority, cdp, nativeProcesses = [], journeyPassed = false;
try {
  report.source_sha256 = sourceFingerprint();
  for (const name of ["ghostlight", "ghostlight-mcp-connector", "ghostlight-browser-connector"]) {
    const source = join(sourceBin, name + ".exe"); assert.ok(existsSync(source), `Build ${name} into GHOSTLIGHT_BIN_DIR first.`);
    cpSync(source, executable(name)); report.binaries[name] = { source, path: executable(name), sha256: hash(readFileSync(source)) };
    assert.equal(hash(readFileSync(executable(name))), report.binaries[name].sha256);
  }
  cpSync(join(repository, "extension"), extension, { recursive: true });
  const sharedPath = join(extension, "lib/shared.js"), shared = readFileSync(sharedPath, "utf8");
  const originalName = 'const NATIVE_HOST_NAME = "org.sylin.ghostlight";';
  assert.equal(shared.split(originalName).length, 2, "Expected one native host identity declaration");
  const specialized = shared.replace(originalName, `const NATIVE_HOST_NAME = "${host}";`);
  assert.equal(specialized.replace(`const NATIVE_HOST_NAME = "${host}";`, originalName), shared);
  writeFileSync(sharedPath, specialized);
  report.adapter_specialization = { path: "lib/shared.js", original_sha256: hash(shared), loaded_sha256: hash(specialized), change: "native host name only" };
  report.worker_sha256 = hash(readFileSync(join(extension, "service-worker.js")));
  assert.equal(report.worker_sha256, hash(readFileSync(join(repository, "extension/service-worker.js"))));
  const extensionManifest = JSON.parse(readFileSync(join(extension, "manifest.json"), "utf8"));
  const extensionId = [...createHash("sha256").update(Buffer.from(extensionManifest.key, "base64")).digest().subarray(0, 16)]
    .flatMap(byte => [byte >> 4, byte & 15]).map(value => String.fromCharCode(97 + value)).join("");
  report.extension_id = extensionId;
  writeFileSync(manifest, JSON.stringify({ name: host, description: "Disposable Ghostlight native proof", path: executable("ghostlight-browser-connector"),
    type: "stdio", allowed_origins: [`chrome-extension://${extensionId}/`] }));
  report.manifest_sha256 = hash(readFileSync(manifest)).toUpperCase();
  report.registrations_before = registry("snapshot"); save();
  report.registration = registry("register"); registered = true; save();
  writeFileSync(environment.GHOSTLIGHT_POLICY_FILE, JSON.stringify({ schema: 3, name: "Disposable installed quiet fixture", version: "1",
    grants: [{ id: "local", hosts: { allow: ["localhost", "127.0.0.1"] }, allowed: ["read", "action", "write", "execute"] }],
    config: [{ key: "browser.startup", value: "manual", level: "mandatory" }, { key: "browser.attention", value: "background", level: "mandatory" }] }));
  const reserved = createSocketServer(); await new Promise(done => reserved.listen(0, "127.0.0.1", done));
  const webviewPort = reserved.address().port; await new Promise(done => reserved.close(done));
  environment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = [`--remote-debugging-port=${webviewPort}`, ...fixtureRenderingArguments()].join(" ");
  report.webview_fixture_arguments = environment.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;
  await new Promise(done => server.listen(0, "127.0.0.1", done));
  const origin = `http://localhost:${server.address().port}`;
  authority = start(executable("ghostlight"));
  await until(() => { if (authority.startError) throw authority.startError; assert.equal(authority.exitCode, null, authority.logs); return existsSync(runtime); }, "ordinary disposable authority startup");
  const { token: _privateToken, ...runtimeIdentity } = JSON.parse(readFileSync(runtime, "utf8"));
  report.runtime_identity = runtimeIdentity; save();
  const profile = join(scratch, "chrome-profile");
  chromium = start(browser, ["--remote-debugging-port=0", `--user-data-dir=${profile}`, `--load-extension=${extension}`,
    ...fixtureRenderingArguments(),
    "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost, EXCLUDE 127.0.0.1", "--no-first-run", "--no-default-browser-check",
    "--disable-background-networking", "--disable-component-update", "--disable-sync", "--window-size=1280,900", "about:blank"]);
  const [debugPort, endpoint] = await readDevToolsPort(profile, chromium); cdp = await connect(`ws://127.0.0.1:${debugPort}${endpoint}`);
  report.browser = (await cdp.send("Browser.getVersion")).product;
  const workerTarget = await until(async () => (await cdp.send("Target.getTargets")).targetInfos.find(target => target.type === "service_worker" && target.url === `chrome-extension://${extensionId}/service-worker.js`), "candidate adapter worker");
  const { sessionId: workerSession } = await cdp.send("Target.attachToTarget", { targetId: workerTarget.targetId, flatten: true });
  const worker = expression => evaluate(cdp, expression, workerSession);
  await until(() => worker("Boolean(globalThis.ghostlightPageRuntime?.sha256)"), "real native transport installs acknowledged page runtime");
  report.page_runtime = await worker("globalThis.ghostlightPageRuntime");
  nativeProcesses = registry("processes"); if (!Array.isArray(nativeProcesses)) nativeProcesses = [nativeProcesses];
  const connector = nativeProcesses.find(item => item.path.toLowerCase() === executable("ghostlight-browser-connector").toLowerCase());
  assert.ok(connector, "Chrome must launch the copied native connector");
  const nativeAncestry = registry("ancestry", connector.id, profile);
  report.native_processes = nativeProcesses; report.native_ancestry = nativeAncestry.chain; save();
  assert.ok(nativeAncestry.chain.some(item=>item.owned_profile && item.path.toLowerCase() === browser.toLowerCase()),
    "Native connector must descend from the exact Chrome binary owning this disposable profile");
  const intermediate = nativeAncestry.chain.slice(1, -1);
  assert.ok(intermediate.every(item=>item.path && [browser.toLowerCase(), join(process.env.SystemRoot, "System32/cmd.exe").toLowerCase()].includes(item.path.toLowerCase())),
    "Only exact Chrome and the Windows native-host command wrapper may connect the host ancestry");
  check("real Chrome native messaging launches exact copied connector and ordinary isolated authority");
  const mcpConnector = start(executable("ghostlight-mcp-connector"));
  const mcp = channel(message => mcpConnector.stdin.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n"));
  createInterface({ input: mcpConnector.stdout }).on("line", line => mcp.receive(JSON.parse(line)));
  await mcp.send("initialize", { protocolVersion: "2025-11-25", capabilities: {}, clientInfo: { name: "quiet-native-installed-journey", version: "1" } });
  mcpConnector.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
  const call = async (name, args) => { const result = (await mcp.send("tools/call", { name, arguments: args })).structuredContent;
    report.receipts.push({ edge: "mcp", tool: name, result }); save(); return result; };
  const cli = async (name, args) => { const child = start(executable("ghostlight"), ["call", name, JSON.stringify(args), "--json"]);
    let output = ""; child.stdout.on("data", bytes => { output += bytes; }); await until(() => child.exitCode !== null || child.startError, "candidate CLI call");
    if (child.startError) throw child.startError; const result = JSON.parse(output); report.receipts.push({ edge: "cli", tool: name, result }); save(); return { code: child.exitCode, result }; };
  const human = await worker(`chrome.tabs.create({url:${JSON.stringify(`${origin}/human`)},active:true})`);
  await until(async () => (await worker(`chrome.tabs.get(${human.id})`)).status === "complete", "human page complete");
  const group = await worker(`chrome.tabs.group({tabIds:[${human.id}]})`);
  await worker(`chrome.tabGroups.update(${group},{title:'Human work',color:'green',collapsed:false})`);
  await worker(`chrome.windows.update(${human.windowId},{focused:true})`);
  const humanTarget = await until(async () => (await cdp.send("Target.getTargets")).targetInfos.find(target => target.url === `${origin}/human`), "human CDP page");
  const { sessionId: humanSession } = await cdp.send("Target.attachToTarget", { targetId: humanTarget.targetId, flatten: true });
  const humanPage = expression => evaluate(cdp, expression, humanSession);
  await humanPage("(()=>{human.focus();return true})()");
  const state = () => worker(`Promise.all([chrome.tabs.get(${human.id}),chrome.windows.getAll(),chrome.tabGroups.get(${group})]).then(([tab,windows,group])=>({url:tab.url,active:tab.active,window:tab.windowId,group:tab.groupId,focused:windows.filter(item=>item.focused).map(item=>item.id),group_title:group.title,group_collapsed:group.collapsed}))`);
  const baseline = await state(); report.observations.push({ name: "human_before", ...baseline });
  await worker("(()=>{globalThis.quietActivations=[];globalThis.quietWindowFocuses=[];chrome.tabs.onActivated.addListener(info=>quietActivations.push(info));chrome.windows.onFocusChanged.addListener(id=>quietWindowFocuses.push(id));return true})()");
  const fragments = ["Human ", "typing ", "survives ", "native ", "agent ", "work."];
  const typing = (async () => { for (const text of fragments) { await cdp.send("Input.insertText", { text }, humanSession); await delay(100); } })();
  const opened = await call("browser_navigate", { url: `${origin}/agent` }); assert.equal(opened.status, "succeeded", JSON.stringify(opened));
  assert.equal(opened.facts.browser_attention.value, "background"); const tab = opened.facts.tab;
  const physical = await worker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/agent`)}).id`); assert.notEqual(physical, human.id);
  const reused = await call("browser_navigate", { url: `${origin}/reused` }); assert.equal(reused.status, "succeeded", JSON.stringify(reused)); assert.equal(reused.facts.tab, tab);
  const filled = await call("browser_fill_form", { tab, fields: [{ selector: { name: "Agent draft", role: "textbox", exact: true }, value: "Native retained" }] });
  const agentPage = () => worker(`chrome.scripting.executeScript({target:{tabId:${physical}},world:'MAIN',func:()=>({value:agent.value,focus:document.hasFocus(),visibility:document.visibilityState,active:document.activeElement.id})}).then(items=>items[0].result)`);
  report.observations.push({ name: "agent_after_fill", ...(await agentPage()) }); save(); assert.equal(filled.status, "succeeded", JSON.stringify(filled));
  await typing; const typed = await humanPage("({value:human.value,active:document.activeElement.id,trusted:inputs.every(Boolean),events:inputs.length})");
  assert.equal(typed.value, fragments.join("")); assert.equal(typed.active, "human"); assert.equal(typed.trusted, true); assert.ok(typed.events >= fragments.length);
  assert.deepEqual(await state(), baseline); assert.deepEqual(await worker(`quietActivations.filter(item=>item.windowId===${human.windowId})`), []); assert.deepEqual(await worker("quietWindowFocuses"), []);
  report.observations.push({ name: "human_after", ...typed, ...(await state()) }); check("background open, owned reuse and fill preserve unowned human typing, URL, selection, window and group");
  const foreignCliRead = await cli("browser_read", { tab });
  assert.notEqual(foreignCliRead.code, 0); assert.equal(foreignCliRead.result.effect, "none");
  assert.equal(foreignCliRead.result.facts.reason, "ownership_mismatch");
  const cliOpened = await cli("browser_navigate", { url: `${origin}/cli`, new_tab: true });
  assert.equal(cliOpened.code, 0); assert.equal(cliOpened.result.status, "succeeded"); const cliTab = cliOpened.result.facts.tab;
  const cliPhysical = await worker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/cli`)}).id`);
  const cliRead = await cli("browser_read", { tab: cliTab }); assert.equal(cliRead.code, 0); assert.equal(cliRead.result.status, "succeeded"); assert.equal(cliRead.result.facts.browser_attention.value, "background");
  for (const edge of ["mcp", "cli"]) { const focused = edge === "mcp" ? await call("browser_tabs", { action: "focus", tab }) : (await cli("browser_tabs", { action: "focus", tab: cliTab })).result;
    assert.equal(focused.status, "blocked", JSON.stringify(focused)); assert.equal(focused.effect, "none"); assert.equal(focused.facts.reason, "browser_attention_background"); }
  assert.deepEqual(await state(), baseline); check("both real intake edges obey operator attention rule through native transport");
  const custodyOpened = await call("browser_navigate", { url: `${origin}/custody-sentinel`, new_tab: true });
  assert.equal(custodyOpened.status, "succeeded", JSON.stringify(custodyOpened));
  const custodyPhysical = await worker(`(await chrome.tabs.query({})).find(item=>item.url===${JSON.stringify(`${origin}/custody-sentinel`)}).id`);
  const custodyPage = () => worker(`chrome.scripting.executeScript({target:{tabId:${custodyPhysical}},world:'MAIN',func:()=>({focus:document.hasFocus(),visibility:document.visibilityState})}).then(items=>items[0].result)`);
  const webviewTarget = await until(async () => { try { const items = await (await fetch(`http://127.0.0.1:${webviewPort}/json/list`)).json(); return items.find(item=>item.type === "page" && item.webSocketDebuggerUrl); } catch { return false; } }, "production WebView2 debug endpoint");
  const webview = await connect(webviewTarget.webSocketDebuggerUrl);
  const ui = expression => evaluate(webview, expression);
  const captureWorkbench = async name => {
    const screenshot = await webview.send("Page.captureScreenshot", { format: "png" });
    const bytes = Buffer.from(screenshot.data, "base64"), path = evidence.replace(/\.json$/, `-${name}.png`);
    writeFileSync(path, bytes); report.artifacts.push({ path, sha256: hash(bytes), content: `Synthetic installed Workbench ${name}` }); save();
  };
  await until(() => ui("Boolean(window.__TAURI__?.core?.invoke && document.querySelector('[data-intent]'))"), "production workbench loaded");
  report.webview = { target_url: webviewTarget.url, debugger_port: webviewPort, product: (await webview.send("Browser.getVersion")).product }; save();
  // Click actual production buttons. No test double or direct adapter reveal is used.
  const click = async selector => { await until(() => ui(`Boolean(document.querySelector(${JSON.stringify(selector)}) && !document.querySelector(${JSON.stringify(selector)}).disabled)`), `Workbench button ${selector}`);
    return ui(`(()=>{document.querySelector(${JSON.stringify(selector)}).click();return true})()`); };
  // Chrome's native Input.insertText can leave hasFocus true even after false+detach.
  // A separate inactive target with no native input proves actual emulation cleanup.
  assert.equal((await worker(`chrome.tabs.get(${cliPhysical})`)).windowId, (await worker(`chrome.tabs.get(${physical})`)).windowId);
  await worker(`chrome.tabs.update(${cliPhysical},{active:true})`);
  assert.equal((await worker(`chrome.tabs.get(${physical})`)).active, false);
  assert.equal((await worker(`chrome.tabs.get(${custodyPhysical})`)).active, false);
  await until(async () => (await custodyPage()).focus === true, "inactive never-native controlled page retains active focus emulation");
  report.observations.push({ name: "inactive_agent_before_pause", ...(await agentPage()), sentinel: await custodyPage() }); save();
  await captureWorkbench("active-effective-attention");
  await click('[data-intent="hold"]'); await until(() => worker("liveState.control_state==='held'"), "native Workbench pause reaches adapter");
  try {
    await until(async () => (await custodyPage()).focus === false, "pause removes inactive never-native controlled-page focus emulation");
    assert.equal((await agentPage()).visibility, "hidden");
  } finally {
    report.observations.push({ name: "inactive_agent_after_pause", ...(await agentPage()),
      sentinel: await custodyPage(),
      tab: await worker(`chrome.tabs.get(${physical})`),
      sibling: await worker(`chrome.tabs.get(${cliPhysical})`),
      control_state: await worker("liveState.control_state"),
      debugger_targets: await worker("chrome.debugger.getTargets()") }); save();
  }
  await captureWorkbench("paused");
  const paused = await call("browser_type_text", { tab, selector: { name: "Agent draft", role: "textbox" }, text: "MUST_NOT_TYPE" }); assert.equal(paused.effect, "none");
  const revealSelector = `[data-reveal-tab="${tab}"]`;
  await click(revealSelector); await until(async () => (await worker(`chrome.tabs.get(${physical})`)).active, "native Workbench Show tab selects exact owned tab");
  assert.equal((await worker(`chrome.windows.get((await chrome.tabs.get(${physical})).windowId)`)).focused, true);
  assert.equal(await worker("liveState.control_state"), "held"); assert.equal((await agentPage()).value, "Native retained");
  await captureWorkbench("paused-show-tab");
  const revealedPaused = await call("browser_type_text", { tab, selector: { name: "Agent draft", role: "textbox" }, text: "REVEAL_IS_NOT_PERMISSION" }); assert.equal(revealedPaused.effect, "none");
  await worker(`chrome.tabs.update(${human.id},{active:true})`); await worker(`chrome.windows.update(${human.windowId},{focused:true})`);
  await worker(`chrome.tabs.update(${cliPhysical},{active:true})`);
  assert.equal((await custodyPage()).focus, false, "reveal while paused must not restore controlled emulation");
  assert.equal((await agentPage()).visibility, "hidden");
  check("actual Workbench Show tab reveals while paused without permitting effects or restoring emulation");
  await click('[data-intent="resume"]'); await until(() => worker("liveState.control_state==='active'"), "native Workbench resume reaches adapter");
  await until(async () => (await custodyPage()).focus === true, "resume restores inactive never-native controlled focus without replay");
  assert.equal((await agentPage()).value, "Native retained"); assert.equal((await call("browser_read", { tab })).status, "succeeded");
  await click('[data-intent="end_session"]'); await until(() => worker("liveState.control_state==='ended'"), "native Workbench Stop reaches adapter");
  await until(async () => (await custodyPage()).focus === false, "stop removes inactive never-native focus emulation");
  const targetsAfterStop = await worker("chrome.debugger.getTargets()");
  for (const ownedPhysical of [physical, cliPhysical, custodyPhysical]) assert.equal(Boolean(targetsAfterStop.find(target=>target.tabId===ownedPhysical)?.attached), false);
  report.observations.push({ name: "stopped_native_page_and_sentinel", ...(await agentPage()), sentinel: await custodyPage(), owned_debuggers_detached: true }); save();
  const stopped = await call("browser_type_text", { tab, selector: { name: "Agent draft", role: "textbox" }, text: "MUST_NOT_REPLAY" }); assert.equal(stopped.status, "blocked"); assert.equal(stopped.effect, "none");
  assert.equal(stopped.facts.reason, "session_ended");
  await click(revealSelector); await until(async () => (await worker(`chrome.tabs.get(${physical})`)).active, "Show tab works after stop");
  assert.equal(await worker("liveState.control_state"), "ended");
  await worker(`chrome.tabs.update(${human.id},{active:true})`); await worker(`chrome.windows.update(${human.windowId},{focused:true})`);
  await worker(`chrome.tabs.update(${cliPhysical},{active:true})`);
  assert.equal((await custodyPage()).focus, false); assert.equal((await agentPage()).visibility, "hidden");
  assert.equal((await agentPage()).value, "Native retained"); assert.deepEqual(await state(), baseline);
  assert.equal(await humanPage("human.value"), fragments.join("")); check("native pause, resume, stop and post-stop reveal preserve work without replay or custody restoration");
  for (const [name, connection, sessionId] of [["human", cdp, humanSession], ["workbench", webview, undefined]]) {
    const screenshot = await connection.send("Page.captureScreenshot", { format: "png" }, sessionId);
    const bytes = Buffer.from(screenshot.data, "base64"), path = evidence.replace(/\.json$/, `-${name}.png`); writeFileSync(path, bytes);
    report.artifacts.push({ path, sha256: hash(bytes), content: `Synthetic installed ${name} fixture` });
  }
  journeyPassed = true;
} catch (error) { report.failure = String(error.stack || error); save(); throw error;
} finally {
  try {
    if (cdp) { try { await cdp.send("Browser.close"); } catch {} } for (const socket of sockets) socket.close();
    if (chromium) await waitForChromiumExit(chromium);
    for (const child of children.toReversed()) if (child.exitCode === null && child.signalCode === null) child.kill();
    await until(() => children.every(child => child.exitCode !== null || child.signalCode !== null || child.startError), "owned children exit");
    if (registered) { report.registration_cleanup = registry("cleanup"); registered = false; }
    report.registrations_after = registry("snapshot");
    if (report.registrations_before) assert.deepEqual(report.registrations_after, report.registrations_before, "Existing registrations and native manifest hashes must stay identical");
    await until(() => { const items = registry("processes"); return !items || (Array.isArray(items) && !items.length); }, "Chrome-owned copied native host exits", 15000);
    if (existsSync(environment.GHOSTLIGHT_AUDIT_FILE)) { const path = evidence.replace(/\.json$/, "-audit.jsonl"); cpSync(environment.GHOSTLIGHT_AUDIT_FILE, path); report.artifacts.push({ path, sha256: hash(readFileSync(path)), content: "Synthetic candidate receipts" }); }
    server.closeAllConnections(); if (server.listening) await new Promise(done => server.close(done));
    assert.equal(dirname(resolve(scratch)), tempRoot); assert.ok(scratch.startsWith(join(tempRoot, "ghostlight-quiet-native-")));
    rmSync(scratch, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
    report.source_sha256_after = sourceFingerprint();
    assert.equal(report.source_sha256_after, report.source_sha256, "Candidate source must remain unchanged during installed verification");
    report.passed = journeyPassed; if (journeyPassed) delete report.failure;
  } catch (error) { report.passed = false; report.cleanup_failure = String(error.stack || error); throw error;
  } finally { report.finished_at = new Date().toISOString(); save(); console.log(`Evidence: ${evidence}`); }
}
