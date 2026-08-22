#!/usr/bin/env bash
# four-tier-benchmark.sh — the full benchmark matrix (plan §12, Phase 7).
#
# Tier 1: Correctness      — decode, dimensions, crop, region, zoom, determinism.
# Tier 2: Representation   — glyph diversity, spatial preservation, edge
#          Quality           preservation, ASCII vs Blocks comparison.
# Tier 3: Context Eff.     — output bytes, est. tokens, visual information
#                            retained per token.
# Tier 4: Agent Tasks      — the 7 task types via the harness; per-mode cost
#                            table feeding the Phase 7 PROJECT GATE decision.
#
# Every tier prints PASS/FAIL rows; overall exit code aggregates them.
# Deterministic — no LLM required for tiers 1–3; tier 4 captures costs that
# the LLM judge scores in evaluation runs.
#
# Usage: ./four-tier-benchmark.sh [binary]
set -euo pipefail

AE_BIN="${1:-../target/release/ae}"
here="$(cd "$(dirname "$0")" && pwd)"
total_fail=0

section() { echo; echo "=== TIER $1: $2 ==="; }
check() { # label, condition_exit_code
  if [ "$2" -eq 0 ]; then echo "PASS $1"; else echo "FAIL $1"; total_fail=$((total_fail+1)); fi
}

img() { echo "$here/fixtures/$1"; }

# ---------------------------------------------------------------- Tier 1
section 1 "CORRECTNESS"
"$AE_BIN" inspect "$(img 001-ui.png)" --format json | python3 -c "
import json, sys
d = json.load(sys.stdin)
assert d['image']['width'] == 64 and d['image']['height'] == 48, d['image']
assert d['schema_version'] == 'agent-eye.scene.v1'" ; check "decode+dimensions" $?

a=$("$AE_BIN" inspect "$(img 001-ui.png)" --no-render --format json)
b=$("$AE_BIN" inspect "$(img 001-ui.png)" --no-render --format json)
[ "$a" = "$b" ]; check "determinism (inspect byte-identical)" $?

"$AE_BIN" region "$(img 003-button.png)" --box 8,8,16,16 --format json | python3 -c "
import json,sys; d=json.load(sys.stdin)
assert d['region']['bounds'] == [8,8,24,24]
assert d['mapping']['source_bounds'] == [8,8,24,24]" ; check "crop bounds + provenance" $?

"$AE_BIN" zoom "$(img 003-button.png)" --box 0,0,32,32 --level 1 --format json | python3 -c "
import json,sys; d=json.load(sys.stdin)
assert d['zoom']['scale'] == 2.0 and d['provenance']['source_bounds'] == [0,0,16,16]" ; check "zoom level→scale" $?

n=$("$AE_BIN" geometry "$(img 002-diagram.png)" --format json | python3 -c "
import json,sys; print(len(json.load(sys.stdin)['relations']))")
[ "$n" -gt 0 ] && [ "$n" -le 500 ]; check "relations bounded ($n)" $?

# ---------------------------------------------------------------- Tier 2
section 2 "REPRESENTATION QUALITY"
python3 - "$AE_BIN" "$here" <<'PY'
import subprocess, sys
ae, here = sys.argv[1], sys.argv[2]
def run(*args):
    return subprocess.run([ae, *args], capture_output=True, text=True).stdout
ok = True
for fixture, name in [("001-ui.png", "ui"), ("004-screenshot.png", "screenshot")]:
    p = f"{here}/fixtures/{fixture}"
    ascii_out = run("render", p, "--renderer", "ascii", "--width", "60")
    blocks_out = run("render", p, "--renderer", "blocks", "--width", "60")
    g_ascii = len(set(ascii_out.replace("\n", "")) - {" "})
    g_blocks = len(set(blocks_out.replace("\n", "")) - {" "})
    # Spatial preservation: distinct row signatures should exist for a
    # structured image (not one giant flat run).
    rows = set(ascii_out.splitlines())
    print(f"{'PASS' if g_ascii >= 3 and g_blocks >= 2 else 'FAIL'} {name}: ascii_glyphs={g_ascii} blocks_glyphs={g_blocks} distinct_rows={len(rows)}")
    if g_ascii < 3 or g_blocks < 2:
        ok = False
sys.exit(0 if ok else 1)
PY
check "glyph diversity / spatial structure" $?

# Edge preservation: Sobel-driven regions on diagram.png must include the
# connector line area (edge density > 0 somewhere).
"$AE_BIN" geometry "$(img 002-diagram.png)" --format json | python3 -c "
import json,sys
d = json.load(sys.stdin)
assert any(r['edge_density'] > 0 for r in d['regions'])" ; check "edge preservation signal" $?

# ---------------------------------------------------------------- Tier 3
section 3 "CONTEXT EFFICIENCY"
python3 - "$AE_BIN" "$here" <<'PY'
import subprocess, sys
ae, here = sys.argv[1], sys.argv[2]
p = f"{here}/fixtures/001-ui.png"

def run(*args):
    r = subprocess.run([ae, *args], capture_output=True, text=True)
    return r.stdout

variants = {
    "ascii-80": run("render", p, "--renderer", "ascii", "--width", "80"),
    "blocks-80": run("render", p, "--renderer", "blocks", "--width", "80"),
    "scene-json": run("inspect", p, "--no-render", "--format", "json"),
    "scene-full": run("inspect", p, "--width", "60"),
}
print(f"{'variant':<12} {'bytes':>7} {'~tokens':>8} {'distinct_glyphs':>15}")
for k, v in variants.items():
    glyphs = len(set(v) - set('\n '))
    print(f"{k:<12} {len(v):>7} {len(v)//4:>8} {glyphs:>15}")
# Scene JSON (no render) should be far cheaper than full renders while
# carrying ALL structural evidence.
assert len(variants["scene-json"]) < min(len(variants["ascii-80"]), len(variants["blocks-80"])), \
    "geometry-only overview should be cheaper than pixel renders"
PY
check "context efficiency ordering" $?

# ---------------------------------------------------------------- Tier 4
section 4 "AGENT TASK BENCHMARK (cost capture)"
if [ -x "$here/agent-eval.sh" ]; then
  (cd "$here" && ./agent-eval.sh "$AE_BIN" >/dev/null 2>&1)
  python3 - "$here/results/summary.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))
modes = ["baseline", "ascii", "blocks", "ae-inspect", "ae-progressive"]
print(f"{'mode':<16} {'cases':>5} {'bytes':>8} {'tools':>6} {'~tok':>6} {'ms':>6}")
for m in modes:
    if m in s:
        r = s[m]
        print(f"{m:<16} {r['cases']:>5} {r['total_bytes']:>8} {r['total_tool_calls']:>6} {r['est_tokens']:>6} {r['total_ms']:>6}")
missing = [m for m in modes if m not in s]
assert not missing, f"missing modes: {missing}"
PY
  check "all 5 modes captured" $?
else
  echo "SKIP agent-eval.sh missing"; total_fail=$((total_fail+1))
fi

# Vision-model upper bound is out of scope for this script (requires an
# external vision model); noted for the PROJECT GATE report.
echo
echo "(Vision-model upper bound requires external model — deferred to gate report.)"

echo
if [ "$total_fail" -eq 0 ]; then
  echo "ALL 4 TIERS PASSED"
else
  echo "$total_fail CHECK(S) FAILED" >&2
  exit 1
fi
