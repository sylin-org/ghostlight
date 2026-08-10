"use strict";

const invoke = window.__TAURI__?.core?.invoke;
const pageNames = {
  home: "Home",
  activity: "Sessions and operations",
  history: "History",
  checkup: "Checkup",
  configuration: "Configuration",
  install: "Installations"
};
const appState = {
  snapshot: null,
  view: "home",
  refreshTimer: null,
  searchTimer: null,
  toastTimer: null,
  connected: false,
  pendingHarnesses: new Set(),
  confirmation: null,
  renderKey: null
};

const elements = {
  title: document.querySelector("#page-title"),
  search: document.querySelector("#global-search"),
  searchResults: document.querySelector("#search-results"),
  railLight: document.querySelector("#rail-light"),
  railStatus: document.querySelector("#rail-status"),
  topbarState: document.querySelector("#topbar-state"),
  toast: document.querySelector("#app-toast")
};

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function label(value) {
  return String(value ?? "unknown")
    .replaceAll("_", " ")
    .replace(/\b\w/g, character => character.toUpperCase());
}

function shortId(value) {
  const text = String(value ?? "");
  if (text.length <= 23) return text;
  return `${text.slice(0, 11)}...${text.slice(-8)}`;
}

function formatTime(timestamp) {
  if (!timestamp) return "Now";
  return new Intl.DateTimeFormat(undefined, {
    month: "short", day: "numeric", hour: "numeric", minute: "2-digit", second: "2-digit"
  }).format(new Date(timestamp));
}

function relativeTime(timestamp) {
  if (!timestamp) return "now";
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1000));
  if (seconds < 10) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}

function setConnection(connected, message = "Orchestrator ready") {
  appState.connected = connected;
  elements.railLight.className = `connection-light ${connected ? "connected" : "error"}`;
  elements.railStatus.textContent = message;
  elements.topbarState.className = `topbar-state ${connected ? "connected" : "error"}`;
  elements.topbarState.innerHTML = `<span class="state-dot"></span><span>${escapeHtml(message)}</span>`;
}

function showToast(message, error = false) {
  clearTimeout(appState.toastTimer);
  elements.toast.textContent = message;
  elements.toast.className = `app-toast${error ? " error" : ""}`;
  elements.toast.hidden = false;
  appState.toastTimer = setTimeout(() => { elements.toast.hidden = true; }, 4200);
}

function renderKey(snapshot) {
  return JSON.stringify({ ...snapshot, generated_at_ms: 0 });
}

function navigate(view) {
  if (!pageNames[view]) return;
  appState.view = view;
  document.querySelectorAll(".view").forEach(element => {
    element.classList.toggle("active", element.dataset.page === view);
  });
  document.querySelectorAll(".rail-item").forEach(element => {
    const active = element.dataset.view === view;
    element.classList.toggle("active", active);
    if (active) element.setAttribute("aria-current", "page");
    else element.removeAttribute("aria-current");
  });
  elements.title.textContent = pageNames[view];
  elements.searchResults.hidden = true;
  document.querySelector("#main-content").scrollTop = 0;
}

async function refreshSnapshot({ quiet = true } = {}) {
  if (!invoke) {
    setConnection(false, "Desktop bridge unavailable");
    return;
  }
  try {
    const snapshot = await invoke("workbench_snapshot");
    const nextRenderKey = renderKey(snapshot);
    appState.snapshot = snapshot;
    setConnection(true, `Ready - ${label(appState.snapshot.service.runtime_state)}`);
    if (nextRenderKey !== appState.renderKey) {
      appState.renderKey = nextRenderKey;
      renderSnapshot(appState.snapshot);
    }
    if (!quiet) showToast("Checkup refreshed.");
  } catch (error) {
    setConnection(false, "Orchestrator unavailable");
    if (!quiet) showToast(String(error), true);
  }
}

function renderSnapshot(snapshot) {
  renderHome(snapshot);
  renderActivity(snapshot);
  renderHistory(snapshot);
  renderCheckup(snapshot);
  renderConfiguration(snapshot);
  renderInstall(snapshot);
}

