#!/bin/sh
# SPDX-License-Identifier: Apache-2.0 OR MIT
# The Sylin Card Foundry story, driven entirely by `ghostlight call`.

set -eu

url="https://sylin.org/ghostlight/demo/foundry/"
ghostlight_arg=""
beat="0.35"
width="1280"
height="800"
keep_recording=0
rejection="Foil registration drifts past the lower-right safe area. Hold for Revision B."
off_domain="https://example.com/"

usage() {
  cat <<'EOF'
Usage: scripts/demo-foundry.sh [options]

Options:
  --url URL              Demo stage
  --ghostlight PATH      Ghostlight executable
  --beat SECONDS         Hold between actions (default: 0.35)
  --width PIXELS         Browser width (default: 1280)
  --height PIXELS        Browser height (default: 800)
  --keep-recording       Leave the recording in memory until it expires
  -h, --help             Show this help
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --url) [ "$#" -ge 2 ] || { echo "demo-foundry: --url needs a value" >&2; exit 64; }; url=$2; shift 2 ;;
    --ghostlight) [ "$#" -ge 2 ] || { echo "demo-foundry: --ghostlight needs a value" >&2; exit 64; }; ghostlight_arg=$2; shift 2 ;;
    --beat) [ "$#" -ge 2 ] || { echo "demo-foundry: --beat needs a value" >&2; exit 64; }; beat=$2; shift 2 ;;
    --width) [ "$#" -ge 2 ] || { echo "demo-foundry: --width needs a value" >&2; exit 64; }; width=$2; shift 2 ;;
    --height) [ "$#" -ge 2 ] || { echo "demo-foundry: --height needs a value" >&2; exit 64; }; height=$2; shift 2 ;;
    --keep-recording) keep_recording=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "demo-foundry: unknown option: $1" >&2; usage >&2; exit 64 ;;
  esac
done

command -v jq >/dev/null 2>&1 || {
  echo "demo-foundry: jq is required to build and read Ghostlight JSON" >&2
  exit 69
}
jq -en --arg value "$beat" '($value | tonumber) >= 0' >/dev/null 2>&1 || {
  echo "demo-foundry: --beat must be a non-negative number" >&2
  exit 64
}
jq -en --arg width "$width" --arg height "$height" \
  '($width | test("^[0-9]+$") and (tonumber >= 320) and (tonumber <= 7680)) and
   ($height | test("^[0-9]+$") and (tonumber >= 240) and (tonumber <= 4320))' \
  >/dev/null 2>&1 || {
    echo "demo-foundry: width must be 320..7680 and height must be 240..4320" >&2
    exit 64
  }

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repository=$(CDPATH= cd "$script_dir/.." && pwd)
resolve_ghostlight() {
  if [ -n "$ghostlight_arg" ]; then
    [ -x "$ghostlight_arg" ] || {
      echo "demo-foundry: Ghostlight is not executable: $ghostlight_arg" >&2
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
  echo "demo-foundry: could not find Ghostlight; put it on PATH or pass --ghostlight" >&2
  exit 69
}

ghostlight=$(resolve_ghostlight)
temporary=$(mktemp -d "${TMPDIR:-/tmp}/ghostlight-demo-foundry.XXXXXX")
result="$temporary/result.json"
lookup="$temporary/lookup.json"
shot="$temporary/revision-b.jpg"
cleanup() {
  case "$temporary" in
    "${TMPDIR:-/tmp}"/ghostlight-demo-foundry.*) rm -rf "$temporary" ;;
  esac
}
trap cleanup EXIT HUP INT TERM

step() {
  label=$1
  tool=$2
  body=$3
  expected=$4
  shift 4
  if "$ghostlight" call "$tool" "$body" --json "$@" >"$result"; then
    call_status=0
  else
    call_status=$?
  fi
  status=$(jq -er '.status' "$result") || {
    echo "demo-foundry: $label returned no result status" >&2
    exit 70
  }
  summary=$(jq -er '.summary' "$result") || {
    echo "demo-foundry: $label returned no summary" >&2
    exit 70
  }
  printf '%-16s %-10s %s\n' "$label" "$status" "$summary"
  if [ "$status" != "$expected" ]; then
    echo "demo-foundry: $label expected $expected but was $status" >&2
    if [ "$call_status" -ne 0 ]; then exit "$call_status"; else exit 70; fi
  fi
  sleep "$beat"
}

target() {
  role=$1
  prefix=$2
  jq -er --arg role "$role" --arg prefix "$prefix" \
    'first(.facts.items[] | select(.role == $role and (.name | startswith($prefix)))) | .target' \
    "$result" || {
      echo "demo-foundry: the stage exposes no $role named '$prefix'" >&2
      exit 70
    }
}

