#!/bin/bash
set -euo pipefail
area=${GHOSTLIGHT_LINUX_GUEST_AREA:?Run through tests/linux/run.sh}
scripts=$(cd "$(dirname "$0")" && pwd)
image=${1:?image}; shift
case "$image" in debian|ubuntu) ;; *) exit 2;; esac
previous=()
if test -f "$GHOSTLIGHT_LINUX_PREVIOUS_DEB"; then
 previous=(--ro-bind "$GHOSTLIGHT_LINUX_PREVIOUS_DEB" /previous.deb)
fi
exec unshare --user --map-auto --map-root-user --mount --pid --fork \
 bwrap --bind "$area/$image/root" / --proc /proc --dev /dev --ro-bind /sys /sys \
 --ro-bind "$scripts" /linux-tests --ro-bind "$GHOSTLIGHT_LINUX_DEB" /candidate.deb "${previous[@]}" \
 --ro-bind "$area/build" /work --chdir /work --unshare-ipc --unshare-uts \
 --setenv GHOSTLIGHT_LINUX_DISPOSABLE_GUEST 1 \
 --setenv GHOSTLIGHT_LINUX_DEB /candidate.deb --setenv GHOSTLIGHT_LINUX_PREVIOUS_DEB /previous.deb \
 --unsetenv DISPLAY --unsetenv WAYLAND_DISPLAY --unsetenv DBUS_SESSION_BUS_ADDRESS \
 --unsetenv XDG_RUNTIME_DIR --setenv PATH /usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin "$@"
