#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- a dependency-free JavaScript lint for unused bindings and undefined names.
//
// It parses with the vendored acorn parser (no npm install), builds lexical scopes, and reports
// two things: a binding that is never read, and a name that is read but never declared and is not
// in the globals allowlist of the file's kind. Usage: node scripts/lint-js.mjs <file-or-dir>...
// Exit status is 1 when anything is reported, with one `file:line:col: message` line each.
//
// Kinds, chosen from the path:
//   module  `.mjs` files: ES modules running under node.
//   node    `*.test.js` files and anything under a `tests` directory: CommonJS under node.
//   page    the protocol-4 page assets: one shared scope per directory, because the service
//           concatenates them into a single function body.
//   extpage the extension's document scripts: the popup, options and offscreen pages and the
//           scripts only their .html files load. They may read the document names and chrome.
//           A script the service worker also imports (lib/state.js, lib/recording.js) takes the
//           worker kind, because it must run where there is no document.
//   worker  every other extension script (service worker, libraries). No document names: a
//           service worker has no window or document.
//
// Allowed without a report: a name that starts with an underscore, an exported binding, a
// positional parameter before the last used one, a name guarded by `typeof`, and a destructured
// name beside a rest element. Assigning onto a member declares nothing, so it is never reported.

import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const acorn = require("../extension/vendor/acorn.js");

const IGNORED_PREFIX = "_";
const PAGE_ASSET_DIR = /chromium\/level4\/assets$/;
/** The extension scripts that run in a document, not in the service worker. */
const EXTENSION_PAGE_SCRIPT =
  /(^|\/)extension\/(offscreen\.js|options\.js|popup\.js|lib\/gif\.js|vendor\/gifenc\.js)$/;

/** Split a whitespace-separated word list. */
function words(text) {
  return text.trim().split(/\s+/);
}

/** Names every JavaScript realm defines. */
const ECMASCRIPT = words(`Array ArrayBuffer Atomics BigInt BigInt64Array BigUint64Array Boolean DataView Date
  Error EvalError FinalizationRegistry Float32Array Float64Array Function Infinity Int8Array Int16Array Int32Array
  Intl JSON Map Math NaN Number Object Promise Proxy RangeError ReferenceError Reflect RegExp Set SharedArrayBuffer
  String Symbol SyntaxError TypeError URIError Uint8Array Uint8ClampedArray Uint16Array Uint32Array WeakMap WeakRef
  WeakSet AggregateError decodeURI decodeURIComponent encodeURI encodeURIComponent escape unescape eval isFinite
  isNaN parseFloat parseInt undefined globalThis`);

/** Web platform names shared by workers, pages and node. */
const WEB_COMMON = words(`console setTimeout clearTimeout setInterval clearInterval queueMicrotask structuredClone
  URL URLSearchParams TextEncoder TextDecoder AbortController AbortSignal Event EventTarget CustomEvent performance
  crypto atob btoa fetch Blob Headers Request Response FormData MessageChannel BroadcastChannel DOMException
  ReadableStream WritableStream TransformStream CompressionStream DecompressionStream WebSocket`);

/** Names a browser-extension script may read: the worker realm and CommonJS for the dual-mode modules. */
const WORKER = words(`chrome importScripts self navigator location createImageBitmap OffscreenCanvas ImageData
  FileReader WebSocket indexedDB caches clients registration WebAssembly module require exports`);

