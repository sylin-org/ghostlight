#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# curl -fsSL https://sylin.org/ghostlight/get.sh | sh
#
# The Ghostlight 2.0 extension's setup page prints the one-line command that fetches this file;
# keep its URL working for 2.x.
#
# It installs the images for the system's C library: glibc, or musl on Alpine and its kind.
#
# GHOSTLIGHT_VERSION installs exactly that release instead of the latest one, found by its tag
# (v<version>) through the GitHub releases API. A prerelease is installable this way and no other:
# the latest release never is:
#   curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | GHOSTLIGHT_VERSION=2.0.0 sh
#
# GHOSTLIGHT_FROM installs from a release bundle on this machine instead of GitHub, for testing a
# build before it is released. It names a folder: the release workflow's `release-candidate`
# artifact, unpacked (SHA256SUMS beside an assets folder), or a folder of release downloads
# (SHA256SUMS beside the images). Every image is checked against SHA256SUMS exactly as a download
# is, and anything it does not bind is refused. A bundle is only as trustworthy as its source; with
# the GitHub CLI installed, the build provenance is checked as well. It cannot be combined with
# GHOSTLIGHT_VERSION:
#   curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/<revision>/scripts/get.sh | GHOSTLIGHT_FROM=~/Downloads/release-candidate sh
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

# The Linux images exist for both C libraries: glibc for most systems, musl for Alpine and its
# kind. ldd belongs to the system's own C library and names it; a system without a readable ldd is
# musl when it has musl's loader and no glibc loader.
linux_target() {
  case "$(ldd --version 2>&1 || true)" in
    *musl*) echo "x86_64-unknown-linux-musl" ;;
    *GNU*|*GLIBC*|*glibc*) echo "x86_64-unknown-linux-gnu" ;;
    *)
      if [ -e /lib/ld-musl-x86_64.so.1 ] && [ ! -e /lib64/ld-linux-x86-64.so.2 ]; then
        echo "x86_64-unknown-linux-musl"
      else
        echo "x86_64-unknown-linux-gnu"
      fi
      ;;
  esac
}

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=$(linux_target) ;;
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
from=${GHOSTLIGHT_FROM:-}
if [ -n "$from" ]; then
  if [ -n "${GHOSTLIGHT_VERSION:-}" ]; then
    echo "ghostlight: GHOSTLIGHT_FROM and GHOSTLIGHT_VERSION cannot be combined" >&2
    exit 1
  fi
  if [ ! -d "$from" ]; then
    echo "ghostlight: GHOSTLIGHT_FROM is not a folder: ${from}" >&2
    exit 1
  fi
  # The release workflow's bundle keeps its images in assets/; a folder of release downloads keeps
  # them beside SHA256SUMS.
  images=$from
  if [ -d "$from/assets" ]; then
    images="$from/assets"
  fi
  if [ ! -f "$from/SHA256SUMS" ]; then
    echo "ghostlight: ${from} has no SHA256SUMS" >&2
    exit 1
  fi
  cp "$from/SHA256SUMS" "$sums"
  # The bundle's manifest names its version; a folder of downloads names none.
  version=""
  if [ -f "$from/release-candidate.json" ]; then
    version=$(sed -n 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$from/release-candidate.json" | head -n 1)
  fi
  if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'; then
    version=""
  fi
elif [ -n "${GHOSTLIGHT_VERSION:-}" ]; then
  # A requested version is installed exactly, whatever the latest release is, prerelease included.
  version=$GHOSTLIGHT_VERSION
  if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$'; then
    echo "ghostlight: GHOSTLIGHT_VERSION must look like 2.0.0 or 2.0.0-rc.1" >&2
    exit 1
  fi
  tag="v${version}"
  release_page="$temporary/release.json"
  # No -f: curl would answer exit 22 to every HTTP error, and a rate limit (403) or a server error
  # (5xx) is not an absent release. Only the status code says which it is.
  status=0
  code=$(curl -sSL --proto '=https' --tlsv1.2 -o "$release_page" -w '%{http_code}' "https://api.github.com/repos/${repository}/releases/tags/${tag}") || status=$?
  if [ "$status" -ne 0 ]; then
    echo "ghostlight: could not look up release ${tag}" >&2
    exit 1
  fi
  case "$code" in
    200) ;;
    404)
      echo "ghostlight: release ${tag} does not exist" >&2
      exit 1
      ;;
    *)
      echo "ghostlight: could not look up release ${tag} (HTTP ${code})" >&2
      exit 1
      ;;
  esac
  found=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$release_page" | head -n 1)
  if [ "$found" != "$tag" ]; then
    echo "ghostlight: release ${tag} does not exist" >&2
    exit 1
  fi
