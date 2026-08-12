(function installGhostlightCommandChunks(root, factory) {
  const api = factory();
  root.GhostlightCommandChunks = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis, function createGhostlightCommandChunksApi() {
  "use strict";

  const DEFAULT_MAX_ACTIVE = 2;
  const DEFAULT_MAX_BYTES = 8 * 1024 * 1024;
  const DEFAULT_MAX_TOTAL_BYTES = 12 * 1024 * 1024;
  const DEFAULT_MAX_CHUNKS = 64;
  const DEFAULT_MAX_CHUNK_BYTES = 512 * 1024;
  const DEFAULT_MAX_COMPLETED = 256;
  const DEFAULT_TTL_MS = 15_000;
  const BASE64_PATTERN = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

  function create({
    decodeBase64,
    decodeUtf8,
    sha256Hex,
    now = Date.now,
    setTimer = setTimeout,
    clearTimer = clearTimeout,
    maxActive = DEFAULT_MAX_ACTIVE,
    maxBytes = DEFAULT_MAX_BYTES,
    maxTotalBytes = DEFAULT_MAX_TOTAL_BYTES,
    maxChunks = DEFAULT_MAX_CHUNKS,
    maxChunkBytes = DEFAULT_MAX_CHUNK_BYTES,
    maxCompleted = DEFAULT_MAX_COMPLETED,
    ttlMs = DEFAULT_TTL_MS
  }) {
    if (typeof decodeBase64 !== "function" || typeof decodeUtf8 !== "function" || typeof sha256Hex !== "function") {
      throw new TypeError("command chunks require base64, UTF-8, and SHA-256 adapters");
    }
    if (typeof now !== "function") throw new TypeError("command chunks require a clock");
    if (!Number.isSafeInteger(maxCompleted) || maxCompleted < 1) {
      throw new TypeError("maxCompleted must be a positive integer");
    }
    const transfers = new Map();
    const completedTransfers = new Map();
    let heldBytes = 0;

    function currentTime() {
      const value = Number(now());
      return Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0;
    }

    function erase(transferId) {
      const transfer = transfers.get(transferId);
      if (!transfer) return;
      transfers.delete(transferId);
      transfer.cancelled = true;
      if (transfer.timer) clearTimer(transfer.timer);
      heldBytes = Math.max(0, heldBytes - transfer.receivedBytes);
      for (const chunk of transfer.chunks) {
        if (chunk instanceof Uint8Array) chunk.fill(0);
      }
      transfer.chunks.fill(null);
    }

    function eraseCompleted(transferId, expected = null) {
      const completed = completedTransfers.get(transferId);
      if (!completed || (expected && completed !== expected)) return;
      completedTransfers.delete(transferId);
      if (completed.timer !== null) clearTimer(completed.timer);
    }

    function pruneCompleted() {
      const time = currentTime();
      for (const [transferId, completed] of completedTransfers) {
        if (time < completed.expiresAt) break;
        eraseCompleted(transferId, completed);
      }
    }

    function completedTransfer(transferId) {
      pruneCompleted();
      return completedTransfers.get(transferId) ?? null;
    }

    function rememberCompleted(transferId) {
      pruneCompleted();
      if (completedTransfers.size >= maxCompleted) return false;
      const completed = {
        expiresAt: currentTime() + ttlMs,
        timer: null
      };
      completedTransfers.set(transferId, completed);
      completed.timer = setTimer(() => eraseCompleted(transferId, completed), ttlMs);
      completed.timer?.unref?.();
      return true;
    }

    function clear() {
      for (const transferId of Array.from(transfers.keys())) erase(transferId);
      for (const transferId of Array.from(completedTransfers.keys())) eraseCompleted(transferId);
    }

    function fail(frame, reason, reject, expected = null) {
      if (expected && transfers.get(frame?.transfer_id) !== expected) return;
      if (typeof frame?.transfer_id === "string") erase(frame.transfer_id);
      const correlation = expected?.correlation
        ?? (typeof frame?.correlation === "string" ? frame.correlation : null);
      reject(correlation, reason);
    }

    function valid(frame) {
      return frame?.kind === "command_chunk"
        && typeof frame.transfer_id === "string" && frame.transfer_id.length > 0 && frame.transfer_id.length <= 160
        && typeof frame.correlation === "string" && frame.correlation.length > 0 && frame.correlation.length <= 160
        && Number.isSafeInteger(frame.index) && frame.index >= 0
        && Number.isSafeInteger(frame.count) && frame.count > 0 && frame.count <= maxChunks
        && frame.index < frame.count
        && Number.isSafeInteger(frame.total_bytes) && frame.total_bytes > 0 && frame.total_bytes <= maxBytes
        && frame.total_bytes <= frame.count * maxChunkBytes
        && typeof frame.sha256 === "string" && /^[0-9a-f]{64}$/.test(frame.sha256)
        && typeof frame.data === "string" && frame.data.length > 0
        && frame.data.length % 4 === 0 && BASE64_PATTERN.test(frame.data);
    }

    function accept(frame, deliver, reject) {
      if (typeof frame?.transfer_id === "string" && completedTransfer(frame.transfer_id)) return;
      if (!valid(frame)) {
        const existing = typeof frame?.transfer_id === "string"
          ? transfers.get(frame.transfer_id) ?? null
          : null;
        fail(frame, "invalid chunk metadata", reject, existing);
        return;
      }
      let transfer = transfers.get(frame.transfer_id);
      if (!transfer) {
        pruneCompleted();
        if (completedTransfers.size >= maxCompleted) {
          fail(frame, "command transfer completion ledger is full", reject);
          return;
        }
        if (transfers.size >= maxActive) {
          fail(frame, "too many concurrent command transfers", reject);
          return;
        }
        transfer = {
          correlation: frame.correlation,
          count: frame.count,
          totalBytes: frame.total_bytes,
          sha256: frame.sha256,
          chunks: new Array(frame.count).fill(null),
          received: 0,
          receivedBytes: 0,
          completing: false,
          cancelled: false,
          timer: null,
          expiresAt: currentTime() + ttlMs
        };
        transfers.set(frame.transfer_id, transfer);
        transfer.timer = setTimer(
          () => fail(frame, "command transfer expired", reject, transfer),
          ttlMs
        );
      } else if (currentTime() >= transfer.expiresAt) {
        fail(frame, "command transfer expired", reject, transfer);
        return;
      }
      if (transfer.completing || transfer.correlation !== frame.correlation
        || transfer.count !== frame.count || transfer.totalBytes !== frame.total_bytes
        || transfer.sha256 !== frame.sha256 || transfer.chunks[frame.index] !== null) {
        fail(frame, "duplicate or inconsistent command chunk", reject, transfer);
        return;
      }

      let bytes;
      const remainingBytes = Math.min(
        transfer.totalBytes - transfer.receivedBytes,
        maxTotalBytes - heldBytes,
        maxChunkBytes
      );
      if (remainingBytes < 1 || frame.data.length > 4 * Math.ceil(remainingBytes / 3)) {
        fail(frame, "command transfer memory bound exceeded", reject, transfer);
        return;
      }
      try {
        bytes = decodeBase64(frame.data);
      } catch (_error) {
        fail(frame, "invalid base64 command chunk", reject, transfer);
        return;
      }
      if (!(bytes instanceof Uint8Array) || bytes.length === 0 || bytes.length > maxChunkBytes
        || transfer.receivedBytes + bytes.length > transfer.totalBytes
        || heldBytes + bytes.length > maxTotalBytes) {
        if (bytes instanceof Uint8Array) bytes.fill(0);
        fail(frame, "command transfer memory bound exceeded", reject, transfer);
        return;
      }
      transfer.chunks[frame.index] = bytes;
      transfer.received += 1;
      transfer.receivedBytes += bytes.length;
      heldBytes += bytes.length;
      if (transfer.received !== transfer.count) return;

      transfer.completing = true;
      clearTimer(transfer.timer);
      Promise.resolve().then(async () => {
        if (transfer.receivedBytes !== transfer.totalBytes) {
          fail(frame, "command transfer byte count mismatch", reject, transfer);
          return;
        }
        const joined = new Uint8Array(transfer.totalBytes);
        let offset = 0;
        for (const chunk of transfer.chunks) {
          joined.set(chunk, offset);
          offset += chunk.length;
        }
        const digest = await sha256Hex(joined);
        if (transfer.cancelled || transfers.get(frame.transfer_id) !== transfer) {
          joined.fill(0);
          return;
        }
        if (digest !== transfer.sha256) {
          joined.fill(0);
          fail(frame, "command transfer digest mismatch", reject, transfer);
          return;
        }
        if (currentTime() >= transfer.expiresAt) {
          joined.fill(0);
          fail(frame, "command transfer expired", reject, transfer);
          return;
        }
        let request;
        try {
          request = JSON.parse(decodeUtf8(joined));
        } catch (_error) {
          joined.fill(0);
          fail(frame, "command transfer is not UTF-8 JSON", reject, transfer);
          return;
        }
        joined.fill(0);
        if (request?.kind !== "request" || request.request?.correlation !== transfer.correlation) {
          fail(frame, "command transfer correlation mismatch", reject, transfer);
          return;
        }
        if (transfer.cancelled || transfers.get(frame.transfer_id) !== transfer) {
          joined.fill(0);
          return;
        }
        if (!rememberCompleted(frame.transfer_id)) {
          fail(frame, "command transfer completion ledger is full", reject, transfer);
          return;
        }
        erase(frame.transfer_id);
        deliver(request);
      }).catch(() => fail(frame, "command transfer verification failed", reject, transfer));
    }

    return Object.freeze({
      accept,
      clear,
      stats: () => ({ active: transfers.size, bytes: heldBytes, completed: completedTransfers.size })
    });
  }

  return Object.freeze({ DEFAULT_MAX_COMPLETED, create });
});