/** Names a document script may read, plus the page-asset wrapper argument. */
const DOCUMENT = words(`window document navigator location history screen Node Element HTMLElement
  HTMLInputElement HTMLTextAreaElement HTMLSelectElement HTMLFormElement HTMLAnchorElement HTMLImageElement
  HTMLIFrameElement HTMLButtonElement HTMLLabelElement HTMLOptionElement HTMLCanvasElement HTMLVideoElement
  HTMLMediaElement SVGElement ShadowRoot DocumentFragment Document Text Comment Range Selection MutationObserver
  IntersectionObserver ResizeObserver PerformanceObserver MouseEvent KeyboardEvent InputEvent FocusEvent PointerEvent
  WheelEvent DragEvent DataTransfer File FileList CSSStyleSheet CSS getComputedStyle requestAnimationFrame
  cancelAnimationFrame requestIdleCallback cancelIdleCallback matchMedia innerWidth innerHeight scrollX scrollY
  pageXOffset pageYOffset addEventListener removeEventListener dispatchEvent postMessage getSelection NodeFilter
  XPathResult scrollBy scrollTo scroll open close`);

/** The argument the extension's function wrapper passes to a page asset. */
const PAGE_WRAPPER = ["__ghostlightAssetId"];

/** Names a node script may read. */
const NODE = words(`require module exports __dirname __filename process Buffer setImmediate clearImmediate global`);
const COMMONJS_ONLY = new Set(["require", "module", "exports", "__dirname", "__filename"]);

const GLOBALS = {
  module: new Set([...ECMASCRIPT, ...WEB_COMMON, ...NODE].filter((n) => !COMMONJS_ONLY.has(n))),
  node: new Set([...ECMASCRIPT, ...WEB_COMMON, ...NODE]),
  page: new Set([...ECMASCRIPT, ...WEB_COMMON, ...DOCUMENT, ...PAGE_WRAPPER]),
  worker: new Set([...ECMASCRIPT, ...WEB_COMMON, ...WORKER]),
  extpage: new Set([...ECMASCRIPT, ...WEB_COMMON, ...WORKER, ...DOCUMENT])
};

/** One lexical scope: its bindings and its parent. */
class Scope {
  constructor(parent, { isFunction = false, isArrow = false } = {}) {
    this.parent = parent;
    this.isFunction = isFunction;
    this.isArrow = isArrow;
    this.bindings = new Map();
  }

  /** The nearest scope that owns `var` declarations. */
  functionScope() {
    let scope = this;
    while (!scope.isFunction && scope.parent) scope = scope.parent;
    return scope;
  }

  /** Whether a non-arrow function encloses this scope, so `arguments` exists. */
  hasArguments() {
    for (let scope = this; scope; scope = scope.parent) {
      if (scope.isFunction && !scope.isArrow) return scope.parent !== null;
    }
    return false;
  }
}

/** Declare `id` in `scope`, merging a repeated declaration of the same name. */
function declare(scope, id, extra) {
  const existing = scope.bindings.get(id.name);
  if (existing) {
    Object.assign(existing, extra);
    return existing;
  }
  const binding = { name: id.name, node: id, reads: 0, writes: 0, exempt: false, ...extra };
  scope.bindings.set(id.name, binding);
  return binding;
}

/** A message with its position, ready for sorting and printing. */
function report(file, node, text) {
  return { file, line: node.loc.start.line, column: node.loc.start.column + 1, text };
}

/** Lint one analysis unit: one or more parsed programs sharing a root scope. */
function analyze(programs, kind) {
  const root = new Scope(null, { isFunction: true });
  const references = [];
  const messages = [];
  const bindings = [];
  const functions = [];

  for (const { file, ast } of programs) {
    new Visitor(file, references, bindings, functions).statements(ast.body, root);
  }

  for (const r of references) {
    let binding = null;
    for (let scope = r.scope; scope && !binding; scope = scope.parent) binding = scope.bindings.get(r.id.name) ?? null;
    if (binding) {
      if (r.read) binding.reads += 1;
      if (r.write) binding.writes += 1;
      continue;
    }
    if (r.guarded || GLOBALS[kind].has(r.id.name)) continue;
    if (r.id.name === "arguments" && r.scope.hasArguments()) continue;
    messages.push(report(r.file, r.id, `'${r.id.name}' is not defined`));
  }

  const lastUsedParam = new Map();
  for (const record of functions) {
    let last = -1;
    record.params.forEach((b, i) => {
      if (b && b.reads > 0) last = i;
    });
    lastUsedParam.set(record, last);
  }
  for (const b of bindings) {
    if (b.reads > 0 || b.exempt || b.name.startsWith(IGNORED_PREFIX)) continue;
    if (b.paramOf && b.paramIndex <= lastUsedParam.get(b.paramOf)) continue;
    messages.push(report(b.file, b.node, `'${b.name}' is ${b.writes > 0 ? "assigned but never read" : "declared but never read"}`));
  }
  return messages;
}