function renderHome(snapshot) {
  const runtime = snapshot.service.runtime_state;
  const heroSummary = document.querySelector("#hero-summary");
  heroSummary.textContent = runtime === "active"
    ? "Your local orchestrator is ready. Browser work remains governed, visible, and inspectable."
    : runtime === "ended"
      ? "The runtime session has ended. Start a fresh session before accepting new browser work."
      : "Browser effects are paused while Ghostlight keeps its current state available.";
  const control = document.querySelector("#hero-control");
  control.disabled = false;
  control.dataset.intent = runtime === "active" ? "hold" : runtime === "ended" ? "start_session" : "resume";
  control.textContent = runtime === "active" ? "Pause browser work" : runtime === "ended" ? "Start new session" : "Resume browser work";

  const metrics = [
    [snapshot.overview.active_sessions, "Sessions", "Connected MCP clients"],
    [snapshot.overview.active_operations, "Operations", "Currently in progress"],
    [snapshot.overview.connected_browsers, "Browsers", "Compatible adapters online"],
    [snapshot.overview.blocked_in_history, "Blocked", "In local bounded history"]
  ];
  document.querySelector("#home-metrics").innerHTML = metrics.map(([value, name, note]) => `
    <article class="metric-card"><span class="metric-label">${name}</span><strong class="metric-value">${value}</strong><span class="metric-note">${note}</span></article>
  `).join("");

  const current = document.querySelector("#home-current");
  if (snapshot.operations.length) {
    current.innerHTML = `<div class="compact-list">${snapshot.operations.slice(0, 4).map(operationRow).join("")}</div>`;
  } else if (snapshot.sessions.length) {
    current.innerHTML = `<div class="compact-list">${snapshot.sessions.slice(0, 4).map(sessionRow).join("")}</div>`;
  } else {
    current.innerHTML = emptyState("Quiet for now", "Connected coding harness sessions and their operations will appear here.");
  }

  const health = document.querySelector("#home-health");
  health.innerHTML = `<div class="compact-list">${snapshot.diagnostics.map(diagnosticRow).join("")}</div>`;
}

function operationRow(operation) {
  return `<div class="compact-row">
    <span class="status-icon ${escapeHtml(operation.phase)}"></span>
    <div><strong>${escapeHtml(operation.tool)}</strong><small>${escapeHtml(operation.activity)} - ${escapeHtml(shortId(operation.workspace))}</small></div>
    <span class="micro-badge">${escapeHtml(operation.phase)}</span>
  </div>`;
}

function sessionRow(session) {
  return `<div class="compact-row">
    <span class="status-icon ${session.leased ? "running" : "passing"}"></span>
    <div><strong>${escapeHtml(session.client_label)}</strong><small>${session.tab_count} controlled tabs - ${session.active_operations} current operations</small></div>
    <span class="micro-badge">Session</span>
  </div>`;
}

function diagnosticRow(item) {
  return `<div class="compact-row">
    <span class="status-icon ${escapeHtml(item.severity)}"></span>
    <div><strong>${escapeHtml(item.label)}</strong><small>${escapeHtml(item.detail)}</small></div>
    <span class="micro-badge">${escapeHtml(item.severity)}</span>
  </div>`;
}

function emptyState(title, detail) {
  return `<div class="empty-state"><div><strong>${escapeHtml(title)}</strong><span>${escapeHtml(detail)}</span></div></div>`;
}

