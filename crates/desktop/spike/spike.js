"use strict";
// The spike page: an echo through the shell bridge and five probes that the content policy and
// the webview settings keep the page off the network. 192.0.2.1 is TEST-NET-1 (RFC 5737) and is
// never routed, so a probe that is not blocked still reaches nothing.
(function () {
  var TARGET = "https://192.0.2.1/";
  var BLOCKED = "blocked by the content policy";
  var NOT_BLOCKED = "NOT blocked";
  var VIOLATION_WAIT_MS = 2000;
  var NAVIGATION_WAIT_MS = 1000;

  var log = document.getElementById("log");
  var probes = document.getElementById("probes");
  var count = 0;
  var pending = [];

  function line(text) {
    log.textContent += text + "\n";
  }

  function row(name) {
    var tr = document.createElement("tr");
    var label = document.createElement("td");
    var result = document.createElement("td");
    label.textContent = name;
    result.textContent = "running";
    tr.appendChild(label);
    tr.appendChild(result);
    probes.appendChild(tr);
    return result;
  }

  function sameUrl(a, b) {
    try {
      return new URL(a, location.href).href === new URL(b, location.href).href;
    } catch (error) {
      return false;
    }
  }

  document.addEventListener("securitypolicyviolation", function (event) {
    pending.forEach(function (probe) {
      if (sameUrl(event.blockedURI, probe.url)) {
        probe.seen = true;
      }
    });
  });

  function policyProbe(name, url, start) {
    var result = row(name);
    var probe = { url: url, seen: false };
    pending.push(probe);
    start(url);
    return new Promise(function (resolve) {
      setTimeout(function () {
        result.textContent = probe.seen ? BLOCKED : NOT_BLOCKED;
        resolve();
      }, VIOLATION_WAIT_MS);
    });
  }

  function windowProbe() {
    var result = row("window.open");
    var opened = null;
    try {
      opened = window.open(TARGET);
    } catch (error) {
      opened = null;
    }
    if (opened === null) {
      result.textContent = "new window denied";
    } else {
      result.textContent = "NOT denied";
      opened.close();
    }
    return Promise.resolve();
  }

  function navigationProbe() {
    var result = row("location.assign");
    var before = location.href;
    location.assign(TARGET);
    return new Promise(function (resolve) {
      setTimeout(function () {
        result.textContent = location.href === before ? "navigation denied" : "NOT denied";
        resolve();
      }, NAVIGATION_WAIT_MS);
    });
  }

  document.getElementById("post").addEventListener("click", function () {
    count += 1;
    window.ghostlightShell.post({ kind: "echo", n: count });
  });
  window.ghostlightShell.listen(function (message) {
    line("reply " + JSON.stringify(message));
  });

  Promise.all([
    policyProbe("fetch", TARGET + "probe", function (url) {
      fetch(url).catch(function () {});
    }),
    policyProbe("img", TARGET + "probe.png", function (url) {
      var image = new Image();
      image.src = url;
    }),
    policyProbe("script", TARGET + "probe.js", function (url) {
      var script = document.createElement("script");
      script.src = url;
      document.head.appendChild(script);
    }),
    windowProbe(),
  ]).then(navigationProbe);
})();