/** The tree walker that builds scopes and records references. */
class Visitor {
  constructor(file, references, bindings, functions) {
    this.file = file;
    this.references = references;
    this.bindings = bindings;
    this.functions = functions;
  }

  ref(id, scope, flags) {
    this.references.push({ id, scope, read: false, write: false, guarded: false, ...flags, file: this.file });
  }

  bind(scope, id, extra = {}) {
    const binding = declare(scope, id, { file: this.file, ...extra });
    if (!this.bindings.includes(binding)) this.bindings.push(binding);
    return binding;
  }

  statements(list, scope) {
    for (const node of list) this.visit(node, scope);
  }

  /**
   * Walk a pattern. A declaration binds each identifier into `into`; an assignment records a write.
   * Default values and computed keys are visited as reads in `scope`. Returns the bindings made.
   */
  pattern(node, scope, into, extra, assignment) {
    const found = [];
    const walk = (p) => {
      switch (p.type) {
        case "Identifier":
          if (assignment) this.ref(p, scope, { write: true });
          else found.push(this.bind(into, p, extra));
          break;
        case "ObjectPattern": {
          const rest = p.properties.some((q) => q.type === "RestElement");
          for (const q of p.properties) {
            if (q.type === "RestElement") {
              walk(q.argument);
              continue;
            }
            if (q.computed) this.visit(q.key, scope);
            const before = found.length;
            walk(q.value);
            if (rest) for (const b of found.slice(before)) b.exempt = true;
          }
          break;
        }
        case "ArrayPattern":
          for (const e of p.elements) if (e) walk(e);
          break;
        case "RestElement":
          walk(p.argument);
          break;
        case "AssignmentPattern":
          walk(p.left);
          this.visit(p.right, scope);
          break;
        default:
          this.visit(p, scope);
      }
    };
    walk(node);
    return found;
  }

  fn(node, scope) {
    let outer = scope;
    if (node.type === "FunctionExpression" && node.id) {
      outer = new Scope(scope);
      this.bind(outer, node.id, { exempt: true });
    }
    const inner = new Scope(outer, { isFunction: true, isArrow: node.type === "ArrowFunctionExpression" });
    const record = { params: [] };
    this.functions.push(record);
    node.params.forEach((p, index) => {
      const simple = p.type === "Identifier" || (p.type === "AssignmentPattern" && p.left.type === "Identifier");
      const found = this.pattern(p, inner, inner, simple ? { paramOf: record, paramIndex: index } : {}, false);
      record.params.push(simple ? found[0] : null);
    });
    if (node.body.type === "BlockStatement") this.statements(node.body.body, inner);
    else this.visit(node.body, inner);
  }

  cls(node, scope) {
    let inner = scope;
    if (node.type === "ClassExpression" && node.id) {
      inner = new Scope(scope);
      this.bind(inner, node.id, { exempt: true });
    }
    this.visit(node.superClass, inner);
    for (const member of node.body.body) this.visit(member, inner);
  }

  assignment(node, scope) {
    if (node.left.type === "Identifier") {
      this.ref(node.left, scope, { read: node.operator !== "=" && !node.discarded, write: true });
    } else if (node.left.type === "MemberExpression") {
      this.visit(node.left, scope);
    } else {
      this.pattern(node.left, scope, scope, {}, true);
    }
    this.visit(node.right, scope);
  }