function renderActivity(snapshot) {
  const groups = [
    ["Sessions", `${snapshot.sessions.length} connected client sessions`, snapshot.sessions.length
      ? `<div class="card-grid">${snapshot.sessions.map(sessionCard).join("")}</div>`
      : emptyState("No client sessions", "A supported harness will appear when it connects through the MCP relay.")],
    ["Operations", `${snapshot.operations.length} current units of work`, snapshot.operations.length
      ? `<div class="card-grid">${snapshot.operations.map(operationCard).join("")}</div>`
      : emptyState("No current operations", "New work appears here for its complete orchestrator lifetime.")],
    ["Browser instances", `${snapshot.browsers.length} compatible adapters`, snapshot.browsers.length
      ? `<div class="card-grid">${snapshot.browsers.map(browserCard).join("")}</div>`
      : emptyState("Waiting for a browser", "Open a supported Chromium browser with Ghostlight in Browser enabled.")]
  ];
  document.querySelector("#activity-content").innerHTML = groups.map(([title, note, content]) => `
    <section class="panel group-panel"><div class="section-heading"><div><span class="section-kicker">${escapeHtml(note)}</span><h2>${escapeHtml(title)}</h2></div></div>${content}</section>
  `).join("");
}

function sessionCard(session) {
  return `<article class="entity-card"><div class="entity-top"><h3>${escapeHtml(session.client_label)}</h3><span class="status-icon ${session.leased ? "running" : "passing"}"></span></div><span class="entity-id">${escapeHtml(session.id)}</span><div class="entity-facts"><span class="fact">${session.tab_count} tabs</span><span class="fact">${session.active_operations} operations</span>${session.held_tab_count ? `<span class="fact">${session.held_tab_count} held</span>` : ""}</div></article>`;
}

function operationCard(operation) {
  return `<article class="entity-card"><div class="entity-top"><h3>${escapeHtml(operation.tool)}</h3><span class="micro-badge">${escapeHtml(operation.phase)}</span></div><span class="entity-id">${escapeHtml(operation.invocation)}</span><div class="entity-facts"><span class="fact">${escapeHtml(operation.activity)}</span><span class="fact">${escapeHtml(relativeTime(operation.started_at_ms))}</span></div></article>`;
}

function browserCard(browser) {
  return `<article class="entity-card"><div class="entity-top"><h3>${escapeHtml(browser.family)}</h3><span class="status-icon passing"></span></div><span class="entity-id">${escapeHtml(browser.id)}</span><div class="entity-facts"><span class="fact">Adapter ${escapeHtml(browser.adapter_version || "unknown")}</span><span class="fact">Connected</span></div></article>`;
}

function renderHistory(snapshot) {
  document.querySelector("#history-count").textContent = `${snapshot.history.length} ${snapshot.history.length === 1 ? "record" : "records"}`;
  const body = document.querySelector("#history-body");
  if (!snapshot.history.length) {
    body.innerHTML = `<tr><td colspan="6">${emptyState("No completed work yet", "Payload-free completion records will appear here.")}</td></tr>`;
    return;
  }
  body.innerHTML = snapshot.history.map(item => `<tr>
    <td title="${escapeHtml(formatTime(item.timestamp_ms))}">${escapeHtml(relativeTime(item.timestamp_ms))}</td>
    <td><strong>${escapeHtml(item.tool)}</strong><div class="mono">${escapeHtml(shortId(item.invocation))}</div></td>
    <td class="mono" title="${escapeHtml(item.workspace)}">${escapeHtml(shortId(item.workspace))}</td>
    <td>${escapeHtml(label(item.capability))}</td>
    <td><span class="outcome ${item.allowed ? "" : "blocked"}">${escapeHtml(label(item.status))}</span><div class="mono">${escapeHtml(item.reason)}</div></td>
    <td>${escapeHtml(label(item.effect))}</td>
  </tr>`).join("");
}

function renderCheckup(snapshot) {
  document.querySelector("#diagnostic-grid").innerHTML = snapshot.diagnostics.map(item => `
    <article class="panel diagnostic-card"><div class="diagnostic-status"><span class="status-icon ${escapeHtml(item.severity)}"></span>${escapeHtml(item.severity)}</div><h2>${escapeHtml(item.label)}</h2><p>${escapeHtml(item.detail)}</p></article>
  `).join("");
}

