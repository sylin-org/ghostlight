// Throwaway probe of the facts a plain capture relies on. No Ghostlight process is involved.
// Questions, each asked of a real headless Chrome with a temporary profile:
//   1. Is a plain Page.captureScreenshot exactly innerWidth x innerHeight times devicePixelRatio?
//   2. Is the scrollbar strip inside the image, and on which side for a right-to-left page?
//   3. Does that hold at a device scale factor other than 1 and at a tab zoom other than 100%?
//   4. Does a plain capture show a change made to the top document just before it?
//   5. Does it show a change made inside a cross-process frame just before it?
//   6. Do OffscreenCanvas, createImageBitmap with a source rectangle and convertToBlob work in a worker?
import http from "node:http";
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";

const CHROME = process.argv[2];
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

let PORT = 0;
const S1 = () => `http://localhost:${PORT}`;
const S2 = () => `http://127.0.0.1:${PORT}`;

function handler(req, res) {
  const url = new URL(req.url, "http://x");
  const q = url.searchParams;
  let body = null;
  if (url.pathname === "/tall") {
    // A page taller than the window, so a classic vertical scrollbar exists. Content is solid red.
    body = `<!doctype html><html dir="${q.get("dir") ?? "ltr"}"><head><style>
html,body{margin:0;background:#ff0000}#fill{height:3000px}</style></head><body dir="${q.get("dir") ?? "ltr"}"><div id="fill"></div></body></html>`;
  } else if (url.pathname === "/host") {
    body = `<!doctype html><html><head><style>html,body{margin:0;height:100%;background:#fff;overflow:hidden}
iframe{position:absolute;left:0;top:0;width:100%;height:100%;border:0}</style></head><body><iframe src="${S2()}/child"></iframe></body></html>`;
  } else if (url.pathname === "/child") {
    body = `<!doctype html><html><body style="margin:0;background:#0000ff"></body></html>`;
  }
  if (body === null) {
    res.writeHead(404);
    res.end();
    return;
  }
  res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  res.end(body);
}

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
  const bpp = colorType === 6 ? 4 : 3;
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

// The horizontal extent of pixels matching `test` along the image's middle row.
function span(img, test) {
  const y = Math.floor(img.height / 2);
  let first = -1;
  let last = -1;
  for (let x = 0; x < img.width; x += 1) {
    const i = (y * img.width + x) * img.bpp;
    if (test(img.data[i], img.data[i + 1], img.data[i + 2])) {
      if (first < 0) first = x;
      last = x;
    }
  }
  return first < 0 ? "none" : `${first}..${last} (${last - first + 1} px)`;
}
const isRed = (r, g, b) => r > 200 && g < 60 && b < 60;
const isBlue = (r, g, b) => r < 60 && g < 60 && b > 200;
const isGreen = (r, g, b) => r < 60 && g > 200 && b < 60;

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
        if (msg.error) waiter.reject(new Error(msg.error.message));
        else waiter.resolve(msg.result);
      } else for (const listener of this.listeners) listener(msg);
    });
  }

  send(method, params = {}, sessionId = undefined) {
    const id = this.next;
    this.next += 1;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
}

async function evaluate(cdp, sid, expression) {
  const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true }, sid);
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? "page threw");
  return result.result.value;
}