  visit(node, scope) {
    if (!node) return;
    switch (node.type) {
      case "Identifier":
        this.ref(node, scope, { read: true });
        return;
      case "VariableDeclaration": {
        const into = node.kind === "var" ? scope.functionScope() : scope;
        for (const d of node.declarations) {
          this.pattern(d.id, scope, into, { exempt: node.exported === true }, false);
          this.visit(d.init, scope);
        }
        return;
      }
      case "FunctionDeclaration":
        if (node.id) this.bind(scope, node.id, { exempt: node.exported === true });
        this.fn(node, scope);
        return;
      case "FunctionExpression":
      case "ArrowFunctionExpression":
        this.fn(node, scope);
        return;
      case "ClassDeclaration":
        if (node.id) this.bind(scope, node.id, { exempt: node.exported === true });
        this.cls(node, scope);
        return;
      case "ClassExpression":
        this.cls(node, scope);
        return;
      case "BlockStatement":
        this.statements(node.body, new Scope(scope));
        return;
      case "StaticBlock":
        this.statements(node.body, new Scope(scope, { isFunction: true }));
        return;
      case "ForStatement": {
        const inner = new Scope(scope);
        if (node.update?.type === "UpdateExpression" || node.update?.type === "AssignmentExpression") node.update.discarded = true;
        this.visit(node.init, inner);
        this.visit(node.test, inner);
        this.visit(node.update, inner);
        this.visit(node.body, inner);
        return;
      }
      case "ForInStatement":
      case "ForOfStatement": {
        const inner = new Scope(scope);
        if (node.left.type === "VariableDeclaration") {
          const into = node.left.kind === "var" ? scope.functionScope() : inner;
          for (const d of node.left.declarations) this.pattern(d.id, inner, into, {}, false);
        } else {
          this.pattern(node.left, inner, inner, {}, true);
        }
        this.visit(node.right, inner);
        this.visit(node.body, inner);
        return;
      }
      case "SwitchStatement": {
        this.visit(node.discriminant, scope);
        const inner = new Scope(scope);
        for (const c of node.cases) {
          this.visit(c.test, inner);
          this.statements(c.consequent, inner);
        }
        return;
      }
      case "CatchClause": {
        const inner = new Scope(scope);
        if (node.param) this.pattern(node.param, inner, inner, {}, false);
        this.statements(node.body.body, inner);
        return;
      }
      case "LabeledStatement":
        this.visit(node.body, scope);
        return;
      case "BreakStatement":
      case "ContinueStatement":
      case "MetaProperty":
        return;
      case "MemberExpression":
        this.visit(node.object, scope);
        if (node.computed) this.visit(node.property, scope);
        return;
      case "Property":
        if (node.computed) this.visit(node.key, scope);
        this.visit(node.value, scope);
        return;
      case "MethodDefinition":
        if (node.computed) this.visit(node.key, scope);
        this.visit(node.value, scope);
        return;
      case "PropertyDefinition":
        if (node.computed) this.visit(node.key, scope);
        this.visit(node.value, new Scope(scope, { isFunction: true }));
        return;
      case "UnaryExpression":
        if (node.operator === "typeof" && node.argument.type === "Identifier") {
          this.ref(node.argument, scope, { read: true, guarded: true });
        } else {
          this.visit(node.argument, scope);
        }
        return;
      case "AssignmentExpression":
        this.assignment(node, scope);
        return;
      case "UpdateExpression":
        if (node.argument.type === "Identifier") {
          this.ref(node.argument, scope, { read: !node.discarded, write: true });
        } else {
          this.visit(node.argument, scope);
        }
        return;
      case "ExpressionStatement":
        if (node.expression.type === "UpdateExpression" || node.expression.type === "AssignmentExpression") {
          node.expression.discarded = true;
        }
        this.visit(node.expression, scope);
        return;
      case "ExportNamedDeclaration":
        if (node.declaration) {
          node.declaration.exported = true;
          this.visit(node.declaration, scope);
        }
        if (!node.source) for (const s of node.specifiers) this.ref(s.local, scope, { read: true });
        return;
      case "ExportDefaultDeclaration":
        if (node.declaration.id) node.declaration.exported = true;
        this.visit(node.declaration, scope);
        return;
      case "ExportAllDeclaration":
        return;
      case "ImportDeclaration":
        for (const s of node.specifiers) this.bind(scope, s.local);
        return;
      default:
        this.children(node, scope);
    }
  }

