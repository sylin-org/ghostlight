// Throwaway probe of Chrome behavior. No Ghostlight process is involved.
// It launches a headless Chrome with a temporary profile, serves a three-site fixture shaped
// like an Office-in-SharePoint page, and asks two questions over raw CDP:
//   A. Does a clipped capture with captureBeyondViewport squeeze a cross-process frame, and is
//      a plain capture immune?
//   B. After the top page navigates away, which auto-attached child sessions never get a
//      Target.detachedFromTarget, and what do they answer to Page.getFrameTree?
import http from "node:http";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";

const CHROME = process.argv[2];
const ONLY = process.argv[3] ?? "all";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const t0 = Date.now();
const stamp = () => String(Date.now() - t0).padStart(6, " ");

// ---- fixture -----------------------------------------------------------------------------------
let PORT = 0;
let S1 = "";
let S2 = "";
let S3 = "";

function page(req) {
  const url = new URL(req.url, "http://x");
  const q = url.searchParams;
  switch (url.pathname) {
    case "/top":
      return `<!doctype html><html><head><title>top</title><style>
html,body{margin:0;height:100%;background:#fff;overflow:hidden}
iframe{position:absolute;left:0;top:0;width:100%;height:100%;min-width:300px;min-height:200px;border:0}
</style></head><body><iframe id="app" src="${S2}/app?busy=${q.get("busy") ?? 0}&churn=${q.get("churn") ?? 0}"></iframe>
<script>${q.get("unload") === "1" ? "addEventListener('unload', () => {});" : ""}</script></body></html>`;
    case "/app":
      return `<!doctype html><html><head><title>app</title><style>
html,body{margin:0;height:100%;background:#0000ff}
#inner{position:absolute;right:20px;bottom:20px;width:200px;height:100px;border:0}
</style></head><body><iframe id="inner" src="${S3}/inner"></iframe><script>
const busy = ${Number(q.get("busy") ?? 0)};
if (busy) addEventListener("resize", () => { const t = performance.now(); while (performance.now() - t < busy) {} });
if (${q.get("churn") === "1"}) setInterval(() => {
  const f = document.createElement("iframe");
  f.style.display = "none";
  f.src = "${S3}/redir?to=" + encodeURIComponent(location.origin + "/blank");
  document.body.appendChild(f);
  setTimeout(() => f.remove(), 250);
}, 400);
</script></body></html>`;
    case "/inner":
      return `<!doctype html><title>inner</title><body style="margin:0;background:#00ff00">inner</body>`;
    case "/blank":
      return `<!doctype html><title>blank</title><body></body>`;
    case "/next":
      return `<!doctype html><title>next</title><body style="background:#fff">next ${q.get("i") ?? ""}</body>`;
    default:
      return null;
  }
}

function handler(req, res) {
  const url = new URL(req.url, "http://x");
  if (url.pathname === "/redir") {
    res.writeHead(302, { location: url.searchParams.get("to") });
    res.end();
    return;
  }
  const body = page(req);
  if (body === null) {
    res.writeHead(404);
    res.end();
    return;
  }
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end(body);
}

async function listen(host, port) {
  const server = http.createServer(handler);
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, resolve);
  });
  return server;
}

// ---- png ---------------------------------------------------------------------------------------
function decodePng(buf) {
  let off = 8;
  let width = 0;
  let height = 0;
  let colorType = 0;
  const idat = [];
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      colorType = data[9];
    } else if (type === "IDAT") idat.push(data);
    off += 12 + len;
  }
  const bpp = colorType === 6 ? 4 : colorType === 2 ? 3 : 0;
  if (!bpp) throw new Error(`unsupported png color type ${colorType}`);
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  const out = Buffer.alloc(height * stride);
  let prev = Buffer.alloc(stride);
  for (let y = 0; y < height; y += 1) {
    const f = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const cur = out.subarray(y * stride, (y + 1) * stride);
    for (let x = 0; x < stride; x += 1) {
      const a = x >= bpp ? cur[x - bpp] : 0;
      const b = prev[x];
      const c = x >= bpp ? prev[x - bpp] : 0;
      let v = line[x];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      cur[x] = v & 255;
    }
    prev = cur;
  }
  return { width, height, bpp, data: out };
}

