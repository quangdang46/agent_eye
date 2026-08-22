#!/usr/bin/env bash
# agent-eval.sh — run the full case matrix across all modes, write results.
#
# Usage: ./agent-eval.sh [binary]
# Writes: results/<mode>.jsonl (one line per case) and results/summary.json
set -euo pipefail

AE_BIN="${1:-../target/release/ae}"
here="$(cd "$(dirname "$0")" && pwd)"
modes=(baseline ascii blocks ae-inspect ae-progressive ae-negative)
results_dir="$here/results"
mkdir -p "$results_dir"

# Collect case ids from cases.jsonl.
ids=$(python3 - "$here/cases.jsonl" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    line = line.strip()
    if line:
        print(json.loads(line)["id"])
PY
)

for mode in "${modes[@]}"; do
  out="$results_dir/$mode.jsonl"
  : > "$out"
  for id in $ids; do
    if "$here/run-case.sh" "$id" "$mode" "$AE_BIN" >> "$out"; then
      echo "  ok  $mode/$id"
    else
      echo "  ERR $mode/$id" >&2
    fi
  done
done

# Summary: cost table per mode — the VID numerator inputs.
python3 - "$results_dir" <<'PY'
import json, glob, os, sys
rd = sys.argv[1]
summary = {}
for path in sorted(glob.glob(os.path.join(rd, "*.jsonl"))):
    mode = os.path.basename(path)[:-6]
    rows = [json.loads(l) for l in open(path) if l.strip()]
    if not rows:
        continue
    summary[mode] = {
        "cases": len(rows),
        "total_bytes": sum(r["bytes_transferred"] for r in rows),
        "total_tool_calls": sum(r["tool_calls"] for r in rows),
        "est_tokens": sum(r["ae_tokens_used"] for r in rows),
        "total_ms": sum(r["duration_ms"] for r in rows),
    }
with open(os.path.join(rd, "summary.json"), "w") as f:
    json.dump(summary, f, indent=2, sort_keys=True)
print(json.dumps(summary, indent=2, sort_keys=True))
PY
echo "results written to $results_dir/"