find_target() {
  tab_id=$1
  text=$2
  role=$3
  body=$(jq -nc --arg tab "$tab_id" --arg text "$text" '{tab:$tab,text:$text}')
  if "$ghostlight" call browser_find "$body" --json >"$lookup"; then
    :
  else
    find_status=$?
    echo "demo-foundry: browser_find failed while looking for '$text'" >&2
    exit "$find_status"
  fi
  found_target=$(jq -er --arg role "$role" 'first(.facts.matches[] | select(.role == $role)) | .target' "$lookup") || {
    echo "demo-foundry: the stage exposes no $role matching '$text'" >&2
    exit 70
  }
}

printf 'Ghostlight: %s\n' "$ghostlight"
printf 'Stage:      %s\n\n' "$url"
printf '%-16s %-10s %s\n' BEAT STATUS 'WHAT HAPPENED'
printf '%-16s %-10s %s\n' ---- ------ '-------------'

step open browser_navigate "$(jq -nc --arg url "$url" '{url:\}')" succeeded
tab=$(jq -er '.facts.tab' "$result")
step frame browser_window "$(jq -nc --arg tab "$tab" --argjson width "$width" --argjson height "$height" '{tab:$tab,action:"resize",width:$width,height:$height}')" succeeded
step 'record start' browser_record "$(jq -nc --arg tab "$tab" '{action:"start",tab:$tab}')" succeeded
step inspect browser_inspect "$(jq -nc --arg tab "$tab" '{tab:$tab,scope:"controls",max_items:200}')" succeeded

rotate=$(target button 'Rotate foil proof')
drift=$(target checkbox 'Foil registration drift')
safe_area=$(target checkbox 'Border safe-area collision')
reason=$(target textbox 'Rejection reason')
ticket=$(target button 'Drag QA-017 defect ticket')
step 'hover foil' browser_hover "$(jq -nc --arg tab "$tab" --arg target "$rotate" '{tab:$tab,target:$target}')" succeeded
step 'rotate card' browser_click "$(jq -nc --arg tab "$tab" --arg target "$rotate" '{tab:$tab,target:$target}')" succeeded
step 'zoom defect' browser_window "$(jq -nc --arg tab "$tab" '{tab:$tab,action:"zoom",percent:150}')" succeeded
step 'zoom back' browser_window "$(jq -nc --arg tab "$tab" '{tab:$tab,action:"zoom",percent:100}')" succeeded
step 'qa drift' browser_click "$(jq -nc --arg tab "$tab" --arg target "$drift" '{tab:$tab,target:$target}')" succeeded
step 'qa safe-area' browser_click "$(jq -nc --arg tab "$tab" --arg target "$safe_area" '{tab:$tab,target:$target}')" succeeded
step reason browser_type_text "$(jq -nc --arg tab "$tab" --arg target "$reason" --arg text "$rejection" '{tab:$tab,target:$target,text:$text}')" succeeded
find_target "$tab" 'Request revision' span
revision=$found_target
step 'drag ticket' browser_drag "$(jq -nc --arg tab "$tab" --arg source "$ticket" --arg destination "$revision" '{tab:$tab,source_target:$source,destination_target:$destination}')" succeeded
step diagnose browser_diagnose "$(jq -nc --arg tab "$tab" '{tab:$tab,source:"both",detail:"all",limit:20}')" succeeded
step 'await rev B' browser_wait "$(jq -nc --arg tab "$tab" '{tab:$tab,condition:"text_present",value:"Revision B ready"}')" succeeded

step capture browser_screenshot "$(jq -nc --arg tab "$tab" '{tab:$tab}')" succeeded --output "$shot"
[ -f "$shot" ] || { echo "demo-foundry: no screenshot was written to $shot" >&2; exit 70; }
step 're-inspect' browser_inspect "$(jq -nc --arg tab "$tab" '{tab:$tab,scope:"controls",max_items:200}')" succeeded

evidence=$(target textbox 'Revision B screenshot evidence')
foil_verified=$(target checkbox 'Foil registration verified')
stamp_verified=$(target checkbox 'Sylin back stamp verified')
visual_attached=$(target checkbox 'Visual evidence attached')
release_name=$(target textbox 'Release name')
set_code=$(target textbox 'Set code')
release_owner=$(target textbox 'Release owner')
qa_note=$(target textbox 'QA note')
complete=$(target button 'Complete release packet')
replay=$(target textbox 'Animated Ghostlight replay')

step 'attach proof' browser_upload "$(jq -nc --arg tab "$tab" --arg target "$evidence" --arg path "$shot" '{tab:$tab,target:$target,paths:[$path]}')" succeeded
step 'qa foil' browser_click "$(jq -nc --arg tab "$tab" --arg target "$foil_verified" '{tab:$tab,target:$target}')" succeeded
step 'qa sylin' browser_click "$(jq -nc --arg tab "$tab" --arg target "$stamp_verified" '{tab:$tab,target:$target}')" succeeded
step 'qa visual' browser_click "$(jq -nc --arg tab "$tab" --arg target "$visual_attached" '{tab:$tab,target:$target}')" succeeded
step 'release packet' browser_fill_form "$(jq -nc \
  --arg tab "$tab" \
  --arg release_name "$release_name" \
  --arg set_code "$set_code" \
  --arg release_owner "$release_owner" \
  --arg qa_note "$qa_note" \
  '{tab:$tab,fields:[
    {target:$release_name,value:"Aurora Drop 01"},
    {target:$set_code,value:"AUR-01"},
    {target:$release_owner,value:"Maya Chen"},
    {target:$qa_note,value:"Revision B clears the foil mask and the Sylin back stamp."}
  ]}')" succeeded
