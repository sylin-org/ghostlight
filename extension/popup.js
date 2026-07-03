// Ghostlight -- popup. Renders binary/worker-reported state and submits user gestures.
// Caches nothing (no chrome.storage, no persisted state) and decides nothing: the service
// worker holds the state and answers every render() call fresh. Two independent controls,
// each its own section: take-the-wheel pause (g10) and the panic kill switch (g11). They
// never share a control.

const statusEl = document.getElementById("status");
const toggleEl = document.getElementById("toggle");

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

// --- Panic kill switch (g11): one gesture, no confirmation. ---

const sessionStatusEl = document.getElementById("session-status");
const sessionButtonEl = document.getElementById("session-button");

function renderSession(state) {
  if (state.killed) {
    sessionStatusEl.textContent =
      "Session ended. Browser access is severed until you start a new session.";
    sessionButtonEl.id = "reconnect-button";
    sessionButtonEl.textContent = "Start new session";
    sessionButtonEl.classList.remove("kill");
    sessionButtonEl.disabled = false;
    return;
  }
  const connectedLine = state.connected ? "Connected to the binary." : "Not connected to the binary.";
  sessionStatusEl.textContent = `${connectedLine} Debugger attached to ${state.attachedTabs} tab(s).`;
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