async function run(scale) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), "chrome-probe-"));
  const args = ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run",
    "--no-default-browser-check", "--window-size=1200,800", "--disable-extensions", "about:blank"];
  if (scale !== 1) args.splice(1, 0, `--force-device-scale-factor=${scale}`);
  const chrome = spawn(CHROME, args, { stdio: "ignore" });
  try {
    const portFile = path.join(profile, "DevToolsActivePort");
    for (let i = 0; i < 100 && !fs.existsSync(portFile); i += 1) await sleep(100);
    const [debugPort, wsPath] = fs.readFileSync(portFile, "utf8").trim().split(/\r?\n/);
    const ws = new WebSocket(`ws://127.0.0.1:${debugPort}${wsPath}`);
    await new Promise((resolve, reject) => {
      ws.addEventListener("open", resolve);
      ws.addEventListener("error", reject);
    });
    const cdp = new Cdp(ws);
    console.log(`\n=== device scale factor ${scale} ===`);

    for (const dir of ["ltr", "rtl"]) {
      const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
      const { sessionId: sid } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
      await cdp.send("Page.enable", {}, sid);
      await cdp.send("Page.navigate", { url: `${S1()}/tall?dir=${dir}` }, sid);
      await sleep(1200);
      const facts = await evaluate(cdp, sid, `({dpr: devicePixelRatio, inner: [innerWidth, innerHeight], client: [document.documentElement.clientWidth, document.documentElement.clientHeight], dir: getComputedStyle(document.body).direction})`);
      const metrics = await cdp.send("Page.getLayoutMetrics", {}, sid);
      const shot = decodePng(Buffer.from((await cdp.send("Page.captureScreenshot", { format: "png" }, sid)).data, "base64"));
      const visual = metrics.cssVisualViewport;
      console.log(`${dir}: dpr ${facts.dpr}, inner ${facts.inner}, client ${facts.client}, visual ${visual.clientWidth}x${visual.clientHeight} scale ${visual.scale}`);
      console.log(`   plain image ${shot.width}x${shot.height}; inner*dpr = ${facts.inner[0] * facts.dpr}x${facts.inner[1] * facts.dpr}; content (red) spans ${span(shot, isRed)}`);
      await cdp.send("Target.closeTarget", { targetId });
    }

    // Freshness: a change made just before the capture, in the top document and in a cross-process frame.
    const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
    const { sessionId: root } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
    let child = null;
    cdp.listeners.add((msg) => {
      if (msg.method === "Target.attachedToTarget" && msg.sessionId === root) child = msg.params.sessionId;
    });
    await cdp.send("Page.enable", {}, root);
    await cdp.send("Target.setAutoAttach", { autoAttach: true, waitForDebuggerOnStart: false, flatten: true, filter: [{ type: "iframe", exclude: false }] }, root);
    await cdp.send("Page.navigate", { url: `${S1()}/host` }, root);
    await sleep(1500);
    let fresh = 0;
    let stale = 0;
    for (let round = 0; round < 6; round += 1) {
      const color = round % 2 === 0 ? "#00ff00" : "#0000ff";
      await evaluate(cdp, child, `document.body.style.background = "${color}"`);
      const shot = decodePng(Buffer.from((await cdp.send("Page.captureScreenshot", { format: "png" }, root)).data, "base64"));
      const shows = span(shot, round % 2 === 0 ? isGreen : isBlue) !== "none";
      if (shows) fresh += 1;
      else stale += 1;
      await sleep(300);
    }
    console.log(`change inside a cross-process frame just before a plain capture: shown ${fresh} of 6, not yet shown ${stale} of 6 (child session ${child ? "attached" : "MISSING"})`);
    let topFresh = 0;
    for (let round = 0; round < 6; round += 1) {
      const color = round % 2 === 0 ? "#ff0000" : "#ffffff";
      await evaluate(cdp, root, `document.querySelector("iframe").style.display = "none"; document.body.style.background = "${color}"`);
      const shot = decodePng(Buffer.from((await cdp.send("Page.captureScreenshot", { format: "png" }, root)).data, "base64"));
      if (round % 2 === 0 && span(shot, isRed) !== "none") topFresh += 1;
      if (round % 2 === 1 && span(shot, isRed) === "none") topFresh += 1;
      await sleep(200);
    }
    console.log(`change in the top document just before a plain capture: shown ${topFresh} of 6`);

    if (scale === 1) {
      // Tab zoom: the browser zoom factor, set the way a person's Ctrl+Plus does it, changes devicePixelRatio.
      const tools = await evaluate(cdp, root, `(async () => {
        const source = \`onmessage = async (event) => {
          try {
            const bitmap = await createImageBitmap(event.data, 10, 20, 100, 50, { resizeWidth: 50, resizeHeight: 25, resizeQuality: "high" });
            const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
            canvas.getContext("2d").drawImage(bitmap, 0, 0);
            bitmap.close();
            const blob = await canvas.convertToBlob({ type: "image/jpeg", quality: 0.8 });
            postMessage({ ok: true, type: blob.type, size: blob.size, width: canvas.width, height: canvas.height });
          } catch (error) { postMessage({ ok: false, error: String(error) }); }
        };\`;
        const worker = new Worker(URL.createObjectURL(new Blob([source], { type: "text/javascript" })));
        const canvas = new OffscreenCanvas(300, 200);
        const context = canvas.getContext("2d");
        context.fillStyle = "#123456";
        context.fillRect(0, 0, 300, 200);
        const png = await canvas.convertToBlob({ type: "image/png" });
        return await new Promise((resolve) => { worker.onmessage = (event) => resolve(event.data); worker.postMessage(png); });
      })()`);
      console.log(`crop, scale and JPEG encode inside a worker: ${JSON.stringify(tools)}`);
    }
    ws.close();
  } finally {
    chrome.kill();
    await sleep(500);
    fs.rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 300 });
  }
}

const servers = [];
try {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  servers.push(server);
  PORT = server.address().port;
  for (const scale of [1, 1.5, 2]) await run(scale);
} finally {
  for (const server of servers) server.close();
}
