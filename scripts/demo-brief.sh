#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# The launch-brief demo story, driven entirely by `ghostlight call`.

set -eu

url="https://sylin.org/ghostlight/demo/brief/"
ghostlight_arg=""
setup_hold="2.0"
scan_hold="1.6"
beat="0.25"
completion_hold="3.0"
project="Moonlight Notes"
owner="Maya Chen"
brief_summary="Turn field observations into a shared release brief."
completion="$project is ready for review."

usage() {
  cat <<'EOF'
Usage: scripts/demo-brief.sh [options]

Options:
  --url URL                    Demo stage
  --ghostlight PATH            Ghostlight executable
  --setup-hold SECONDS         Hold after opening (default: 2.0)
  --scan-hold SECONDS          Hold after reading (default: 1.6)
  --beat SECONDS               Hold between actions (default: 0.25)
  --completion-hold SECONDS    Final hold (default: 3.0)
  -h, --help                   Show this help
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --url) [ "$#" -ge 2 ] || { echo "demo-brief: --url needs a value" >&2; exit 64; }; url=$2; shift 2 ;;
    --ghostlight) [ "$#" -ge 2 ] || { echo "demo-brief: --ghostlight needs a value" >&2; exit 64; }; ghostlight_arg=$2; shift 2 ;;
    --setup-hold) [ "$#" -ge 2 ] || { echo "demo-brief: --setup-hold needs a value" >&2; exit 64; }; setup_hold=$2; shift 2 ;;
    --scan-hold) [ "$#" -ge 2 ] || { echo "demo-brief: --scan-hold needs a value" >&2; exit 64; }; scan_hold=$2; shift 2 ;;
    --beat) [ "$#" -ge 2 ] || { echo "demo-brief: --beat needs a value" >&2; exit 64; }; beat=$2; shift 2 ;;
    --completion-hold) [ "$#" -ge 2 ] || { echo "demo-brief: --completion-hold needs a value" >&2; exit 64; }; completion_hold=$2; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "demo-brief: unknown option: $1" >&2; usage >&2; exit 64 ;;
  esac
done

command -v jq >/dev/null 2>&1 || {
  echo "demo-brief: jq is required to build and read Ghostlight JSON" >&2
  exit 69
}
for delay in "$setup_hold" "$scan_hold" "$beat" "$completion_hold"; do
  jq -en --arg value "$delay" '($value | tonumber) >= 0' >/dev/null 2>&1 || {
    echo "demo-brief: hold values must be non-negative numbers" >&2
    exit 64
  }
done

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repository=$(CDPATH= cd "$script_dir/.." && pwd)
resolve_ghostlight() {
  if [ -n "$ghostlight_arg" ]; then
    [ -x "$ghostlight_arg" ] || {
      echo "demo-brief: Ghostlight is not executable: $ghostlight_arg" >&2
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
  echo "demo-brief: could not find Ghostlight; put it on PATH or pass --ghostlight" >&2
  exit 69
}

ghostlight=$(resolve_ghostlight)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/ghostlight-demo-brief.XXXXXX")
result="$temporary/result.json"
cleanup() {
  case "$temporary" in
    "${TMPDIR:-/tmp}"/ghostlight-demo-brief.*) rm -rf "$temporary" ;;
  esac
}
trap cleanup EXIT HUP INT TERM

step() {
  label=$1
  tool=$2
  body=$3
  if "$ghostlight" call "$tool" "$body" --json >"$result"; then
    call_status=0
  else
    call_status=$?
  fi
  status=$(jq -er '.status' "$result") || {
    echo "demo-brief: $label returned no result status" >&2
    exit 70
  }
  summary=$(jq -er '.summary' "$result") || {
    echo "demo-brief: $label returned no summary" >&2
    exit 70
  }
  printf '%-14s %-10s %s\n' "$label" "$status" "$summary"
  if [ "$status" != "succeeded" ]; then
    echo "demo-brief: $label did not succeed" >&2
    if [ "$call_status" -ne 0 ]; then exit "$call_status"; else exit 70; fi
  fi
}

target() {
  role=$1
  prefix=$2
  jq -er --arg role "$role" --arg prefix "$prefix" \
    'first(.facts.items[] | select(.role == $role and (.name | startswith($prefix)))) | .target' \
    "$result" || {
      echo "demo-brief: the stage exposes no $role named '$prefix'" >&2
      exit 70
    }
}

printf 'Ghostlight: %s\n' "$ghostlight"
printf 'Stage:      %s\n\n' "$url"
printf '%-14s %-10s %s\n' STEP STATUS 'WHAT HAPPENED'
printf '%-14s %-10s %s\n' ---- ------ '-------------'

step open browser_navigate "$(jq -nc --arg url "$url" '{url:$url}')"
tab=$(jq -er '.facts.tab' "$result")
sleep "$setup_hold"
step scan browser_read "$(jq -nc --arg tab "$tab" '{tab:$tab}')"
sleep "$scan_hold"
step inventory browser_inspect "$(jq -nc --arg tab "$tab" '{tab:$tab,scope:"controls"}')"

project_target=$(target textbox Project)
owner_target=$(target textbox Owner)
summary_target=$(target textbox Summary)
screenshots_target=$(target checkbox 'Include screenshots')
local_target=$(target checkbox 'Keep data local')
submit_target=$(target button 'Create brief')

step 'field project' browser_type_text "$(jq -nc --arg tab "$tab" --arg target "$project_target" --arg text "$project" '{tab:$tab,target:$target,text:$text}')"
sleep "$beat"
step 'field owner' browser_type_text "$(jq -nc --arg tab "$tab" --arg target "$owner_target" --arg text "$owner" '{tab:$tab,target:$target,text:$text}')"
sleep "$beat"
step 'field summary' browser_type_text "$(jq -nc --arg tab "$tab" --arg target "$summary_target" --arg text "$brief_summary" '{tab:$tab,target:$target,text:$text}')"
sleep "$beat"
step 'tick shots' browser_click "$(jq -nc --arg tab "$tab" --arg target "$screenshots_target" '{tab:$tab,target:$target}')"
sleep "$beat"
step 'tick local' browser_click "$(jq -nc --arg tab "$tab" --arg target "$local_target" '{tab:$tab,target:$target}')"
sleep "$beat"
step submit browser_click "$(jq -nc --arg tab "$tab" --arg target "$submit_target" '{tab:$tab,target:$target}')"
step completion browser_wait "$(jq -nc --arg tab "$tab" --arg value "$completion" '{tab:$tab,condition:"text_present",value:$value}')"
sleep "$completion_hold"

printf '\nStory complete. The tab stays open for your capture; close it when you are done.\n'
