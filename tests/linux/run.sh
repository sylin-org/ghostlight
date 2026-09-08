#!/bin/bash
# Run an opt-in Linux acceptance driver with environment-specific fixture paths.
set -euo pipefail

scripts=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repository=$(cd "$scripts/../.." && pwd)
cd "$repository"
driver=${1:?usage: tests/linux/run.sh DRIVER [ARGUMENTS...] (see tests/linux/README.md)}
shift
case "$driver" in
    --print-env) ;;
    installed-journey.mjs|installed/*.py|installed/*.mjs|acceptance/*.mjs|acceptance/*.py|acceptance/guest/*.sh|acceptance/guest/*.py) ;;
    *) echo "Unknown Linux driver: $driver" >&2; exit 2 ;;
esac
if test "$driver" != --print-env; then test -f "$scripts/$driver"; fi

export GHOSTLIGHT_LINUX_VERSION=${GHOSTLIGHT_LINUX_VERSION:-$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"])')}
export GHOSTLIGHT_LINUX_PREVIOUS_VERSION=${GHOSTLIGHT_LINUX_PREVIOUS_VERSION:-1.3.4}
export GHOSTLIGHT_LINUX_AREA=${GHOSTLIGHT_LINUX_AREA:-$repository/.tmp/linux-local}
export GHOSTLIGHT_LINUX_INSTALLED_AREA=${GHOSTLIGHT_LINUX_INSTALLED_AREA:-$repository/.tmp/linux-installed}
export GHOSTLIGHT_LINUX_BIN_DIR=${GHOSTLIGHT_LINUX_BIN_DIR:-$GHOSTLIGHT_LINUX_AREA/portable-a/ghostlight-v$GHOSTLIGHT_LINUX_VERSION-x86_64-unknown-linux-gnu}
export GHOSTLIGHT_LINUX_PORTABLE_ARCHIVE=${GHOSTLIGHT_LINUX_PORTABLE_ARCHIVE:-$GHOSTLIGHT_LINUX_AREA/portable-fixed-a/ghostlight-v$GHOSTLIGHT_LINUX_VERSION-x86_64-unknown-linux-gnu.tar.gz}
export GHOSTLIGHT_LINUX_CHROMIUM=${GHOSTLIGHT_LINUX_CHROMIUM:-/usr/lib/chromium/chromium}
export GHOSTLIGHT_LINUX_BRAVE=${GHOSTLIGHT_LINUX_BRAVE:-$GHOSTLIGHT_LINUX_AREA/brave/opt/brave.com/brave/brave}
export GHOSTLIGHT_LINUX_EXTENSION=${GHOSTLIGHT_LINUX_EXTENSION:-$repository/extension}
export GHOSTLIGHT_LINUX_GUEST_AREA=${GHOSTLIGHT_LINUX_GUEST_AREA:-$GHOSTLIGHT_LINUX_AREA/guest}
export GHOSTLIGHT_LINUX_POWERSHELL_DIR=${GHOSTLIGHT_LINUX_POWERSHELL_DIR:-$repository/.tmp/linux-tools/powershell-7.6.5}
export GHOSTLIGHT_LINUX_CARGO_HOME=${GHOSTLIGHT_LINUX_CARGO_HOME:-${CARGO_HOME:-$HOME/.cargo}}
export GHOSTLIGHT_LINUX_DEB=${GHOSTLIGHT_LINUX_DEB:-$GHOSTLIGHT_LINUX_GUEST_AREA/build/target/x86_64-unknown-linux-gnu/release/bundle/deb/Ghostlight_${GHOSTLIGHT_LINUX_VERSION}_amd64.deb}
export GHOSTLIGHT_LINUX_PREVIOUS_DEB=${GHOSTLIGHT_LINUX_PREVIOUS_DEB:-$GHOSTLIGHT_LINUX_GUEST_AREA/build/public/ghostlight-v$GHOSTLIGHT_LINUX_PREVIOUS_VERSION-x86_64-unknown-linux-gnu.deb}
# Paths cross host/guest boundaries; resolve overrides before entering another working directory.
for name in GHOSTLIGHT_LINUX_AREA GHOSTLIGHT_LINUX_INSTALLED_AREA GHOSTLIGHT_LINUX_BIN_DIR \
    GHOSTLIGHT_LINUX_PORTABLE_ARCHIVE GHOSTLIGHT_LINUX_CHROMIUM GHOSTLIGHT_LINUX_BRAVE \
    GHOSTLIGHT_LINUX_EXTENSION GHOSTLIGHT_LINUX_GUEST_AREA GHOSTLIGHT_LINUX_POWERSHELL_DIR \
    GHOSTLIGHT_LINUX_CARGO_HOME GHOSTLIGHT_LINUX_DEB GHOSTLIGHT_LINUX_PREVIOUS_DEB; do
    export "$name=$(realpath -m "${!name}")"
done

if test "$driver" = --print-env; then
    while IFS= read -r name; do printf '%s=%q\n' "$name" "${!name}"; done < <(compgen -v GHOSTLIGHT_LINUX_)
    exit 0
fi

case "$driver" in
    *.mjs) exec node "$scripts/$driver" "$@" ;;
    *.py) exec python3 "$scripts/$driver" "$@" ;;
    *.sh) exec bash "$scripts/$driver" "$@" ;;
esac