// Where the app frame's blue shows in a capture: its share of the image and its bounding box.
function blueReport(base64) {
  const img = decodePng(Buffer.from(base64, "base64"));
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -1;
  let maxY = -1;
  let count = 0;
  for (let y = 0; y < img.height; y += 1) {
    for (let x = 0; x < img.width; x += 1) {
      const i = (y * img.width + x) * img.bpp;
      if (img.data[i] < 60 && img.data[i + 1] < 60 && img.data[i + 2] > 200) {
        count += 1;
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }
  const share = Math.round((1000 * count) / (img.width * img.height)) / 10;
  const box = count ? `${maxX - minX + 1}x${maxY - minY + 1}` : "none";
  return `image ${img.width}x${img.height}, app frame visible as ${box} (${share}% of image)`;
}

// ---- cdp ---------------------------------------------------------------------------------------
class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.next = 1;
    this.pending = new Map();
    this.listeners = new Set();
    ws.addEventListener("message", (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id !== undefined) {
        const waiter = this.pending.get(msg.id);
        if (!waiter) return;
        this.pending.delete(msg.id);
        clearTimeout(waiter.timer);
        if (msg.error) waiter.reject(new Error(msg.error.message));
        else waiter.resolve(msg.result);
      } else for (const listener of this.listeners) listener(msg);
    });
  }

  send(method, params = {}, sessionId = undefined, timeoutMs = 5000) {
    const id = this.next;
    this.next += 1;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`no answer in ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
}

const AUTO = { autoAttach: true, waitForDebuggerOnStart: true, flatten: true, filter: [{ type: "iframe", exclude: false }] };

// One page target with the same registry the extension keeps: every auto-attached iframe session,
// which session announced it, and whether a detach event ever named it.
async function openTab(cdp, label) {
  const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
  const { sessionId: root } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
  const sessions = new Map();
  const events = [];
  const listener = (msg) => {
    if (msg.method === "Target.attachedToTarget") {
      const info = msg.params.targetInfo;
      const known = msg.sessionId === root || sessions.has(msg.sessionId);
      if (!known) return;
      const via = msg.sessionId === root ? "root" : short(msg.sessionId);
      sessions.set(msg.params.sessionId, { via, url: info.url, type: info.type, detached: false });
      events.push(`${stamp()} attached ${short(msg.params.sessionId)} via ${via} ${info.type} ${trim(info.url)}`);
      setUp(msg.params.sessionId, msg.params.waitingForDebugger);
    } else if (msg.method === "Target.detachedFromTarget") {
      const entry = sessions.get(msg.params.sessionId);
      if (!entry) return;
      entry.detached = true;
      const via = msg.sessionId === root ? "root" : short(msg.sessionId);
      events.push(`${stamp()} detached ${short(msg.params.sessionId)} said by ${via}`);
    }
  };
  async function setUp(sid, waiting) {
    try {
      await cdp.send("Page.enable", {}, sid);
      await cdp.send("Target.setAutoAttach", AUTO, sid);
    } catch (error) {
      events.push(`${stamp()} setup of ${short(sid)} failed: ${error.message}`);
    } finally {
      if (waiting) await cdp.send("Runtime.runIfWaitingForDebugger", {}, sid).catch(() => {});
    }
  }
  cdp.listeners.add(listener);
  await cdp.send("Page.enable", {}, root);
  await cdp.send("Target.setAutoAttach", AUTO, root);
  return {
    label,
    root,
    targetId,
    sessions,
    events,
    async go(url, settleMs = 1500) {
      await cdp.send("Page.navigate", { url }, root, 15000);
      await sleep(settleMs);
    },
    // What every still-registered session answers to the inventory's own question.
    async audit() {
      const lines = [];
      for (const [sid, entry] of sessions) {
        if (entry.detached) continue;
        let answer;
        try {
          const tree = (await cdp.send("Page.getFrameTree", {}, sid, 3000)).frameTree;
          answer = `answers, frame ${trim(tree.frame.url)}`;
        } catch (error) {
          answer = `FAILS: ${error.message}`;
        }
        lines.push(`  still registered ${short(sid)} (via ${entry.via}, ${trim(entry.url)}): ${answer}`);
      }
      return lines.length ? lines : ["  no session left registered"];
    },
    async close() {
      cdp.listeners.delete(listener);
      await cdp.send("Target.closeTarget", { targetId }).catch(() => {});
    }
  };
}

const short = (sid) => String(sid).slice(0, 6);
const trim = (url) => String(url).replace(/^https?:\/\//, "").slice(0, 60);

// ---- experiments -------------------------------------------------------------------------------
async function captureExperiment(cdp) {
  console.log("\n=== A. capture: clipped+beyondViewport (as Ghostlight asks) vs plain ===");
  for (const busy of [0, 120, 400]) {
    const tab = await openTab(cdp, `capture busy=${busy}`);
    await tab.go(`${S1}/top?busy=${busy}`, 2000);
    const metrics = await cdp.send("Page.getLayoutMetrics", {}, tab.root);
    const visual = metrics.cssVisualViewport;
    const clip = { x: visual.pageX, y: visual.pageY, width: visual.clientWidth, height: visual.clientHeight, scale: 1 };
    console.log(`app frame takes ${busy} ms to handle a resize; viewport ${visual.clientWidth}x${visual.clientHeight}`);
    for (let round = 1; round <= 2; round += 1) {
      const plain = await cdp.send("Page.captureScreenshot", { format: "png" }, tab.root, 20000);
      console.log(`  plain            #${round}: ${blueReport(plain.data)}`);
      await sleep(1200);
      const ghost = await cdp.send("Page.captureScreenshot",
        { format: "png", clip, captureBeyondViewport: true, fromSurface: true }, tab.root, 20000);
      console.log(`  clip+beyond      #${round}: ${blueReport(ghost.data)}`);
      await sleep(1200);
      const clipped = await cdp.send("Page.captureScreenshot", { format: "png", clip, fromSurface: true }, tab.root, 20000);
      console.log(`  clip only        #${round}: ${blueReport(clipped.data)}`);
      await sleep(1200);
    }
    await tab.close();
  }
}

