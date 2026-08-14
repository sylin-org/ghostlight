#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh

set -eu

repository="sylin-org/ghostlight"
walkthrough="https://sylin.org/ghostlight/chromium-extension/post-install/"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
  *)
    echo "ghostlight: no published binary for $(uname -s)/$(uname -m)" >&2
    exit 1
    ;;
esac

temporary=$(mktemp -d "${TMPDIR:-/tmp}/ghostlight-install.XXXXXX")
cleanup() {
  case "$temporary" in
    "${TMPDIR:-/tmp}"/ghostlight-install.*) rm -rf "$temporary" ;;
  esac
}
trap cleanup EXIT HUP INT TERM

sums="$temporary/SHA256SUMS"
latest="https://github.com/${repository}/releases/latest/download/SHA256SUMS"
resolved=$(curl -fsSL --proto '=https' --tlsv1.2 -o "$sums" -w '%{url_effective}' "$latest")
case "$resolved" in
  "https://github.com/${repository}/releases/download/"v*/SHA256SUMS) ;;
  *) echo "ghostlight: release checksums resolved to an unexpected URL" >&2; exit 1 ;;
esac
release_root=${resolved%/SHA256SUMS}
tag=${release_root##*/}
version=${tag#v}
if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "ghostlight: latest release tag is not a three-part version" >&2
  exit 1
fi
if [ -n "${GHOSTLIGHT_VERSION:-}" ] && [ "$GHOSTLIGHT_VERSION" != "$version" ]; then
  echo "ghostlight: latest release is ${version}, not requested ${GHOSTLIGHT_VERSION}" >&2
  exit 1
fi

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "ghostlight: sha256sum or shasum is required" >&2
    exit 1
  fi
}

install_directory="${HOME}/.ghostlight/bin/v${version}"
mkdir -p "$install_directory"
for component in ghostlight ghostlight-mcp-connector ghostlight-browser-connector; do
  asset="${component}-${target}"
  expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$sums")
  case "$expected" in
    *[!0-9a-f]*|'') echo "ghostlight: invalid checksum for ${asset}" >&2; exit 1 ;;
  esac
  if [ "${#expected}" -ne 64 ]; then
    echo "ghostlight: SHA256SUMS does not bind ${asset}" >&2
    exit 1
  fi
  if [ "$(awk -v name="$asset" '$2 == name { count += 1 } END { print count + 0 }' "$sums")" -ne 1 ]; then
    echo "ghostlight: SHA256SUMS contains duplicate entries for ${asset}" >&2
    exit 1
  fi
  downloaded="$temporary/$component"
  curl -fsSL --proto '=https' --tlsv1.2 -o "$downloaded" "${release_root}/${asset}"
  if [ "$(sha256_file "$downloaded")" != "$expected" ]; then
    echo "ghostlight: checksum verification failed for ${asset}" >&2
    exit 1
  fi
  if command -v gh >/dev/null 2>&1 && gh attestation verify "$downloaded" --repo "$repository" >/dev/null 2>&1; then
    echo "  ${component}: checksum and build provenance verified"
  else
    echo "  ${component}: checksum verified"
  fi
  chmod 0755 "$downloaded"
  mv -f "$downloaded" "$install_directory/$component"
done

ghostlight="$install_directory/ghostlight"
echo "Ghostlight ${version} installed at ${install_directory}"
if [ "${GHOSTLIGHT_NO_REGISTER:-0}" != "1" ]; then
  "$ghostlight" install
fi
echo "Run '${ghostlight} doctor' to check the installation."
echo "Browser walkthrough: ${walkthrough}"
