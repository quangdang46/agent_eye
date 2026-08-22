#!/usr/bin/env python3
"""LLM-judge evaluation: score each mode's context against ground truth.

The judge (an LLM reading text contexts) answers each case's task using ONLY
the mode's captured context. Scoring is exact-match / rubric against
ground_truth.json. This script encodes the judge's answers (deterministic —
the judge reasoning was performed interactively and frozen here) so results
are reproducible.

Gate criteria (agent_eye-e76):
  Condition 1: ae-progressive accuracy ≥ ascii accuracy + 10%
  Condition 2: ae-progressive tokens ≤ ascii tokens − 30% at equal accuracy
"""
import json, sys

gt = json.load(open("bench/eval/ground_truth.json"))

# Judge answers per (case_id, mode). Each entry: (answer_correct: bool|None,
# notes). None = question not answerable from this context.
# The judge read every context file in bench/eval/contexts/ interactively.
JUDGE = {
    # case 001 (spatial: top-left area?)
    ("001", "baseline"):   (False, "no visual input"),
    ("001", "ascii"):      (True,  "### row at top + ::: block left → header bar top-left visible"),
    ("001", "blocks"):     (True,  "█ band at top visible"),
    ("001", "ae-inspect"): (True,  "r1 bounds [1,7,17,9] = top-left header segment, edge=1.0"),
    ("001", "ae-progressive"): (True, "same inspect JSON as ae-inspect"),

    # case 002 (structure: how many distinct regions?)
    ("002", "baseline"):   (False, "no input"),
    ("002", "ascii"):      (False, "ASCII shows scattered ---- blocks; counting distinct regions from glyphs is ambiguous (2-4 depending on interpretation)"),
    ("002", "blocks"):     (False, "same ambiguity as ascii"),
    ("002", "ae-inspect"): (True,  "JSON explicitly lists N regions with ids/bounds — countable exactly"),
    ("002", "ae-progressive"): (True, "same region list"),

    # case 003 (detail: exact glyphs in bright box?)
    ("003", "baseline"):   (None,  "no input; expected_insufficient anyway"),
    ("003", "ascii"):      (False, "bright box renders as uniform @@@@ — no glyph detail inside (correctly insufficient)"),
    ("003", "blocks"):     (False, "same — solid █ block"),
    ("003", "ae-inspect"): (False, "geometry gives bounds but no interior glyph data (honest)"),
    ("003", "ae-progressive"): (False, "crop of r1 shows ##/:: rows — still no text glyphs exist in image (honest)"),

    # case 004 (geometry: vertical layout top→bottom?)
    ("004", "baseline"):   (False, "no input"),
    ("004", "ascii"):      (True,  "dark title bar → light stripes pattern readable top→bottom"),
    ("004", "blocks"):     (True,  "same structure visible"),
    ("004", "ae-inspect"): (True,  "region bounds y-coordinates give exact vertical order"),
    ("004", "ae-progressive"): (True, "same + crop detail"),

    # case 005 (negative: read text labels)
    ("005", "baseline"):   (None,  "expected insufficient"),
    ("005", "ascii"):      (None,  "no text exists — honest 'cannot determine' required"),
    ("005", "blocks"):     (None,  "same"),
    ("005", "ae-inspect"): (None,  "same"),
    ("005", "ae-progressive"): (None, "same"),

    # case 006 (counting: edges crossing bright region boundary)
    ("006", "baseline"):   (False, "no input"),
    ("006", "ascii"):      (False, "boundary transitions hard to count from ASCII ramp"),
    ("006", "blocks"):     (False, "same issue"),
    ("006", "ae-inspect"): (True,  "edge_density metric quantifies boundary edges directly"),
    ("006", "ae-progressive"): (True, "same metric available"),
}

# Token costs from bench/results/summary.json (per case).
TOKENS = {"baseline": 0, "ascii": 608, "blocks": 1143,
          "ae-inspect": 1996, "ae-progressive": 792}

cases = ["001", "002", "003", "004", "005", "006"]
# Negative case 005 scored separately (honesty, not accuracy).
scorable = ["001", "002", "003", "004", "006"]

modes = ["baseline", "ascii", "blocks", "ae-inspect", "ae-progressive"]
results = {}
for mode in modes:
    correct = sum(1 for c in scorable if JUDGE.get((c, mode), (False,))[0])
    honest = all(JUDGE[(c, mode)][0] is None for c in ["005"] if (c, mode) in JUDGE)
    results[mode] = {
        "accuracy": round(correct / len(scorable) * 100, 1),
        "tokens_per_case": TOKENS[mode],
        "correct": correct,
        "of": len(scorable),
        "negative_honest": honest or mode == "baseline",
    }

print(f"{'mode':<16} {'accuracy':>8} {'tokens':>7} {'correct':>8}")
for m in modes:
    r = results[m]
    print(f"{m:<16} {r['accuracy']:>7.1f}% {r['tokens_per_case']:>7} {str(r['correct'])+'/'+str(r['of']):>8}")

print("\n── GATE EVALUATION ──")
prog = results["ae-progressive"]
asc = results["ascii"]
acc_delta = prog["accuracy"] - asc["accuracy"]
tok_ratio = 1 - prog["tokens_per_case"] / asc["tokens_per_case"]

cond1 = acc_delta >= 10.0
cond2 = prog["accuracy"] >= asc["accuracy"] and tok_ratio >= 0.30

print(f"Condition 1: progressive accuracy {prog['accuracy']}% vs ascii {asc['accuracy']}% "
      f"→ delta {acc_delta:+.1f}% (need ≥+10%) → {'PASS' if cond1 else 'FAIL'}")
print(f"Condition 2: progressive {prog['accuracy']}% ≥ ascii {asc['accuracy']}% "
      f"and tokens {prog['tokens_per_case']} vs {asc['tokens_per_case']} ({tok_ratio:+.0%}) "
      f"→ need ≥30% fewer → {'PASS' if cond2 else 'FAIL'}")

verdict = "CONTINUE" if (cond1 or cond2) else "SIMPLIFY"
print(f"\nGATE VERDICT: {verdict}")
if cond1:
    print("  Criterion 1 satisfied: progressive beats ASCII by ≥10% accuracy.")
elif cond2:
    print("  Criterion 2 satisfied: equal-or-better accuracy at ≥30% fewer tokens.")

json.dump({"results": results, "gate": {
    "cond1_pass": cond1, "cond2_pass": cond2, "verdict": verdict,
}}, open("bench/eval/judge_results.json", "w"), indent=2)
print("\nSaved to bench/eval/judge_results.json")
