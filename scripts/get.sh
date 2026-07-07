#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# Ghostlight one-line installer (macOS / Linux):
#   curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh
# Downloads the latest release binary, places it in ~/.ghostlight/bin, and runs
# `ghostlight install` (idempotent: registers the native messaging host and any MCP clients
# it finds). Safe to re-run. Set GHOSTLIGHT_NO_REGISTER=1 to download only.

set -eu

REPO="sylin-org/ghostlight"
INSTALL_PAGE="https://sylin-org.github.io/ghostlight/install.html"

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
URL="https://github.com/${REPO}/releases/latest/download/ghostlight-${TARGET}"

mkdir -p "$BIN_DIR"
echo "ghostlight: downloading latest release for ${TARGET}..."
curl -fSL --proto '=https' --tlsv1.2 -o "${BIN}.download" "$URL"
mv "${BIN}.download" "$BIN"
chmod 0755 "$BIN"
echo "ghostlight: installed to ${BIN} ($("$BIN" --version 2>/dev/null || echo version unknown))"

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
