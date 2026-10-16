// The stages of the setup view, shaped as the service composes them (language/setup.rs).
//
// The preview server and the setup tests read the same fixtures, so what a person sees in the
// preview is what the tests drive. Each stage is a plain SetupView; the retired stage, which says
// nothing, belongs to the file that needs it. The preview server opens at one with
// `?setup=computer|browser|agent|done`: the page shows setup for a stage with something to do, and
// shows a finished setup only to a window that watched it finish, so `done` opens at the agent
// stage and finishes a few seconds later.

export const step = (name, state, word = "") => ({
  name, state, word,
  sr: name + (state === "done" ? ", done" : state === "current" ? ", current step" : "")
});
export const setupView = (stage, parts) => ({
  stage, eyebrow: "Set up Ghostlight", lead: null, code: null, rows_label: null, rows: [],
  extra_code: null, live: null, connected_line: null, tagline: null, ...parts
});
export const steps = (...entries) => entries.map((entry) => step(...entry));
export const browserRow = (id, name, short, detail, action) => ({
  id, name, detail, tone: "todo", action: {
    action, label: action.kind === "add_in_browser" ? "Add" : `Open ${short}`,
    name: action.kind === "add_in_browser" ? `Add to ${name}` : `Open ${name}`,
    primary: action.kind === "add_in_browser", external: action.kind === "add_in_browser",
    pending: {
      detail: action.kind === "add_in_browser" ? "Finish in the Chrome Web Store." : `Opening ${short}.`,
      live: `Waiting for ${short}.`
    }
  }
});
export const agentRow = (id, name, detail, tone, action = null) => ({ id, name, detail, tone, action });
export const setupStages = {
  computer: setupView("computer", {
    headline: { lead: "Finish installing on ", accent: "this computer", trail: "." },
    lead: "Run this once from a regular terminal.",
    steps: steps(["This computer", "current"], ["Your browser", "todo"], ["Your agent", "todo"]),
    light: "waiting",
    code: { label: "Paste into a terminal", text: "~/.ghostlight/bin/ghostlight install", copy_name: "Copy command" },
    live: { text: "Waiting for this computer.", done: false }
  }),
  browser: setupView("browser", {
    headline: { lead: "Add Ghostlight to ", accent: "your browser", trail: "." },
    lead: "One browser is enough.",
    steps: steps(["This computer", "done", "Installed"], ["Your browser", "current"], ["Your agent", "todo"]),
    light: "waiting",
    rows: [
      browserRow("chrome", "Google Chrome", "Chrome", "Not added", { kind: "add_in_browser", browser: "chrome" }),
      browserRow("edge", "Microsoft Edge", "Edge", "Added in your Work profile. Open Edge to finish.",
        { kind: "open_browser", browser: "edge", profile: "Profile 1" })
    ],
    live: { text: "Waiting for a browser.", done: false }
  }),
  agent: setupView("agent", {
    headline: { lead: "Ask ", accent: "your agent", trail: "." },
    steps: steps(["This computer", "done", "Installed"], ["Your browser", "done", "Connected in Chrome"], ["Your agent", "current"]),
    light: "lit",
    code: { label: "Paste into your agent", text: "Open sylin.org/ghostlight/demo/desk and ring the bell.", copy_name: "Copy prompt" },
    rows_label: "Agents on this computer",
    rows: [
      agentRow("claude-code", "Claude Code", "Connected", "done"),
      agentRow("claude-desktop", "Claude Desktop", "Found", "todo", {
        action: { kind: "connect_agent", agent: "claude-desktop" }, label: "Connect", name: "Connect Claude Desktop",
        primary: false, external: false, pending: { detail: "Connecting", live: null }
      }),
      agentRow("cursor", "Cursor", "Needs attention", "attention", {
        action: { kind: "review_agent", agent: "cursor" }, label: "Review", name: "Review Cursor",
        primary: false, external: false, pending: null
      })
    ],
    live: { text: "Waiting for your agent.", done: false },
    connected_line: "Chrome is connected."
  }),
  done: setupView("done", {
    eyebrow: "Ready",
    headline: { lead: "Claude Code just used your browser. ", accent: "You're set.", trail: "" },
    steps: steps(["This computer", "done", "Installed"], ["Your browser", "done", "Connected in Chrome"], ["Your agent", "done", "Claude Code"]),
    light: "lit",
    tagline: "A light left burning, so the halls stay safe."
  })
};

