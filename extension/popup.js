(function ghostlightPopup() {
  "use strict";

  const status = document.getElementById("status");
  const toggle = document.getElementById("toggle");
  const linkDot = document.getElementById("link-dot");
  const sessionStatus = document.getElementById("session-status");
  const sessionButton = document.getElementById("session-button");
  const attentionSection = document.getElementById("attention-section");
  const attentionList = document.getElementById("attention-list");
  const captions = document.getElementById("captions-toggle");
  let latestSnapshot = null;
  let latestPreferences = null;

  async function request(message) {
    const response = await chrome.runtime.sendMessage(message);
    if (!response?.ok) throw new Error(response?.error || "Ghostlight did not respond.");
    return response.value;
  }

  function renderLink(snapshot) {
    linkDot.className = "";
    if (snapshot.control_state === "ended") {
      linkDot.title = "Session ended";
    } else if (snapshot.connected && snapshot.compatible) {
      linkDot.className = "on";
      linkDot.title = "Connected to Ghostlight";
    } else {
      linkDot.className = "wait";
      linkDot.title = snapshot.compatible ? "Waiting for the Ghostlight service..." : "Ghostlight version mismatch";
    }
  }

  function renderHold(snapshot) {
    const connected = snapshot.connected && snapshot.compatible;
    if (!connected || snapshot.control_state === "ended") {
      status.textContent = "No active browsing session.";
      toggle.textContent = "Pause agent browsing (take the wheel)";
      toggle.disabled = true;
      return;
    }
    toggle.disabled = false;
    if (["held", "attention"].includes(snapshot.control_state)) {
      status.textContent = "Agent browsing is PAUSED.";
      toggle.textContent = "Resume agent browsing";
    } else {
      status.textContent = "Agent browsing is allowed.";
      toggle.textContent = "Pause agent browsing (take the wheel)";
    }
  }

  function renderSession(snapshot) {
    if (snapshot.control_state === "ended") {
      sessionStatus.textContent = "Session ended. Browser access is severed until you start a new session.";
      sessionButton.textContent = "Start new session";
      sessionButton.dataset.intent = "start_session";
      sessionButton.classList.remove("kill");
      sessionButton.disabled = !snapshot.connected || !snapshot.compatible;
      return;
    }
    const connectedLine = snapshot.connected && snapshot.compatible
      ? "Connected to Ghostlight."
      : snapshot.compatible ? "Waiting for the Ghostlight service..." : "Ghostlight version mismatch.";
    const recordingLine = snapshot.recording_tabs > 0 ? ` REC on ${snapshot.recording_tabs} tab(s).` : "";
    sessionStatus.textContent = `${connectedLine} Debugger attached to ${snapshot.attached_tabs || 0} tab(s).${recordingLine}`;
    sessionButton.textContent = "End session now";
    sessionButton.dataset.intent = "end_session";
    sessionButton.classList.add("kill");
    sessionButton.disabled = !snapshot.connected || !snapshot.compatible;
  }

  function attentionRecords(snapshot) {
    if (snapshot.control_state !== "attention") return [];
    const grouped = new Map();
    for (const item of snapshot.activity || []) {
      const key = item.workspace || "workspace";
      const current = grouped.get(key) || { label: item.client_label || "MCP client", count: 0 };
      current.count += 1;
      grouped.set(key, current);
    }
    return grouped.size ? Array.from(grouped.values()) : [{ label: "MCP client", count: 1 }];
  }

  function renderAttention(snapshot) {
    const records = attentionRecords(snapshot);
    attentionSection.hidden = records.length === 0;
    attentionList.replaceChildren(...records.map((record) => {
      const item = document.createElement("div");
      item.className = "attention-item";
      const label = document.createElement("div");
      label.className = "attention-label";
      label.textContent = `${record.label} is paused`;
      const meta = document.createElement("div");
      meta.className = "attention-meta";
      meta.textContent = `${record.count} blocked action${record.count === 1 ? "" : "s"}`;
      const actions = document.createElement("div");
      actions.className = "attention-actions";
      for (const [intent, text, danger] of [
        ["keep_paused", "Keep paused", false],
        ["resume", "Resume", false],
        ["resume_quiet", "Resume + quiet", false],
        ["end_session", "End session", true]
      ]) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = text;
        if (danger) button.className = "danger";
        button.addEventListener("click", () => attentionAction(intent));
        actions.appendChild(button);
      }
      item.append(label, meta, actions);
      return item;
    }));
  }

  function render(snapshot) {
    latestSnapshot = snapshot;
    renderLink(snapshot);
    renderHold(snapshot);
    renderSession(snapshot);
    renderAttention(snapshot);
  }

  async function refresh() {
    try {
      const [snapshot, preferences] = await Promise.all([
        request({ kind: "ui_snapshot" }),
        request({ kind: "get_preferences" })
      ]);
      latestPreferences = preferences;
      captions.checked = Boolean(preferences.captions);
      render(snapshot);
    } catch (error) {
      render({ connected: false, compatible: true, control_state: "active", attached_tabs: 0, recording_tabs: 0, activity: [] });
      sessionStatus.textContent = String(error?.message ?? error);
    }
  }

  async function attentionAction(intent) {
    try {
      if (intent === "keep_paused") return refresh();
      if (intent === "resume_quiet") {
        latestPreferences = await request({
          kind: "set_preferences",
          preferences: { ...latestPreferences, effects: false, captions: false }
        });
        captions.checked = false;
        intent = "resume";
      }
      await request({ kind: "runtime_control", intent });
      await refresh();
    } catch (error) {
      sessionStatus.textContent = String(error?.message ?? error);
    }
  }

  toggle.addEventListener("click", async () => {
    toggle.disabled = true;
    await attentionAction(latestSnapshot?.control_state === "active" ? "hold" : "resume");
  });
  sessionButton.addEventListener("click", async () => {
    sessionButton.disabled = true;
    await attentionAction(sessionButton.dataset.intent);
  });
  captions.addEventListener("change", async () => {
    try {
      latestPreferences = await request({
        kind: "set_preferences",
        preferences: { ...latestPreferences, captions: captions.checked }
      });
    } catch (error) {
      captions.checked = !captions.checked;
      sessionStatus.textContent = String(error?.message ?? error);
    }
  });
  document.getElementById("open-options").addEventListener("click", () => chrome.runtime.openOptionsPage());
  chrome.runtime.onMessage.addListener((message) => { if (message?.kind === "ui_state_changed") refresh(); });
  setInterval(refresh, 1500);
  refresh();
})();
