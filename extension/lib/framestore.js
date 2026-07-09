// SPDX-License-Identifier: Apache-2.0 OR MIT
// Ghostlight -- gif_creator durable frame store (ADR-0052 Decision 2). A thin IndexedDB wrapper
// holding recording frames as raw JPEG Blobs keyed [tabId, seq] plus one small per-tab state record
// ({tabId, active, vpW}); sequence bookkeeping is DERIVED from the stored frame keys on rehydration,
// so nothing hot-path writes state. IndexedDB survives service-worker death, which is the point: an
// export crash or idle kill no longer destroys the recording, and a retry re-reads the frames.
//
// IndexedDB does not exist under node, so this module is worker-only and live-verified; the pure
// logic around it (delay computation, action tagging, overlay routing) lives in lib/gifoverlay.js
// and is node-tested. IIFE-wrapped and exposed as a namespace per lib/constants.js's pattern
// (idempotent under MV3 worker re-evaluation).
(function () {
  "use strict";

  var DB_NAME = "ghostlight_gif";
  var DB_VERSION = 1;
  var FRAMES = "frames"; // keyPath ["tabId","seq"], index by_tab on tabId
  var STATE = "state"; // keyPath "tabId"

  var dbPromise = null;

  function open() {
    if (dbPromise) return dbPromise;
    dbPromise = new Promise(function (resolve, reject) {
      var req = indexedDB.open(DB_NAME, DB_VERSION);
      req.onupgradeneeded = function () {
        var db = req.result;
        if (!db.objectStoreNames.contains(FRAMES)) {
          var frames = db.createObjectStore(FRAMES, { keyPath: ["tabId", "seq"] });
          frames.createIndex("by_tab", "tabId");
        }
        if (!db.objectStoreNames.contains(STATE)) {
          db.createObjectStore(STATE, { keyPath: "tabId" });
        }
      };
      req.onsuccess = function () {
        var db = req.result;
        // Reopen lazily if the browser closes the connection underneath us.
        db.onclose = function () {
          dbPromise = null;
        };
        resolve(db);
      };
      req.onerror = function () {
        dbPromise = null;
        reject(req.error);
      };
    });
    return dbPromise;
  }

  function done(req) {
    return new Promise(function (resolve, reject) {
      req.onsuccess = function () {
        resolve(req.result);
      };
      req.onerror = function () {
        reject(req.error);
      };
    });
  }

  // Store one frame record: { tabId, seq, blob, ts, vpW?, ...action metadata }.
  function putFrame(record) {
    return open().then(function (db) {
      return done(db.transaction(FRAMES, "readwrite").objectStore(FRAMES).put(record));
    });
  }

  // Delete a single frame (used for oldest-frame eviction at the cap).
  function deleteFrame(tabId, seq) {
    return open().then(function (db) {
      return done(db.transaction(FRAMES, "readwrite").objectStore(FRAMES).delete([tabId, seq]));
    });
  }

  // All of a tab's frames, seq-ascending (index order is [tabId] then primary key [tabId, seq]).
  function frames(tabId) {
    return open().then(function (db) {
      return done(db.transaction(FRAMES).objectStore(FRAMES).index("by_tab").getAll(IDBKeyRange.only(tabId)));
    });
  }

  // The tab's stored frame seq numbers, ascending. Rehydration derives firstSeq/nextSeq/count from
  // this instead of trusting a state record that hot-path writes could leave stale.
  function frameSeqs(tabId) {
    return open().then(function (db) {
      return done(
        db.transaction(FRAMES).objectStore(FRAMES).index("by_tab").getAllKeys(IDBKeyRange.only(tabId))
      ).then(function (keys) {
        return keys.map(function (k) {
          return k[1];
        });
      });
    });
  }

  function putState(state) {
    return open().then(function (db) {
      return done(db.transaction(STATE, "readwrite").objectStore(STATE).put(state));
    });
  }

  function getState(tabId) {
    return open().then(function (db) {
      return done(db.transaction(STATE).objectStore(STATE).get(tabId));
    });
  }

  // Discard the tab's recording entirely: its state record and every frame.
  function clear(tabId) {
    return open().then(function (db) {
      return new Promise(function (resolve, reject) {
        var t = db.transaction([FRAMES, STATE], "readwrite");
        t.oncomplete = function () {
          resolve();
        };
        t.onerror = function () {
          reject(t.error);
        };
        t.objectStore(STATE).delete(tabId);
        var cursorReq = t.objectStore(FRAMES).index("by_tab").openCursor(IDBKeyRange.only(tabId));
        cursorReq.onsuccess = function () {
          var cursor = cursorReq.result;
          if (cursor) {
            cursor.delete();
            cursor.continue();
          }
        };
      });
    });
  }

  var GhostlightFramestore = {
    putFrame: putFrame,
    deleteFrame: deleteFrame,
    frames: frames,
    frameSeqs: frameSeqs,
    putState: putState,
    getState: getState,
    clear: clear,
  };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightFramestore;
  } else {
    self.GhostlightFramestore = GhostlightFramestore;
  }
})();
