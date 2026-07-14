// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- popup. Renders binary/worker-reported state and submits user gestures.
// Caches no session state (the worker holds it) and decides nothing: the service
// worker holds the state and answers every render() call fresh. Two independent controls,
// each its own section: take-the-wheel pause (g10) and the panic kill switch (g11). They
// never share a control.

const statusEl = document.getElementById("status");
const toggleEl = document.getElementById("toggle");
const attentionSection = document.getElementById("attention-section");
const attentionList = document.getElementById("attention-list");

function render(state) {
  if (!state.session) {
    statusEl.textContent = "No active browsing session.";
    toggleEl.textContent = "Pause agent browsing (take the wheel)";
    toggleEl.disabled = true;
    return;
  }
  toggleEl.disabled = false;
  if (state.held) {
    statusEl.textContent = "Agent browsing is PAUSED.";
    toggleEl.textContent = "Resume agent browsing";
  } else {
    statusEl.textContent = "Agent browsing is allowed.";
    toggleEl.textContent = "Pause agent browsing (take the wheel)";
  }
}

function refresh() {
  chrome.runtime.sendMessage({ type: "getHoldState" }, (state) => {
    render(state || { session: false, held: false });
  });
}

toggleEl.addEventListener("click", () => {
  const nextHeld = toggleEl.textContent.indexOf("Resume") === -1;
  chrome.runtime.sendMessage({ type: "setHold", held: nextHeld }, (state) => {
    render(state || { session: false, held: false });
  });
});

refresh();

function attentionAction(guid, disposition) {
  chrome.runtime.sendMessage({ type: "ATTENTION_ACTION", guid, disposition }, renderAttention);
}

function renderAttention(state) {
  const sessions = (state && Array.isArray(state.sessions)) ? state.sessions : [];
  attentionSection.hidden = sessions.length === 0;
  attentionList.replaceChildren();
  for (const session of sessions) {
    const item = document.createElement("div");
    item.className = "attention-item";
    const label = document.createElement("div");
    label.className = "attention-label";
    label.textContent = String(session.label || "MCP client") + " is paused";
    const meta = document.createElement("div");
    meta.className = "attention-meta";
    meta.textContent = (session.origin ? String(session.origin) + " - " : "") +
      String(session.count || 0) + " blocked actions";
    const actions = document.createElement("div");
    actions.className = "attention-actions";
    for (const [disposition, text] of [
      ["keep_paused", "Keep paused"],
      ["resume", "Resume"],
      ["resume_quiet", "Resume + quiet"],
      ["end_session", "End session"],
    ]) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = text;
      if (disposition === "end_session") button.className = "danger";
      button.addEventListener("click", () => attentionAction(session.guid, disposition));
      actions.appendChild(button);
    }
    item.append(label, meta, actions);
    attentionList.appendChild(item);
  }
}

function refreshAttention() {
  chrome.runtime.sendMessage({ type: "GET_ATTENTION_STATE" }, renderAttention);
}

refreshAttention();

// --- Panic kill switch (g11): one gesture, no confirmation. ---

const sessionStatusEl = document.getElementById("session-status");
const sessionButtonEl = document.getElementById("session-button");
const linkDot = document.getElementById("link-dot");

// The header connection dot: a live, at-a-glance signal that the agent can reach this browser.
function renderLinkDot(state) {
  if (state.killed) {
    linkDot.className = "";
    linkDot.title = "Session ended";
  } else if (state.connected) {
    linkDot.className = "on";
    linkDot.title = "Connected to Ghostlight";
  } else {
    linkDot.className = "wait";
    linkDot.title = "Waiting for the Ghostlight service...";
  }
}

function renderSession(state) {
  renderLinkDot(state);
  if (state.killed) {
    sessionStatusEl.textContent =
      "Session ended. Browser access is severed until you start a new session.";
    sessionButtonEl.id = "reconnect-button";
    sessionButtonEl.textContent = "Start new session";
    sessionButtonEl.classList.remove("kill");
    sessionButtonEl.disabled = false;
    return;
  }
  const connectedLine = state.connected
    ? "Connected to Ghostlight."
    : "Waiting for the Ghostlight service...";
  const recordingLine = state.recordingTabs > 0 ? ` REC on ${state.recordingTabs} tab(s).` : "";
  sessionStatusEl.textContent = `${connectedLine} Debugger attached to ${state.attachedTabs} tab(s).${recordingLine}`;
  sessionButtonEl.id = "kill-button";
  sessionButtonEl.textContent = "End session now";
  sessionButtonEl.classList.add("kill");
  sessionButtonEl.disabled = false;
}

function refreshSession() {
  chrome.runtime.sendMessage({ type: "GET_SESSION_STATE" }, (state) => {
    renderSession(state || { killed: false, connected: false, attachedTabs: 0 });
  });
}

sessionButtonEl.addEventListener("click", () => {
  const type = sessionButtonEl.id === "reconnect-button" ? "RECONNECT_SESSION" : "KILL_SESSION";
  sessionButtonEl.disabled = true;
  chrome.runtime.sendMessage({ type }, (state) => {
    renderSession(state || { killed: false, connected: false, attachedTabs: 0 });
  });
});

refreshSession();
// Poll while the popup is open so the dot flips green on its own when the ~24s keepalive
// (or a just-started service) connects -- no reopen needed.
setInterval(refreshSession, 1500);
setInterval(refreshAttention, 1500);

// --- Action captions (visual feedback dictionary): a persisted, off-by-default UI preference the
// content-script indicator reads on every page. The one bit this popup persists; all session state
// stays with the worker. ---

const captionsToggle = document.getElementById("captions-toggle");
chrome.storage.local.get("ghostlight_captions", (r) => {
  captionsToggle.checked = !!(r && r.ghostlight_captions);
});
captionsToggle.addEventListener("change", () => {
  chrome.storage.local.set({ ghostlight_captions: captionsToggle.checked });
});

document.getElementById("open-options").addEventListener("click", () => {
  chrome.runtime.openOptionsPage();
});