function renderConfiguration(snapshot) {
  const config = snapshot.configuration;
  const runtime = config.runtime_state;
  document.querySelector("#runtime-heading").textContent = runtime === "active" ? "Browser work is active" : runtime === "ended" ? "The runtime session has ended" : "Browser work is paused";
  document.querySelector("#runtime-detail").textContent = runtime === "active" ? "New governed operations may proceed." : runtime === "ended" ? "Start a new session before accepting further effects." : "State remains available while later effects wait.";
  document.querySelectorAll(".segmented [data-intent]").forEach(button => {
    const selected = (runtime === "active" && button.dataset.intent === "resume") ||
      (["held", "attention"].includes(runtime) && button.dataset.intent === "hold") ||
      (runtime === "ended" && button.dataset.intent === "end_session");
    button.classList.toggle("selected", selected);
  });
  const cards = [
    ["Local policy", config.local_policy_configured, config.local_policy_valid, "Optional user-owned authority rules."],
    ["Managed authority", config.managed_authority_configured, config.managed_authority_valid, "An optional monotonic managed restriction layer."],
    ["Runtime control file", config.runtime_control_file_configured, true, "An optional local final-boundary control source."]
  ];
  document.querySelector("#configuration-grid").innerHTML = cards.map(([title, configured, valid, detail]) => `
    <article class="panel configuration-card"><span class="section-kicker">Authority source</span><h2>${escapeHtml(title)}</h2><p>${escapeHtml(detail)}</p><span class="config-state ${!configured ? "neutral" : !valid ? "invalid" : ""}">${!configured ? "Not configured" : valid ? "Configured and valid" : "Invalid - failing closed"}</span></article>
  `).join("");
}

function renderInstall(snapshot) {
  const install = document.querySelector("#install-content");
  if (Array.isArray(snapshot.harnesses) && snapshot.harnesses.length) {
    install.innerHTML = snapshot.harnesses.map(harness => harnessCard(harness)).join("");
  } else {
    install.innerHTML = `<section class="panel group-panel">${emptyState("No supported harnesses discovered", "Ghostlight has not found a supported development harness in this user context.")}</section>`;
  }
}

function harnessCard(harness) {
  const installed = harness.state === "installed";
  const pending = appState.pendingHarnesses.has(harness.id);
  const allowed = installed ? harness.can_uninstall : harness.can_install;
  return `<article class="panel harness-card"><div><div class="entity-top"><h2>${escapeHtml(harness.name)}</h2><span class="micro-badge">${escapeHtml(label(harness.state))}</span></div><p>${escapeHtml(harness.detail)}</p></div><div class="harness-actions"><button class="button quiet" type="button" data-harness-action="check" data-harness="${escapeHtml(harness.id)}" data-harness-name="${escapeHtml(harness.name)}" ${pending ? "disabled" : ""}>Check</button><button class="button ${installed ? "danger" : "primary"}" type="button" data-harness-action="${installed ? "uninstall" : "install"}" data-harness="${escapeHtml(harness.id)}" data-harness-name="${escapeHtml(harness.name)}" ${pending || !allowed ? "disabled" : ""}>${pending ? "Working..." : installed ? "Uninstall" : "Install"}</button></div></article>`;
}

function confirmUninstall(name) {
  if (appState.confirmation) return Promise.resolve(false);
  const backdrop = document.querySelector("#confirm-dialog");
  document.querySelector("#confirm-title").textContent = `Remove Ghostlight from ${name}?`;
  return new Promise(resolve => {
    const finish = confirmed => {
      backdrop.hidden = true;
      appState.confirmation = null;
      document.removeEventListener("keydown", onKeyDown);
      resolve(confirmed);
    };
    const onKeyDown = event => {
      if (event.key === "Escape") finish(false);
    };
    appState.confirmation = finish;
    backdrop.hidden = false;
    backdrop.querySelector('[data-confirm="cancel"]').focus();
    document.addEventListener("keydown", onKeyDown);
  });
}

