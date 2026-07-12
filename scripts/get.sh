#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# Ghostlight one-line installer (macOS / Linux):
#   curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh
# Downloads the latest release binary, places it in ~/.ghostlight/bin, and runs
# `ghostlight install` (idempotent: registers the native messaging host and any MCP clients
# it finds). Safe to re-run. Set GHOSTLIGHT_NO_REGISTER=1 to download only.

set -eu

REPO="sylin-org/ghostlight"
INSTALL_PAGE="https://sylin.org/ghostlight/"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) TARGET="aarch64-apple-darwin" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  Linux-x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
  *)
    echo "ghostlight: no prebuilt binary for $(uname -s)/$(uname -m)." >&2
    echo "See ${INSTALL_PAGE} for source builds." >&2
    exit 1
    ;;
esac

BIN_DIR="${HOME}/.ghostlight/bin"
BIN="${BIN_DIR}/ghostlight"

# SHA-256 of a file, via whichever tool this platform ships (macOS: shasum; Linux: sha256sum).
# Empty when neither exists, which the caller treats as "cannot verify -> refuse".
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo ""
  fi
}

# Download one release binary and VERIFY it before trusting it (SEC-MED-06). These installers
# register a native-messaging host wired to your browser, so an unverified binary is high-impact.
# Mandatory gate: SHA-256 against the release's published checksum (catches corruption and
# transport tampering); the install refuses if it cannot compute or match the hash. Best-effort
# escalation: cryptographic build provenance via `gh attestation verify`, which -- unlike a
# co-located checksum -- a release-asset swap cannot forge; a gh miss is a warning, not a stop,
# since gh may be absent or unauthenticated. The manual installer verifies provenance
# unconditionally (see ${INSTALL_PAGE}).
download_and_verify() {
  b="$1"
  url="https://github.com/${REPO}/releases/latest/download/${b}-${TARGET}"
  tmp="${BIN_DIR}/${b}.download"
  curl -fSL --proto '=https' --tlsv1.2 -o "$tmp" "$url"

  expected=$(curl -fsSL --proto '=https' --tlsv1.2 "${url}.sha256" 2>/dev/null | awk '{print $1}')
  actual=$(sha256_of "$tmp")
  if [ -z "$actual" ]; then
    rm -f "$tmp"
    echo "ghostlight: no sha256 tool (sha256sum/shasum) to verify the download; refusing to install unverified. See ${INSTALL_PAGE}." >&2
    exit 1
  fi
  if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    rm -f "$tmp"
    echo "ghostlight: checksum verification failed for ${b}-${TARGET} (expected '${expected}', got '${actual}'); aborting." >&2
    exit 1
  fi

  if command -v gh >/dev/null 2>&1 && gh attestation verify "$tmp" --repo "$REPO" >/dev/null 2>&1; then
    echo "  ${b}: sha256 + build provenance verified"
  elif command -v gh >/dev/null 2>&1; then
    echo "  ${b}: sha256 verified (gh could not confirm provenance; verify manually via ${INSTALL_PAGE})"
  else
    echo "  ${b}: sha256 verified (install GitHub CLI 'gh' to also verify cryptographic build provenance)"
  fi

  mv "$tmp" "${BIN_DIR}/${b}"
  chmod 0755 "${BIN_DIR}/${b}"
}

mkdir -p "$BIN_DIR"
echo "ghostlight: downloading latest release for ${TARGET}..."
# ADR-0046 + ADR-0051 Phase 3: two executables ship together (the ghostlight brain + the single
# ghostlight-relay pass-through). They sit in one dir, so `ghostlight install` finds the relay sibling.
for b in ghostlight ghostlight-relay; do
  download_and_verify "$b"
done
echo "ghostlight: installed to ${BIN_DIR} ($("$BIN" --version 2>/dev/null || echo version unknown))"

if [ "${GHOSTLIGHT_NO_REGISTER:-0}" != "1" ]; then
  echo "ghostlight: registering (native messaging host + detected MCP clients)..."
  "$BIN" install
fi

# PATH convenience only; registration above uses absolute paths, so this is optional.
case ":${PATH}:" in
  *":${BIN_DIR}:"*) : ;;
  *) echo "ghostlight: tip: add ${BIN_DIR} to your PATH for the ghostlight CLI (doctor, config, policy)." ;;
esac

echo ""
echo "Next: add the 'Ghostlight in Browser' extension, then reload your MCP client."
echo "Walkthrough: ${INSTALL_PAGE}"
