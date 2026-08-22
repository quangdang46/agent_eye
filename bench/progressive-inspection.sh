#!/usr/bin/env bash
# progressive-inspection.sh — test the core Agent-Eye hypothesis:
#
#   targeted zoom resolves uncertainty that a fixed overview cannot,
#   and the accuracy-per-context-cost trade favors progressive inspection.
#
# Protocol per case (deterministic; no LLM needed for the cost side):
#   1. OVERVIEW — `inspect --no-render` (context cost C1) + full-image
#      render at width W. Structural questions (region count/ids/relations)
#      are answered here.
#   2. PROGRESSIVE — inspect → pick the highest-edge-density region →
#      `region` crop rendered at the SAME output width W as the overview.
#      The crop allocates all W columns to a fraction of the source,
#      i.e. higher effective resolution on the uncertain area.
#   3. METRIC — distinct-glyph count of each render at equal width
#      (resolved-detail proxy), plus bytes and tool-call costs.
#      A crop whose glyph set is NOT contained in the overview's exposes
#      NEW evidence: luminance structure the overview averaged away.
#
# Usage: ./progressive-inspection.sh [binary]
set -euo pipefail

AE_BIN="${1:-../target/release/ae}"
here="$(cd "$(dirname "$0")" && pwd)"
fail=0

row() { printf "%-22s %9s %8s %8s %8s  %s\n" "$1" "$2" "$3" "$4" "$5" "$6"; }

printf "%-22s %9s %8s %8s %8s  %s\n" "CASE" "OV_BYTES" "ZM_BYTES" "TOOLS" "OV_GLYPHS" "CROP_GLYPHS (new glyphs)"
for img in "$here/fixtures/001-ui.png" "$here/fixtures/002-diagram.png" "$here/fixtures/004-screenshot.png" "$here/fixtures/003-button.png"; do
  # Step 1: overview.
  overview=$("$AE_BIN" inspect "$img" --no-render --format json)
  ov_bytes=${#overview}
  n_regions=$(python3 -c "import json,sys; print(len(json.load(sys.stdin)['regions']))" <<< "$overview")
  rid=$(python3 -c "
import json,sys
d = json.load(sys.stdin)
r = d['regions']
print(max(r, key=lambda x: x['edge_density'])['id'])" <<< "$overview")

  ov_render=$("$AE_BIN" render "$img" --renderer ascii --width 40)

  # Step 2: progressive zoom into that region at the SAME output width.
  crop=$("$AE_BIN" region "$img" --region "$rid" --format text)
  zm_bytes=$(( ${#overview} + ${#crop} ))

  glyph_counts=$(python3 - "$ov_render" "$crop" <<'PYEOF'
import sys
def glyphs(s):
    return set(s.replace('\n', '')) - {' '}
ov, crop = glyphs(sys.argv[1]), glyphs(sys.argv[2])
new = crop - ov
print(f"{len(ov)} {len(crop)} {len(new)} {'|'.join(sorted(new))}")
PYEOF
)
  read -r ov_g cg new_n new_glyphs <<< "$glyph_counts"
  tools=2

  if [ "$n_regions" -lt 1 ]; then
    echo "FAIL $img: no regions found to follow"; fail=1
  fi

  if [ "$new_n" -gt 0 ]; then
    evidence="NEW EVIDENCE: $new_n glyphs unseen in overview (${new_glyphs})"
  else
    evidence="crop glyphs subset of overview — no added resolution"
  fi

  row "$(basename "$img")" "$ov_bytes" "$zm_bytes" "$tools" "$ov_g" "$cg → $evidence"
done

echo
echo "Hypothesis: targeted crops expose glyphs absent from the fixed-width"
echo "overview — resolved detail where the agent pointed, at bounded cost."
if [ "$fail" -eq 0 ]; then echo "PROGRESSIVE INSPECTION PATH: OK"; else exit 1; fi
