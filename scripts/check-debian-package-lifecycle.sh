#!/bin/bash
# SPDX-License-Identifier: Apache-2.0 OR MIT

set -euo pipefail

artifact=${1:?usage: check-debian-package-lifecycle.sh ARTIFACT [DISTRIBUTION] [STATE_LABEL]}
distribution=${2:-unknown}
state_label=${3:-$(sha256sum "$artifact" | cut -c 1-12)}
case "$state_label" in
    *[!a-zA-Z0-9.-]*) echo "state label contains unsupported characters: $state_label" >&2; exit 2 ;;
esac
artifact=$(realpath "$artifact")
test_home=/home/ghostlight-package-test-$state_label
runtime=$test_home/.cache/ghostlight/ghostlight-runtime.json
service_pid=

cleanup_service() {
    if test -n "$service_pid"; then
        kill "$service_pid" 2>/dev/null || true
        wait "$service_pid" 2>/dev/null || true
    fi
    pkill -f '^/usr/bin/ghostlight --headless$' 2>/dev/null || true
}
trap cleanup_service EXIT

echo "distribution=$distribution"
grep -E '^(PRETTY_NAME|VERSION_ID|ID)=' /etc/os-release
echo "kernel=$(uname -r)"
echo "candidate_sha256=$(sha256sum "$artifact" | cut -d ' ' -f 1)"

if dpkg-query -W ghostlight >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get purge -y ghostlight
fi
DEBIAN_FRONTEND=noninteractive apt-get install -y "$artifact"
test "$(ghostlight --version)" = "ghostlight 1.0.0"
test "$(dpkg-query -W -f='${Version}' ghostlight)" = "1.0.0"
test "$(dpkg-query -W -f='${Maintainer}' ghostlight)" = "Leonardo Botinelly <hello@sylin.org>"
test "$(dpkg-query -W -f='${Section}' ghostlight)" = "utils"
dpkg-query -W -f='${Depends}' ghostlight | grep -Fq 'libc6 (>= 2.34)'
test -z "$(dpkg --verify ghostlight)"

for binary in ghostlight ghostlight-mcp-connector ghostlight-browser-connector; do
    path=/usr/bin/$binary
    test -x "$path"
    test "$(stat -c %a "$path")" = "755"
    ! ldd "$path" | grep -Fq 'not found'
    ! readelf -d "$path" | grep -Eq '(RPATH|RUNPATH)'
    max_glibc=$(readelf --version-info "$path" | grep -o 'GLIBC_[0-9.]*' | sort -V | tail -1)
    echo "$binary max_glibc=${max_glibc#GLIBC_}"
    test "$(printf '%s\n2.35\n' "${max_glibc#GLIBC_}" | sort -V | tail -1)" = "2.35"
done

for directory in \
    /etc/opt/chrome/native-messaging-hosts \
    /etc/opt/edge/native-messaging-hosts \
    /etc/brave/native-messaging-hosts \
    /etc/chromium/native-messaging-hosts; do
    manifest=$directory/org.sylin.ghostlight.json
    test -f "$manifest"
    jq -e '.name == "org.sylin.ghostlight" and
        .path == "/usr/bin/ghostlight-browser-connector" and
        .type == "stdio" and
        (.allowed_origins | length) == 2' "$manifest" >/dev/null
    dpkg-query -W -f='${Conffiles}\n' ghostlight | grep -Fq " $manifest "
done

mapfile -t desktops < <(grep -R -l '^X-Ghostlight-Owned=true$' /usr/share/applications)
test "${#desktops[@]}" -eq 1
grep -Eq '^Exec=.*ghostlight.* open$' "${desktops[0]}"
grep -Fqx 'Keywords=browser;automation;MCP;' "${desktops[0]}"
desktop-file-validate "${desktops[0]}"
test -s /usr/share/doc/ghostlight/changelog.gz
test -s /usr/share/doc/ghostlight/copyright
test -z "$(find /usr/bin/ghostlight /usr/bin/ghostlight-mcp-connector \
    /usr/bin/ghostlight-browser-connector /usr/lib/Ghostlight /usr/share/doc/ghostlight \
    -type f -perm /6000 -print)"
test -z "$(find /usr/bin/ghostlight /usr/bin/ghostlight-mcp-connector \
    /usr/bin/ghostlight-browser-connector /usr/lib/Ghostlight /usr/share/doc/ghostlight \
    -type f -perm -0002 -print)"

install -d -m 0700 -o 1000 -g 1000 "$test_home" "$test_home/run"
setpriv --reuid=1000 --regid=1000 --clear-groups \
    env HOME="$test_home" XDG_RUNTIME_DIR="$test_home/run" \
    ghostlight --headless >"$test_home/service.log" 2>&1 &
service_pid=$!
for attempt in $(seq 1 100); do
    test -s "$runtime" && break
    kill -0 "$service_pid"
    sleep 0.05
done
test -s "$runtime"
test "$(stat -c %a "$runtime")" = "600"
setpriv --reuid=1000 --regid=1000 --clear-groups \
    env HOME="$test_home" XDG_RUNTIME_DIR="$test_home/run" ghostlight status |
    grep -Fq 'running'
setpriv --reuid=1000 --regid=1000 --clear-groups \
    env HOME="$test_home" XDG_RUNTIME_DIR="$test_home/run" ghostlight doctor >/dev/null
setpriv --reuid=1000 --regid=1000 --clear-groups \
    env HOME="$test_home" XDG_RUNTIME_DIR="$test_home/run" ghostlight native-host check >/dev/null
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"package-test","version":"1"}}}' |
    setpriv --reuid=1000 --regid=1000 --clear-groups \
        env HOME="$test_home" XDG_RUNTIME_DIR="$test_home/run" \
        timeout 10s ghostlight-mcp-connector >"$test_home/mcp.json"
jq -e '.result.serverInfo.name == "ghostlight" and .result.serverInfo.version == "1.0.0"' \
    "$test_home/mcp.json" >/dev/null
cleanup_service
service_pid=

DEBIAN_FRONTEND=noninteractive apt-get remove -y ghostlight
test ! -e /usr/bin/ghostlight
DEBIAN_FRONTEND=noninteractive apt-get install -y "$artifact"
test "$(ghostlight --version)" = "ghostlight 1.0.0"
DEBIAN_FRONTEND=noninteractive apt-get purge -y ghostlight
! dpkg-query -W ghostlight >/dev/null 2>&1
test ! -e /usr/bin/ghostlight
for directory in \
    /etc/opt/chrome/native-messaging-hosts \
    /etc/opt/edge/native-messaging-hosts \
    /etc/brave/native-messaging-hosts \
    /etc/chromium/native-messaging-hosts; do
    test ! -e "$directory/org.sylin.ghostlight.json"
done
test -s "$runtime"
echo 'result=PASS'
