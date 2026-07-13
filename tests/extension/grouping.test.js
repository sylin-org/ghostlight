// SPDX-License-Identifier: Apache-2.0 OR MIT
// Node unit tests for extension/lib/grouping.js (H7 tab-group-per-session presentation; ADR-0030
// Decision 6/7; docs/tasks/hub/H7-tab-group-per-session.md; oracle PINNED in
// docs/tasks/hub/PINS.md SS6).
//
// ORACLE transcribed VERBATIM from docs/adr/0030-ghostlight-hub-orchestrator.md (the source of
// assertion 3's "no policy decision", per the task file's own header comment instruction):
// - Decision 1 topology: "MV3 extension (POLICY-FREE; owns all durable browser state: tabs, tab
//   GROUPS, debugger, console/network buffers, auth/cookies)".
// - Decision 6: "The extension's per-group checks remain defense-in-depth only."
// - Migration H7: "H7 Tab-group-per-session presentation (extension owns the durable group;
//   groups on request only)."

const { test } = require("node:test");
const assert = require("node:assert");
const { groupSessionTabs, managedGroupIds, isManagedGroupId, pruneDeadGroups, reclaimGroupsByTitle } = require("../../extension/lib/grouping.js");

// A minimal fake `chrome.tabs`/`chrome.tabGroups` recording every `chrome.tabs.group` call
// (`groupCalls`, in the shape `{ tabIds: [...], groupId: <number|null> }`) and every
// `chrome.tabGroups.update` call (`updateCalls`). `tabUrls` maps tabId -> url for the fake
// `chrome.tabs.get`; a tabId absent from it behaves like a closed/unknown tab (get() rejects).
function fakeChrome(tabUrls) {
  const groupCalls = [];
  const updateCalls = [];
  let nextGroupId = 1;
  const liveGroupIds = new Set();
  const chrome = {
    tabs: {
      async get(tabId) {
        if (!Object.prototype.hasOwnProperty.call(tabUrls, tabId)) {
          throw new Error(`no such tab ${tabId}`);
        }
        return { id: tabId, url: tabUrls[tabId] };
      },
      async group(opts) {
        const tabIds = [...opts.tabIds].sort((a, b) => a - b);
        const groupId = opts.groupId === undefined ? null : opts.groupId;
        groupCalls.push({ tabIds, groupId });
        if (groupId !== null) {
          liveGroupIds.add(groupId);
          return groupId;
        }
        const id = nextGroupId++;
        liveGroupIds.add(id);
        return id;
      },
    },
    tabGroups: {
      async get(groupId) {
        if (!liveGroupIds.has(groupId)) throw new Error(`no such group ${groupId}`);
        return { id: groupId };
      },
      async update(groupId, opts) {
        updateCalls.push({ groupId, ...opts });
      },
    },
  };
  return { chrome, groupCalls, updateCalls };
}

test("owned_tabs_are_grouped_on_service_request_only", async () => {
  // Assertion 1 -- GROUPS ONLY ON REQUEST: with the fake chrome constructed and NO group request
  // issued, the recorded chrome.tabs.group call list is empty.
  const { groupCalls: neverCalled } = fakeChrome({
    101: "https://a.example/",
    202: "https://b.example/",
  });
  assert.deepStrictEqual(
    neverCalled,
    [],
    "the extension groups nothing on its own, before any group_request"
  );

  // Assertion 2 -- GROUPS EXACTLY THE NAMED TABS: one group request naming tabIds [101, 202] for
  // session "S" groups exactly those tab ids -- none dropped, none added.
  const { chrome: chromeS, groupCalls: callsS } = fakeChrome({
    101: "https://a.example/",
    202: "https://b.example/",
  });
  const sessionGroups = new Map();
  const groupIdS1 = await groupSessionTabs(chromeS, sessionGroups, "S", [101, 202], "title-S");
  assert.strictEqual(callsS.length, 1, "exactly one chrome.tabs.group call for the request");
  assert.deepStrictEqual(
    callsS[0].tabIds,
    [101, 202],
    "grouped exactly the named tabs, none dropped, none added"
  );

  // Assertion 3 -- MAKES NO POLICY DECISION: repeat with the fake chrome reporting one of the
  // named tabs on a plausibly sensitive host. The SAME [101, 202] set is grouped, byte-for-byte
  // identical to assertion 2: the helper never reads the tab's url/host and applies no filter.
  const { chrome: chromeSensitive, groupCalls: callsSensitive } = fakeChrome({
    101: "https://mybank.example/login",
    202: "https://internal.corp/admin",
  });
  const sensitiveSessionGroups = new Map();
  await groupSessionTabs(chromeSensitive, sensitiveSessionGroups, "S", [101, 202], "title-S");
  assert.strictEqual(callsSensitive.length, 1);
  assert.deepStrictEqual(
    callsSensitive[0].tabIds,
    [101, 202],
    "a sensitive-host tab is grouped identically -- no url/host inspection, no filtering"
  );

  // Assertion 4 -- SAME GUID REUSES ITS GROUP; DISTINCT GUID MAKES A NEW GROUP: a second request
  // for session "S" reuses the same groupId (no new group created); a request for a different
  // session "T" creates a distinct group (ADR-0030 Decision 7: "two adapters in one editor -> two
  // GUIDs -> two groups").
  const groupIdS2 = await groupSessionTabs(chromeS, sessionGroups, "S", [101, 202], "title-S");
  assert.strictEqual(
    groupIdS2,
    groupIdS1,
    "the same guid reuses its existing group; no new group is created"
  );
  assert.strictEqual(
    callsS.length,
    2,
    "the reused-group call still records one chrome.tabs.group call, never a chrome.tabGroups.create"
  );
  assert.strictEqual(callsS[1].groupId, groupIdS1, "the reused call carries the existing groupId");

  const groupIdT = await groupSessionTabs(chromeS, sessionGroups, "T", [101], "title-T");
  assert.notStrictEqual(groupIdT, groupIdS1, "a distinct guid gets a distinct group");
});

