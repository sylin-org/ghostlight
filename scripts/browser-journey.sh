#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# A complete governed browser journey driven entirely by `ghostlight call`.

set -eu

url="https://example.com"
ghostlight_arg=""
output_path="${TMPDIR:-/tmp}/ghostlight-journey.jpg"

usage() {
  cat <<'EOF'
Usage: scripts/browser-journey.sh [options]

Options:
  --url URL              Page to open (default: https://example.com)
  --ghostlight PATH      Ghostlight executable
  --output PATH          Screenshot destination
  -h, --help             Show this help
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --url) [ "$#" -ge 2 ] || { echo "browser-journey: --url needs a value" >&2; exit 64; }; url=$2; shift 2 ;;
    --ghostlight) [ "$#" -ge 2 ] || { echo "browser-journey: --ghostlight needs a value" >&2; exit 64; }; ghostlight_arg=$2; shift 2 ;;
    --output) [ "$#" -ge 2 ] || { echo "browser-journey: --output needs a value" >&2; exit 64; }; output_path=$2; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "browser-journey: unknown option: $1" >&2; usage >&2; exit 64 ;;
  esac
done

command -v jq >/dev/null 2>&1 || {
  echo "browser-journey: jq is required to read Ghostlight's JSON results" >&2
  exit 69
}

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repository=$(CDPATH= cd "$script_dir/.." && pwd)

resolve_ghostlight() {
  if [ -n "$ghostlight_arg" ]; then
    [ -x "$ghostlight_arg" ] || {
      echo "browser-journey: Ghostlight is not executable: $ghostlight_arg" >&2
      exit 69
    }
    ghostlight_dir=$(CDPATH= cd "$(dirname "$ghostlight_arg")" && pwd)
    printf '%s/%s\n' "$ghostlight_dir" "$(basename "$ghostlight_arg")"
    return
  fi
  if command -v ghostlight >/dev/null 2>&1; then
    command -v ghostlight
    return
  fi
  for candidate in \
    "$repository/target/release/ghostlight" \
    "$repository/.target-ghostlight-1.0/debug/ghostlight" \
    "$repository/target/debug/ghostlight"
  do
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  echo "browser-journey: could not find Ghostlight; put it on PATH or pass --ghostlight" >&2
  exit 69
}

ghostlight=$(resolve_ghostlight)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/ghostlight-browser-journey.XXXXXX")
result="$temporary/result.json"
cleanup() {
  case "$temporary" in
    "${TMPDIR:-/tmp}"/ghostlight-browser-journey.*) rm -rf "$temporary" ;;
  esac
}
trap cleanup EXIT HUP INT TERM

worst=0
run_step() {
  label=$1
  tool=$2
  body=$3
  shift 3
  if "$ghostlight" call "$tool" "$body" --json "$@" >"$result"; then
    call_status=0
  else
    call_status=$?
  fi
  status=$(jq -er '.status' "$result") || {
    echo "browser-journey: $label returned no result status" >&2
    exit 70
  }
  summary=$(jq -er '.summary' "$result") || {
    echo "browser-journey: $label returned no summary" >&2
    exit 70
  }
  printf '%-12s %-10s %s\n' "$label" "$status" "$summary"
  if [ "$call_status" -ne 0 ]; then
    worst=$call_status
  fi
}

printf 'Ghostlight: %s\n\n' "$ghostlight"
printf '%-12s %-10s %s\n' STEP STATUS 'WHAT HAPPENED'
printf '%-12s %-10s %s\n' ---- ------ '-------------'

run_step open browser_navigate "$(jq -nc --arg url "$url" '{url:$url}')"
if [ "$(jq -r '.status' "$result")" != "succeeded" ]; then
  echo "browser-journey: could not open $url" >&2
  if [ "$worst" -ne 0 ]; then exit "$worst"; else exit 70; fi
fi
tab=$(jq -er '.facts.tab' "$result")
run_step list browser_tabs '{"action":"list"}'
run_step read browser_read "$(jq -nc --arg tab "$tab" '{tab:$tab}')"
run_step screenshot browser_screenshot "$(jq -nc --arg tab "$tab" '{tab:$tab}')" --output "$output_path"
run_step close browser_tabs "$(jq -nc --arg tab "$tab" '{action:"close",tab:$tab}')"

printf '\n'
if [ -f "$output_path" ]; then
  bytes=$(wc -c <"$output_path" | tr -d ' ')
  printf 'Screenshot: %s (%s bytes)\n' "$output_path" "$bytes"
fi

case "$worst" in
  0) echo 'Journey complete. Every step ran through ghostlight call, governed and audited as cli.' ;;
  2) echo 'Journey finished with a governed refusal. That is Ghostlight working, not failing.' ;;
  *) echo "Journey did not complete cleanly (exit $worst)." ;;
esac
exit "$worst"
