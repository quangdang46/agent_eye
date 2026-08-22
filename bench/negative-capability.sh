#!/usr/bin/env bash
# negative-capability.sh — verify ae's outputs structurally CANNOT answer
# the negative cases, i.e. that honest "I cannot determine" is the only
# correct agent behavior. This pins down calibrated uncertainty: if any of
# these probes DID return text/semantic content, the tool would be leaking
# fabricated evidence and this check fails.
#
# Usage: ./negative-capability.sh [binary]
set -euo pipefail

AE_BIN="${1:-../target/release/ae}"
here="$(cd "$(dirname "$0")" && pwd)"
fail=0

probe() { # name, expected_absent_regex_list..., command...
  local name="$1"; shift
  local out
  out=$("$@") || { echo "FAIL $name: command failed"; fail=1; return; }
  for pattern in "$@"; do :; done
  # Check every --absent pattern against the output.
  echo "$out"
}

check_absent() {
  local label="$1" output="$2"; shift 2
  for pat in "$@"; do
    if grep -qiE "$pat" <<< "$output"; then
      echo "FAIL $label: output contains '$pat' — evidence leak (hallucinated capability)"
      fail=1
    fi
  done
  echo "ok   $label: no forbidden content"
}

# n001: tiny button text — render + inspect must contain NO readable words.
out=$(probe n001 "$AE_BIN" render "$here/fixtures/003-button.png" --width 80)
check_absent "n001-render-text"    "$out" '[Aa]pple|[Bb]utton text|OK|Cancel|Submit|Click'
out=$(probe n001i "$AE_BIN" inspect "$here/fixtures/003-button.png" --format json)
check_absent "n001-inspect-text"   "$out" '"label"|description|ocr|text_content'

# n002: application identity — geometry JSON has no semantic fields.
out=$(probe n002 "$AE_BIN" geometry "$here/fixtures/004-screenshot.png" --format json)
check_absent "n002-app-identity"   "$out" 'application|app_name|semantic|class|confidence'

# n003: client-server intent — relations are geometric only.
out=$(probe n003 "$AE_BIN" geometry "$here/fixtures/002-diagram.png" --format json)
check_absent "n003-intent"         "$out" 'client|server|connection_meaning|intends'

# n004: exact hex color — region.v1 carries quantized metrics only.
out=$(probe n004 "$AE_BIN" region "$here/fixtures/003-button.png" --box 8,8,16,16 --format json)
check_absent "n004-exact-color"    "$out" '#[0-9a-fA-F]{6}|"rgb"|hex_color'

# Schema-wide guard: no prohibited fields anywhere in v1 outputs.
for cmd in "inspect $here/fixtures/001-ui.png" \
           "geometry $here/fixtures/001-ui.png" \
           "region $here/fixtures/001-ui.png --box 0,0,10,10"; do
  out=$(probe schema "$AE_BIN" $cmd --format json)
  check_absent "schema($cmd)" "$out" \
    '"label"|"confidence"|"importance"|"object"|"description"'
done

if [ "$fail" -eq 0 ]; then
  echo "ALL NEGATIVE CAPABILITY CHECKS PASSED — uncertainty is structural"
else
  echo "NEGATIVE CAPABILITY VIOLATIONS FOUND" >&2
  exit 1
fi