// ADR-0047 D1 -- the managed-surface predicate (PINS P1). The gate recognizes a tab as in-surface
// when it sits in ANY Ghostlight-managed group: the legacy global group OR any per-session group.
test("managed_surface_accepts_global_and_session_groups", () => {
  const m = new Map([["S", 9], ["T", 12]]);
  assert.deepStrictEqual(
    Array.from(managedGroupIds(7, m)).sort((a, b) => a - b),
    [7, 9, 12]
  );
  assert.strictEqual(isManagedGroupId(9, 7, m), true);
  assert.strictEqual(isManagedGroupId(7, 7, m), true);
});

test("managed_surface_rejects_foreign_and_ungrouped", () => {
  const m = new Map([["S", 9], ["T", 12]]);
  assert.strictEqual(isManagedGroupId(8, 7, m), false);
  assert.strictEqual(isManagedGroupId(-1, 7, m), false);
  assert.strictEqual(isManagedGroupId(5, null, new Map()), false);
  assert.strictEqual(managedGroupIds(null, new Map()).size, 0);
});

// ADR-0047 D5 (PINS P6): pruneDeadGroups drops session-map entries whose Chrome group is gone,
// returns true when it removed anything (the caller persists), and is a no-op returning false on a
// clean map. An INLINE fake here (not the shared fakeChrome helper, whose liveGroupIds set cannot
// express a pre-existing live group) reports group 9 alive and every other group dead.
test("dead_groups_are_pruned_from_the_session_map", async () => {
  const chrome = {
    tabGroups: {
      async get(groupId) {
        if (groupId !== 9) throw new Error(`no such group ${groupId}`);
        return { id: 9 };
      },
    },
  };
  const sessionGroups = new Map([["S", 9], ["T", 12]]);
  assert.strictEqual(await pruneDeadGroups(chrome, sessionGroups), true);
  assert.deepStrictEqual(Array.from(sessionGroups.entries()), [["S", 9]]);
  assert.strictEqual(await pruneDeadGroups(chrome, sessionGroups), false);
});

// ADR-0066 D4: reclaimGroupsByTitle re-attaches groups Chrome restored (after a browser restart
// cleared the persisted map) by stripping the managed title prefix (`\u{1F47B} `, glyph + space)
// back to the clientKey. It maps only titles carrying that exact prefix, ignores everything else
// (including the legacy global title `\u{1F47B}Ghostlight`, which has NO space), and returns
// whether it changed anything.
test("groups_are_reclaimed_by_title_after_a_browser_restart", async () => {
  const PREFIX = "\u{1F47B} ";
  const chrome = {
    tabGroups: {
      async query() {
        return [
          { id: 21, title: PREFIX + "Claude Code" },
          { id: 22, title: PREFIX + "Cursor" },
          { id: 23, title: "My own group" }, // not managed: ignored
          { id: 24, title: "\u{1F47B}Ghostlight" }, // legacy global (no space): ignored by this prefix
        ];
      },
    },
  };
  const clientGroups = new Map();
  assert.strictEqual(await reclaimGroupsByTitle(chrome, clientGroups, PREFIX), true);
  assert.deepStrictEqual(
    Array.from(clientGroups.entries()).sort(),
    [["Claude Code", 21], ["Cursor", 22]],
    "each managed title maps its clientKey to its group id; non-managed titles are ignored"
  );
  // Idempotent: a second pass with everything already mapped changes nothing.
  assert.strictEqual(await reclaimGroupsByTitle(chrome, clientGroups, PREFIX), false);
});

// ADR-0066 D4: reclaim never overwrites an existing live mapping (a key already present) and never
// claims a group id already mapped, so a duplicate title (legacy "... (2)" litter renamed by hand,
// or a stale duplicate) cannot steal a live group; the first title wins and the rest are left as
// orphans rather than re-created for a fresh session.
test("reclaim_does_not_overwrite_a_live_mapping_or_double_claim_a_group_id", async () => {
  const PREFIX = "\u{1F47B} ";
  const chrome = {
    tabGroups: {
      async query() {
        return [
          { id: 30, title: PREFIX + "Claude Code" }, // key already mapped to a different live id
          { id: 12, title: PREFIX + "Zed" }, // id 12 already claimed by another key below
        ];
      },
    },
  };
  const clientGroups = new Map([["Claude Code", 99], ["Cursor", 12]]);
  assert.strictEqual(await reclaimGroupsByTitle(chrome, clientGroups, PREFIX), false);
  assert.deepStrictEqual(
    Array.from(clientGroups.entries()).sort(),
    [["Claude Code", 99], ["Cursor", 12]],
    "a present key is not overwritten, and an already-claimed group id is not re-mapped"
  );
});
