#!/bin/bash
set -euo pipefail
test "${GHOSTLIGHT_LINUX_DISPOSABLE_GUEST:-}" = 1 || {
 echo 'Run upgrade.sh through acceptance/guest/consumer.sh in a disposable guest.' >&2; exit 2;
}
scripts=$(cd "$(dirname "$0")" && pwd)
scenario=${1:?fresh scenario id}
case "$scenario" in *[!a-zA-Z0-9-]*) exit 2;; esac
test_home=/home/ghostlight-upgrade-$scenario
test_signal=/tmp/ghostlight-upgrade-$scenario
old=${GHOSTLIGHT_LINUX_PREVIOUS_DEB:?}
new=${GHOSTLIGHT_LINUX_DEB:?}
DEBIAN_FRONTEND=noninteractive apt-get install -y --allow-downgrades "$old"
rm -f $test_signal/{ready,upgraded,results.json}
install -d -m 0700 -o 1000 -g 1000 $test_home $test_home/run $test_signal
printf '%s\n' 'foreign native registration fixture' >/etc/opt/chrome/native-messaging-hosts/org.example.foreign.json
printf '%s\n' '{"mcpServers":{"foreign":{"command":"/bin/true"}}}' >$test_home/.claude.json
chown 1000:1000 $test_home/.claude.json
before=$(sha256sum $test_home/.claude.json /etc/opt/chrome/native-messaging-hosts/org.example.foreign.json)
setpriv --reuid=1000 --regid=1000 --clear-groups env HOME=$test_home XDG_RUNTIME_DIR=$test_home/run GHOSTLIGHT_UPGRADE_FIXTURE=$test_signal xvfb-run -a timeout 120s python3 "$scripts/upgrade-user.py" > $test_signal/user.log 2>&1 &
runner=$!
trap 'kill "$runner" 2>/dev/null || true; pkill -f "^/usr/bin/ghostlight$" || true' EXIT
for attempt in $(seq 1 300); do
 test -e $test_signal/ready && break
 kill -0 "$runner"; sleep .1
done
test -e $test_signal/ready
DEBIAN_FRONTEND=noninteractive apt-get install -y "$new"
test "$(ghostlight --version)" = "ghostlight $GHOSTLIGHT_LINUX_VERSION"
test "$before" = "$(sha256sum $test_home/.claude.json /etc/opt/chrome/native-messaging-hosts/org.example.foreign.json)"
touch $test_signal/upgraded
wait "$runner"
cat $test_signal/results.json
echo result=PASS
