// Ghostlight Console client script. Fetches this machine's own local API (never a remote
// control plane) and renders read-mostly views: live sessions, the provenance-aware config
// table, and the single "enable remote connections" write action. Populated incrementally
// (config: K3, sessions: K4, enable-remote: K5); this file is the page-load entry point only.

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

async function loadConfig() {
  const el = document.getElementById("config-placeholder");
  try {
    const res = await fetch("/api/v1/config");
    if (!res.ok) {
      el.textContent = "Could not load configuration (" + res.status + ").";
      return;
    }
    const data = await res.json();
    const rows = data.keys.map((k) => {
      const locked = k.locked ? '<span class="locked-badge">org-locked</span>' : "";
      return "<tr><td>" + escapeHtml(k.key) + "</td><td>" + escapeHtml(JSON.stringify(k.value)) +
        "</td><td>" + escapeHtml(k.source) + "</td><td>" + locked + "</td></tr>";
    }).join("");
    el.outerHTML = "<table><thead><tr><th>Key</th><th>Value</th><th>Layer</th><th></th></tr></thead>" +
      "<tbody>" + rows + "</tbody></table>";
  } catch (e) {
    el.textContent = "Could not load configuration.";
  }
}

async function loadSessions() {
  const el = document.getElementById("sessions-placeholder");
  try {
    const res = await fetch("/api/v1/sessions");
    if (!res.ok) {
      el.textContent = "Could not load sessions (" + res.status + ").";
      return;
    }
    const data = await res.json();
    const rows = data.adapter_bindings.map((b) => {
      return "<tr><td>" + escapeHtml(b.guid) + "</td><td>" + escapeHtml(b.pid) +
        "</td><td>" + escapeHtml(b.owned_tab_ids.join(", ")) + "</td></tr>";
    }).join("");
    const summary = "<p>Live sessions: " + escapeHtml(data.live_session_count) + "</p>";
    const table = "<table><thead><tr><th>Session</th><th>PID</th><th>Tabs</th></tr></thead>" +
      "<tbody>" + rows + "</tbody></table>";
    const note = "<p class=\"note\">" + escapeHtml(data.note) + "</p>";
    el.outerHTML = summary + table + note;
  } catch (e) {
    el.textContent = "Could not load sessions.";
  }
}

function wireEnableRemote() {
  const button = document.getElementById("enable-remote-button");
  const status = document.getElementById("remote-status");
  if (!button) return;
  button.addEventListener("click", async () => {
    button.disabled = true;
    status.textContent = "Enabling...";
    try {
      const res = await fetch("/api/v1/config/inbound-web-enable-remote", { method: "POST" });
      const data = await res.json();
      if (res.ok) {
        status.textContent = "Enabled: inbound.web.from = " + JSON.stringify(data.value) +
          ". " + data.note;
        loadConfig();
      } else {
        status.textContent = "Could not enable remote connections: " + data.error;
      }
    } catch (e) {
      status.textContent = "Could not enable remote connections.";
    } finally {
      button.disabled = false;
    }
  });
}

document.addEventListener("DOMContentLoaded", () => {
  loadConfig();
  loadSessions();
  wireEnableRemote();
});