else
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
fi
if [ -z "$from" ]; then
  release_root="https://github.com/${repository}/releases/download/${tag}"
  curl -fsSL --proto '=https' --tlsv1.2 -o "$sums" "${release_root}/SHA256SUMS"
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
  if [ -n "$from" ]; then
    if [ ! -f "$images/$asset" ]; then
      echo "ghostlight: ${images} has no ${asset}" >&2
      exit 1
    fi
    # The copy is what runs, so the copy is what is checked.
    cp "$images/$asset" "$downloaded"
  else
    curl -fsSL --proto '=https' --tlsv1.2 -o "$downloaded" "${release_root}/${asset}"
  fi
  if [ "$(sha256_file "$downloaded")" != "$expected" ]; then
    echo "ghostlight: checksum verification failed for ${asset}" >&2
    exit 1
  fi
  if ! { command -v gh >/dev/null 2>&1 && gh attestation verify "$downloaded" --repo "$repository" >/dev/null 2>&1; }; then
    provenance_for_all=0
  fi
  chmod 0755 "$downloaded"
done

product="Ghostlight${version:+ ${version}}"
obtained="downloaded"
if [ -n "$from" ]; then
  obtained="copied from ${from}"
fi
if [ "$provenance_for_all" -eq 1 ]; then
  echo "${product} ${obtained}. Checksums and build provenance verified."
else
  echo "${product} ${obtained}. Checksums verified."
fi

# The verified service installs the whole set (the edge, the service and the shell) into the
# Ghostlight home from its own directory. Its stdout is discarded; its errors still show. A failed
# download cannot partially replace a working installation.
service="$temporary/ghostlight-service"
if ! "$service" deployment install $restore >/dev/null; then
  echo "ghostlight: installation failed" >&2
  exit 1
fi

if [ "${GHOSTLIGHT_NO_REGISTER:-0}" = "1" ]; then
  echo "${product} is installed. Run 'ghostlight install' to connect your browsers and agents."
else
  # The same verified service hands `install` to the copy it just installed, which prints the
  # install summary in this console.
  if ! "$service" install; then
    # The ghostlight command may not exist yet, so the same downloaded service, which reads the
    # same home, reports what it finds before its temporary directory goes.
    "$service" doctor || true
    echo "ghostlight: installation did not complete" >&2
    exit 1
  fi
fi

# The window links the system's WebKitGTK 4.1. Where its libraries are missing (a fresh Alpine
# desktop often lacks them) the window cannot start, while the service, the browser connection and
# the agents work. ldd names what is missing, so say what to install.
if command -v ldd >/dev/null 2>&1 &&
  ldd "$temporary/ghostlight-desktop" 2>&1 | grep -q -E 'not found|Error loading shared library'; then
  echo "ghostlight: the Ghostlight window needs WebKitGTK 4.1, which is not installed. Browsers and agents work without it."
  case "$target" in
    *-musl) echo "ghostlight: On Alpine, add it with: doas apk add webkit2gtk-4.1 libayatana-appindicator" ;;
    *) echo "ghostlight: Install your system's WebKitGTK 4.1 package to have the window." ;;
  esac
fi