async function sessionExperiment(cdp) {
  console.log("\n=== B. sessions left registered after the top page navigates away ===");
  const scenarios = [
    ["nested frames, then a plain page", `${S1}/top`, `${S1}/next`],
    ["nested frames with an unload handler, then a plain page", `${S1}/top?unload=1`, `${S1}/next`],
    ["nested frames, then another page with nested frames", `${S1}/top`, `${S1}/top?v=2`],
    ["nested frames plus short-lived cross-site frames, then a plain page", `${S1}/top?churn=1`, `${S1}/next`],
    ["nested frames, then a cross-site plain page", `${S1}/top`, `${S2}/next`]
  ];
  for (const [name, first, second] of scenarios) {
    const tab = await openTab(cdp, name);
    await tab.go(first, 2500);
    const before = Array.from(tab.sessions.values()).filter((entry) => !entry.detached).length;
    await tab.go(second, 2500);
    console.log(`\n-- ${name}: ${before} child sessions before leaving`);
    for (const line of await tab.audit()) console.log(line);
    // Push the first page out of any back/forward cache, then ask again.
    for (let i = 0; i < 8; i += 1) await tab.go(`${S1}/next?i=${i}`, 250);
    console.log("   after 8 more plain pages:");
    for (const line of await tab.audit()) console.log(line);
    if (process.argv.includes("--events")) for (const event of tab.events) console.log(`     ${event}`);
    await tab.close();
  }
}

// ---- main --------------------------------------------------------------------------------------
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "chrome-probe-"));
let chrome = null;
const servers = [];
try {
  const first = await listen("127.0.0.1", 0);
  servers.push(first);
  PORT = first.address().port;
  S1 = `http://localhost:${PORT}`;
  S2 = `http://127.0.0.1:${PORT}`;
  try {
    servers.push(await listen("::1", PORT));
    S3 = `http://[::1]:${PORT}`;
  } catch (_error) {
    S3 = S1;
  }
  console.log(`sites: top ${S1}, app ${S2}, inner ${S3}`);

  chrome = spawn(CHROME, [
    "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run",
    "--no-default-browser-check", "--window-size=1200,800", "--disable-extensions", "about:blank"
  ], { stdio: "ignore" });
  const portFile = path.join(profile, "DevToolsActivePort");
  for (let i = 0; i < 100 && !fs.existsSync(portFile); i += 1) await sleep(100);
  const [debugPort, wsPath] = fs.readFileSync(portFile, "utf8").trim().split(/\r?\n/);
  const ws = new WebSocket(`ws://127.0.0.1:${debugPort}${wsPath}`);
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve);
    ws.addEventListener("error", reject);
  });
  const cdp = new Cdp(ws);
  const version = await cdp.send("Browser.getVersion");
  console.log(`browser: ${version.product}`);
  if (ONLY === "all" || ONLY === "capture") await captureExperiment(cdp);
  if (ONLY === "all" || ONLY === "sessions") await sessionExperiment(cdp);
  ws.close();
} finally {
  if (chrome) chrome.kill();
  for (const server of servers) server.close();
  await sleep(500);
  fs.rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
}
