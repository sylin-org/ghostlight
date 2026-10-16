#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# curl -fsSL https://sylin.org/ghostlight/get.sh | sh
#
# The Ghostlight 2.0 extension's setup page prints the one-line command that fetches this file;
# keep its URL working for 2.x.
#
# --restore installs this release over a development installation (a build the development loop
# installed), whatever version it is: the one allowed move down. It runs the downloaded service
# with `deployment install --restore` instead of `deployment install`:
#   curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh -s -- --restore

set -eu

restore=""
for argument in "$@"; do
  case "$argument" in
    --restore) restore="--restore" ;;
    *) echo "ghostlight: unknown option ${argument}" >&2; exit 2 ;;
  esac
done

repository="sylin-org/ghostlight"
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
# Resolve the release page first: asset downloads end at GitHub's signed CDN URL,
# which does not carry the release tag. Pin one tag for checksums and every binary.
latest="https://github.com/${repository}/releases/latest"
resolved=$(curl -fsSIL --proto '=https' --tlsv1.2 -o /dev/null -w '%{url_effective}' "$latest")
case "$resolved" in
  "https://github.com/${repository}/releases/tag/"v*) ;;
  *) echo "ghostlight: latest release resolved to an unexpected URL" >&2; exit 1 ;;
esac
tag=${resolved##*/}
version=${tag#v}
if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "ghostlight: latest release tag is not a three-part version" >&2
  exit 1
fi
if [ -n "${GHOSTLIGHT_VERSION:-}" ] && [ "$GHOSTLIGHT_VERSION" != "$version" ]; then
  echo "ghostlight: latest release is ${version}, not requested ${GHOSTLIGHT_VERSION}" >&2
  exit 1
fi
release_root="https://github.com/${repository}/releases/download/${tag}"
curl -fsSL --proto '=https' --tlsv1.2 -o "$sums" "${release_root}/SHA256SUMS"

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

install_directory="${HOME}/.ghostlight/bin"
provenance_for_all=1
for component in ghostlight ghostlight-service ghostlight-desktop; do
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
  if ! { command -v gh >/dev/null 2>&1 && gh attestation verify "$downloaded" --repo "$repository" >/dev/null 2>&1; }; then
    provenance_for_all=0
  fi
  chmod 0755 "$downloaded"
done

if [ "$provenance_for_all" -eq 1 ]; then
  echo "Ghostlight ${version} downloaded. Checksums and build provenance verified."
else
  echo "Ghostlight ${version} downloaded. Checksums verified."
fi

# The verified service installs the whole set (the edge, the service and the shell) into
# ~/.ghostlight/bin from its own directory. Its stdout is discarded; its errors still show. A
# failed download cannot partially replace a working installation.
if ! "$temporary/ghostlight-service" deployment install $restore >/dev/null; then
  echo "ghostlight: installation failed" >&2
  exit 1
fi

ghostlight="$install_directory/ghostlight"
if [ "${GHOSTLIGHT_NO_REGISTER:-0}" = "1" ]; then
  echo "Ghostlight ${version} is installed in ${install_directory}. Run 'ghostlight install' to connect your browsers and agents."
else
  # The installed edge prints the install summary in this console.
  if ! "$ghostlight" install; then
    echo "ghostlight: installation did not complete. Run '${ghostlight} doctor' for details." >&2
    exit 1
  fi
fi
