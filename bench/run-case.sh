#!/usr/bin/env bash
# run-case.sh — execute one benchmark case in one mode, capture metrics.
#
# Usage: ./run-case.sh <case-id> <mode> [binary]
#   case-id : id from cases.jsonl (e.g. 001)
#   mode    : baseline | ascii | blocks | ae-inspect | ae-progressive | ae-negative
#   binary  : path to `ae` binary (default: ../target/release/ae)
#
# Output: one JSON line on stdout matching the plan §12 capture format.
set -euo pipefail

case_id="$1"
mode="$2"
AE_BIN="${3:-../target/release/ae}"
here="$(cd "$(dirname "$0")" && pwd)"

case_line=$(grep -F "\"id\":\"$case_id\"" "$here/cases.jsonl") || {
  echo "unknown case $case_id" >&2; exit 2;
}
image=$(echo "$case_line" | sed -E 's/.*"image":"([^"]+)".*/\1/')
task=$(echo "$case_line" | sed -E 's/.*"task":"([^"]+)".*/\1/')
img_path="$here/fixtures/$image"

ae_calls="[]"
tool_calls=0
bytes_transferred=0
context_payload=""

capture() { # $1 = command label; reads bytes on stdout
  local n
  n=$(wc -c < /dev/stdin | tr -d ' ')
  bytes_transferred=$((bytes_transferred + n))
  tool_calls=$((tool_calls + 1))
  if [ "$ae_calls" = "[]" ]; then
    ae_calls="[\"$1\"]"
  else
    ae_calls="${ae_calls%]} ,\"$1\"]"
  fi
}

start=$(date +%s%N 2>/dev/null || python3 -c 'import time; print(int(time.time()*1e9))')

case "$mode" in
  baseline)
    context_payload="(no visual input — agent must answer from task text alone)"
    ;;
  ascii)
    payload=$("$AE_BIN" render "$img_path" --renderer ascii --width 80) || exit 3
    context_payload="$payload"
    echo "$payload" > /dev/null; capture render <<EOF2
$( "$AE_BIN" render "$img_path" --renderer ascii --width 80 )
EOF2
    ;;
  blocks)
    payload=$("$AE_BIN" render "$img_path" --renderer blocks --width 80) || exit 3
    context_payload="$payload"
    capture blocks <<< "$( "$AE_BIN" render "$img_path" --renderer blocks --width 80 )"
    ;;
  ae-inspect)
    payload_json=$("$AE_BIN" inspect "$img_path" --width 60 --no-render --format json) || exit 3
    payload_text=$("$AE_BIN" inspect "$img_path" --width 60 --format json) || exit 3
    context_payload="$payload_json"
    capture inspect <<< "$payload_json"
    capture inspect-text <<< "$payload_text"
    ;;
  ae-progressive)
    step1=$("$AE_BIN" inspect "$img_path" --width 40 --no-render --format json) || exit 3
    capture inspect <<< "$step1"
    # Follow the first detected region as a progressive zoom.
    rid=$(echo "$step1" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["regions"][0]["id"] if d["regions"] else "")')
    if [ -n "$rid" ]; then
      step2=$("$AE_BIN" region "$img_path" --region "$rid" --format text) || exit 3
      capture region <<< "$step2"
    fi
    context_payload="inspect+region($rid)"
    ;;
  ae-negative)
    # Negative control: prove what ae CANNOT do (OCR). Capture proves the
    # tool returns geometry only — no text content exists to extract.
    payload=$("$AE_BIN" geometry "$img_path" --format json) || exit 3
    context_payload="$payload"
    capture geometry <<< "$payload"
    ;;
  *)
    echo "unknown mode $mode" >&2; exit 2;;
esac

end=$(date +%s%N 2>/dev/null || python3 -c 'import time; print(int(time.time()*1e9))')
duration_ms=$(( (end - start) / 1000000 ))

python3 - "$case_id" "$mode" "$task" "$ae_calls" "$tool_calls" "$bytes_transferred" "$duration_ms" <<'PY'
import json, sys
case, mode, task, calls, tools, nbytes, ms = sys.argv[1:8]
out = {
    "case": case,
    "mode": mode,
    "answer": f"[{mode}] harness placeholder — LLM judge fills this in Phase 7 evaluation",
    "task": task,
    "ae_calls": json.loads(calls),
    "ae_tokens_used": int(nbytes) // 4,  # ~4 chars/token heuristic
    "tool_calls": int(tools),
    "bytes_transferred": int(nbytes),
    "duration_ms": int(ms),
}
print(json.dumps(out))
PY