# Completion replaces the packet view, so this is the keyboard beat's only honest moment.
step 'key to end' browser_press_key "$(jq -nc --arg tab "$tab" --arg target "$release_name" '{tab:$tab,target:$target,key:"End"}')" succeeded
step complete browser_click "$(jq -nc --arg tab "$tab" --arg target "$complete" '{tab:$tab,target:$target}')" succeeded
step off-domain browser_navigate "$(jq -nc --arg tab "$tab" --arg url "$off_domain" '{tab:$tab,url:$url,restrict_hosts:["sylin.org"]}')" blocked
step 'save replay' browser_record "$(jq -nc --arg target "$replay" '{action:"save",target:$target}')" succeeded
step 'replay landed' browser_wait "$(jq -nc --arg tab "$tab" '{tab:$tab,condition:"text_present",value:"Replay ready"}')" succeeded

if [ "$keep_recording" -eq 0 ]; then
  step 'erase bytes' browser_record '{"action":"discard"}' succeeded
  outcome='Story complete: inspected, rejected, revised, evidenced, refused off-domain, replayed, erased.'
else
  outcome='Story complete: inspected, rejected, revised, evidenced, refused off-domain, and replayed. Recording retained until expiry.'
fi

# Whole-catalog coda. The story above is the narrative; this is the rehearsal, so one script
# exercises every tool in the catalog. The dialog beats ring the desk stage's bell, whose
# prompt() gives browser_dialog something honest to status, answer, and dismiss.
step 'scroll stage' browser_scroll "$(jq -nc --arg tab "$tab" '{tab:$tab,direction:"down",amount:"page"}')" succeeded
step 'scroll back' browser_scroll "$(jq -nc --arg tab "$tab" '{tab:$tab,direction:"up",amount:"medium"}')" succeeded
step 'read title' browser_execute "$(jq -nc --arg tab "$tab" '{tab:$tab,script:"document.title"}')" succeeded
step 'seq scroll-wait' browser_sequence "$(jq -nc --arg tab "$tab" '{tab:$tab,steps:[{action:"scroll",direction:"down",amount:"small"},{action:"wait",condition:"text_present",value:"SYLIN"}]}')" succeeded
step 'flow title-find' browser_flow "$(jq -nc --arg tab "$tab" '{tab:$tab,steps:[{id:"title",tool:"browser_execute",arguments:{script:"document.title"}},{id:"find it",tool:"browser_find",arguments:{text:{flow_ref:{step:"title",pointer:"/facts/value"}}}}]}')" succeeded
# The 24th catalog tool belongs in the story, or "whole catalog rehearsed" stops being true
# (CachyOS finding 3, 2026-08-25).
step 'explain policy' policy_explain '{}' succeeded
step 'demo index' browser_navigate "$(jq -nc --arg tab "$tab" --arg index 'https://sylin.org/ghostlight/demo/' '{tab:$tab,url:$index}')" succeeded
step 'history back' browser_history "$(jq -nc --arg tab "$tab" '{tab:$tab,action:"back"}')" succeeded

step 'desk stage' browser_navigate "$(jq -nc --arg tab "$tab" --arg desk 'https://sylin.org/ghostlight/demo/desk/' '{tab:$tab,url:$desk}')" succeeded
find_target "$tab" 'Ring the bell' button
bell=$found_target
step 'ring once' browser_click "$(jq -nc --arg tab "$tab" --arg target "$bell" '{tab:$tab,target:$target}')" succeeded
step 'dialog status' browser_dialog "$(jq -nc --arg tab "$tab" '{tab:$tab,action:"status"}')" succeeded
step 'dialog answer' browser_dialog "$(jq -nc --arg tab "$tab" '{tab:$tab,action:"respond",text:"Ghostlight was here"}')" succeeded
step 'bell answered' browser_wait "$(jq -nc --arg tab "$tab" '{tab:$tab,condition:"text_present",value:"the bell says"}')" succeeded
step 'ring again' browser_click "$(jq -nc --arg tab "$tab" --arg target "$bell" '{tab:$tab,target:$target}')" succeeded
step 'dialog dismiss' browser_dialog "$(jq -nc --arg tab "$tab" '{tab:$tab,action:"dismiss"}')" succeeded
step 'bell silent' browser_wait "$(jq -nc --arg tab "$tab" '{tab:$tab,condition:"text_present",value:"dismissed without an answer"}')" succeeded

printf '\n%s\n' "$outcome"
echo 'Whole catalog rehearsed, ending with the desk bell answering and dismissing.'
echo 'The tab stays open for your capture; close it when you are done.'