  /** Visit every child node of a node with no special scoping. */
  children(node, scope) {
    for (const key of Object.keys(node)) {
      if (key === "type" || key === "loc" || key === "start" || key === "end") continue;
      const value = node[key];
      if (Array.isArray(value)) {
        for (const item of value) if (item && typeof item.type === "string") this.visit(item, scope);
      } else if (value && typeof value.type === "string") {
        this.visit(value, scope);
      }
    }
  }
}

/** Parse source text; throws the acorn error on a syntax error. */
function parse(text, kind) {
  return acorn.parse(text, {
    ecmaVersion: "latest",
    sourceType: kind === "module" ? "module" : "script",
    locations: true,
    allowReturnOutsideFunction: kind === "page",
    allowHashBang: true
  });
}

/** Choose the kind of a file from its path. */
export function kindOf(file) {
  const normalized = file.split(path.sep).join("/");
  if (normalized.endsWith(".mjs")) return "module";
  if (normalized.endsWith(".test.js") || /(^|\/)tests\//.test(normalized)) return "node";
  if (PAGE_ASSET_DIR.test(path.posix.dirname(normalized))) return "page";
  if (EXTENSION_PAGE_SCRIPT.test(normalized)) return "extpage";
  return "worker";
}

/**
 * Lint source texts. `sources` is a list of `{ file, text }`. Page assets share one scope per
 * directory; every other file is its own unit. `forcedKind` overrides the path rules. Returns
 * `{ file, line, column, text }` messages sorted by position.
 */
export function lintSources(sources, forcedKind) {
  const units = new Map();
  for (const { file, text } of sources) {
    const kind = forcedKind ?? kindOf(file);
    const key = kind === "page" ? `page:${path.dirname(file)}` : `file:${file}`;
    if (!units.has(key)) units.set(key, { kind, programs: [], syntax: [] });
    const unit = units.get(key);
    try {
      unit.programs.push({ file, ast: parse(text, kind) });
    } catch (error) {
      const at = error.loc ?? { line: 1, column: 0 };
      unit.syntax.push({ file, line: at.line, column: at.column + 1, text: `syntax error: ${error.message}` });
    }
  }
  const all = [];
  for (const unit of units.values()) {
    all.push(...unit.syntax);
    if (unit.syntax.length === 0) all.push(...analyze(unit.programs, unit.kind));
  }
  return all.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line || a.column - b.column);
}

/** Expand directory arguments to the .js and .mjs files beneath them. */
function expand(arg) {
  if (!fs.statSync(arg).isDirectory()) return [arg];
  return fs.readdirSync(arg, { withFileTypes: true }).flatMap((entry) => {
    const child = path.join(arg, entry.name);
    if (entry.isDirectory()) return entry.name === "node_modules" ? [] : expand(child);
    return /\.m?js$/.test(entry.name) ? [child] : [];
  });
}

/** Run the command line; returns the exit status. */
export function main(args) {
  if (args.length === 0) {
    console.error("usage: node scripts/lint-js.mjs <file-or-dir>...");
    return 2;
  }
  const sources = args.flatMap(expand).map((file) => ({ file, text: fs.readFileSync(file, "utf8") }));
  const messages = lintSources(sources);
  for (const m of messages) console.log(`${m.file}:${m.line}:${m.column}: ${m.text}`);
  return messages.length === 0 ? 0 : 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
