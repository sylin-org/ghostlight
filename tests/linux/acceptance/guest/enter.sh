#!/bin/bash
set -euo pipefail
area=${GHOSTLIGHT_LINUX_GUEST_AREA:?Run through tests/linux/run.sh}
toolchain=${GHOSTLIGHT_LINUX_RUST_TOOLCHAIN_DIR:-$(dirname "$(dirname "$(rustup which rustc)")")}
exec unshare --user --map-auto --map-root-user --mount --pid --fork \
 bwrap --bind "$area/root" / --proc /proc --dev /dev --ro-bind /sys /sys \
 --bind "$area/build" /work --ro-bind "$GHOSTLIGHT_LINUX_POWERSHELL_DIR" /opt/pwsh --ro-bind "$toolchain" /opt/rust \
 --ro-bind "$GHOSTLIGHT_LINUX_CARGO_HOME/registry" /cargo/registry --ro-bind "$GHOSTLIGHT_LINUX_CARGO_HOME/git" /cargo/git --chdir /work \
 --setenv PATH /opt/rust/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
 --setenv CARGO_HOME /cargo --setenv CARGO_TARGET_DIR /work/target "$@"