async function handleHarnessAction(button) {
  if (!invoke || button.disabled) return;
  const { harness: id, harnessAction: action, harnessName: name } = button.dataset;
  if (action === "uninstall" && !await confirmUninstall(name)) return;
  appState.pendingHarnesses.add(id);
  if (appState.snapshot) renderInstall(appState.snapshot);
  try {
    const result = await invoke("manage_harness", { id, action });
    showToast(result.message);
  } catch (error) {
    showToast(String(error), true);
  } finally {
    appState.pendingHarnesses.delete(id);
    await refreshSnapshot();
    if (appState.snapshot) renderInstall(appState.snapshot);
  }
}

async function applyIntent(intent) {
  if (!invoke) return;
  try {
    const result = await invoke("apply_runtime_intent", { intent });
    showToast(result.message);
    await refreshSnapshot();
  } catch (error) {
    showToast(String(error), true);
  }
}

async function search(query) {
  const trimmed = query.trim();
  if (!trimmed) {
    elements.searchResults.hidden = true;
    elements.searchResults.innerHTML = "";
    return;
  }
  try {
    const hits = await invoke("workbench_search", { query: trimmed });
    elements.searchResults.innerHTML = hits.length ? hits.map(hit => `
      <button class="search-hit" type="button" data-search-view="${escapeHtml(hit.view)}">
        <span class="search-kind">${escapeHtml(hit.kind)}</span><span><strong>${escapeHtml(hit.title)}</strong><small>${escapeHtml(hit.detail)}</small></span>
      </button>
    `).join("") : `<div class="search-empty">No matching Ghostlight records.</div>`;
    elements.searchResults.hidden = false;
  } catch (error) {
    elements.searchResults.innerHTML = `<div class="search-empty">${escapeHtml(String(error))}</div>`;
    elements.searchResults.hidden = false;
  }
}

document.addEventListener("click", event => {
  const confirmation = event.target.closest("[data-confirm]");
  if (confirmation && appState.confirmation) {
    appState.confirmation(confirmation.dataset.confirm === "remove");
    return;
  }
  const viewButton = event.target.closest("[data-view]");
  if (viewButton) navigate(viewButton.dataset.view);
  const intentButton = event.target.closest("[data-intent]");
  if (intentButton && !intentButton.disabled) applyIntent(intentButton.dataset.intent);
  const searchHit = event.target.closest("[data-search-view]");
  if (searchHit) {
    navigate(searchHit.dataset.searchView);
    elements.search.value = "";
    elements.searchResults.hidden = true;
  }
  const harnessButton = event.target.closest("[data-harness-action]");
  if (harnessButton) handleHarnessAction(harnessButton);
  if (!event.target.closest(".search-wrap")) elements.searchResults.hidden = true;
});

elements.search.addEventListener("input", () => {
  clearTimeout(appState.searchTimer);
  appState.searchTimer = setTimeout(() => search(elements.search.value), 140);
});

elements.search.addEventListener("keydown", event => {
  if (event.key === "Escape") {
    elements.search.value = "";
    elements.searchResults.hidden = true;
    elements.search.blur();
  }
});

document.addEventListener("keydown", event => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    elements.search.focus();
    elements.search.select();
  }
});

document.querySelector("#refresh-checkup").addEventListener("click", () => refreshSnapshot({ quiet: false }));
document.querySelector("#refresh-install").addEventListener("click", async event => {
  event.currentTarget.disabled = true;
  try {
    await invoke("refresh_harnesses");
    await refreshSnapshot();
    showToast("Installations checked.");
  } catch (error) {
    showToast(String(error), true);
  } finally {
    event.currentTarget.disabled = false;
  }
});
document.querySelector("#test-notification").addEventListener("click", async event => {
  event.currentTarget.disabled = true;
  try {
    await invoke("test_notification");
    showToast("Test notification sent.");
  } catch (error) {
    showToast(String(error), true);
  } finally {
    event.currentTarget.disabled = false;
  }
});

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refreshSnapshot();
});

refreshSnapshot();
appState.refreshTimer = setInterval(() => {
  if (!document.hidden) refreshSnapshot();
}, 1500);
