"use strict";
// The one bridge between a page and the Ghostlight shell: post JSON, receive JSON.
// The shell injects this file into the main frame before any page script runs.
(function () {
  var handlers = [];

  function post(message) {
    window.ipc.postMessage(JSON.stringify(message));
  }

  function listen(handler) {
    handlers.push(handler);
    return function () {
      var index = handlers.indexOf(handler);
      if (index >= 0) {
        handlers.splice(index, 1);
      }
    };
  }

  function deliver(text) {
    var value = JSON.parse(text);
    handlers.slice().forEach(function (handler) {
      try {
        handler(value);
      } catch (error) {
        console.error(error);
      }
    });
  }

  Object.defineProperty(window, "ghostlightShell", {
    value: Object.freeze({ post: post, listen: listen }),
    writable: false,
    configurable: false,
    enumerable: true,
  });
  Object.defineProperty(window, "__ghostlightShellDeliver", {
    value: deliver,
    writable: false,
    configurable: false,
    enumerable: false,
  });
})();
