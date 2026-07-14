// SPDX-License-Identifier: Apache-2.0 OR MIT
// Bounded, memory-only host-message reassembly (ADR-0074). Mechanism only.
(function initWireChunks(root) {
  "use strict";

  function createWireChunkStore(options) {
    const decodeBase64 = options.decodeBase64;
    const sha256Hex = options.sha256Hex;
    const decodeUtf8 = options.decodeUtf8;
    const setTimer = options.setTimer || setTimeout;
    const clearTimer = options.clearTimer || clearTimeout;
    const maxActive = options.maxActive || 2;
    const maxBytes = options.maxBytes || 32 * 1024 * 1024;
    const maxTotalBytes = options.maxTotalBytes || 40 * 1024 * 1024;
    const maxChunks = options.maxChunks || 64;
    const ttlMs = options.ttlMs || 15000;
    const transfers = new Map();
    let totalHeld = 0;

    function erase(transferId) {
      const transfer = transfers.get(transferId);
      if (!transfer) return;
      transfers.delete(transferId);
      if (transfer.expiryTimer) clearTimer(transfer.expiryTimer);
      totalHeld = Math.max(0, totalHeld - transfer.receivedBytes);
      for (const chunk of transfer.chunks) {
        if (chunk instanceof Uint8Array) chunk.fill(0);
      }
      transfer.chunks.fill(null);
    }

    function clear() {
      for (const transferId of Array.from(transfers.keys())) erase(transferId);
    }

    function reject(msg, reason, onReject) {
      if (msg && typeof msg.transferId === "string") erase(msg.transferId);
      if (msg && typeof msg.requestId === "string") onReject(msg.requestId, reason);
    }

    function accept(msg, deliver, onReject) {
      const validHeader =
        msg && typeof msg.transferId === "string" && msg.transferId.length > 0 &&
        typeof msg.requestId === "string" && msg.requestId.length > 0 &&
        Number.isSafeInteger(msg.index) && Number.isSafeInteger(msg.count) &&
        Number.isSafeInteger(msg.totalBytes) && typeof msg.sha256 === "string" &&
        typeof msg.data === "string" && msg.index >= 0 && msg.index < msg.count &&
        msg.count > 0 && msg.count <= maxChunks && msg.totalBytes > 0 &&
        msg.totalBytes <= maxBytes && /^[0-9a-f]{64}$/.test(msg.sha256);
      if (!validHeader) {
        reject(msg, "invalid chunk metadata", onReject);
        return;
      }

      let transfer = transfers.get(msg.transferId);
      if (!transfer) {
        if (transfers.size >= maxActive) {
          reject(msg, "too many concurrent transfers", onReject);
          return;
        }
        transfer = {
          requestId: msg.requestId,
          count: msg.count,
          totalBytes: msg.totalBytes,
          sha256: msg.sha256,
          chunks: new Array(msg.count).fill(null),
          received: 0,
          receivedBytes: 0,
          completing: false,
          expiryTimer: null,
        };
        transfer.expiryTimer = setTimer(
          () => reject(msg, "transfer expired", onReject),
          ttlMs
        );
        transfers.set(msg.transferId, transfer);
      }
      if (transfer.completing || transfer.requestId !== msg.requestId ||
          transfer.count !== msg.count || transfer.totalBytes !== msg.totalBytes ||
          transfer.sha256 !== msg.sha256 || transfer.chunks[msg.index] !== null) {
        reject(msg, "duplicate or inconsistent chunk", onReject);
        return;
      }

      let bytes;
      try { bytes = decodeBase64(msg.data); } catch {
        reject(msg, "invalid base64 chunk", onReject);
        return;
      }
      if (!(bytes instanceof Uint8Array) || totalHeld + bytes.length > maxTotalBytes ||
          transfer.receivedBytes + bytes.length > transfer.totalBytes) {
        reject(msg, "memory bound exceeded", onReject);
        return;
      }
      transfer.chunks[msg.index] = bytes;
      transfer.received += 1;
      transfer.receivedBytes += bytes.length;
      totalHeld += bytes.length;
      if (transfer.received !== transfer.count) return;
      transfer.completing = true;
      clearTimer(transfer.expiryTimer);

      (async () => {
        if (transfer.receivedBytes !== transfer.totalBytes) {
          reject(msg, "byte count mismatch", onReject);
          return;
        }
        const joined = new Uint8Array(transfer.totalBytes);
        let offset = 0;
        for (const chunk of transfer.chunks) {
          joined.set(chunk, offset);
          offset += chunk.length;
        }
        if (await sha256Hex(joined) !== transfer.sha256) {
          joined.fill(0);
          reject(msg, "digest mismatch", onReject);
          return;
        }
        let request;
        try { request = JSON.parse(decodeUtf8(joined)); } catch {
          joined.fill(0);
          reject(msg, "request is not valid UTF-8 JSON", onReject);
          return;
        }
        if (!request || request.id !== transfer.requestId) {
          joined.fill(0);
          reject(msg, "request identity mismatch", onReject);
          return;
        }
        erase(msg.transferId);
        joined.fill(0);
        deliver(request);
      })().catch(() => reject(msg, "verification failed", onReject));
    }

    return {
      accept,
      clear,
      stats: () => ({ active: transfers.size, bytes: totalHeld }),
    };
  }

  const GhostlightWireChunks = { createWireChunkStore };
  if (typeof module !== "undefined" && module.exports) {
    module.exports = GhostlightWireChunks;
  } else {
    root.GhostlightWireChunks = GhostlightWireChunks;
  }
})(typeof self !== "undefined" ? self : globalThis);
