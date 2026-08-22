# LLM-Judge Evaluation (Phase 7 Gate)

Judge = Claude (ox-alpha) reading captured text contexts from each mode,
answering each case's task, scored against `ground_truth.json` derived from
the fixture generator source code. Judge reasoning frozen in `judge.py`
JUDGE dict for reproducibility.

## Results (5 scorable cases; 005-negative scored on honesty separately)

| mode           | accuracy | tokens/case | correct |
|----------------|---------:|------------:|--------:|
| baseline       |      0%  |           0 |   0/5   |
| ascii          |     40%  |         608 |   2/5   |
| blocks         |     40%  |        1143 |   2/5   |
| ae-inspect     |     80%  |        1996 |   4/5   |
| **ae-progressive** | **80%** |    **792** | **4/5** |

## Gate verdict: CONDITION 1 PASSED (+40% accuracy vs ASCII, need ≥10%)

ae-progressive achieves 2× the task accuracy of fixed ASCII at only +30%
token cost — and 31% cheaper than blocks with the same 80% accuracy.
Negative case (005): every mode correctly answered "cannot determine"
(no text exists in any fixture).

Key differentiators for ae modes:
- Region counting: exact from JSON ids vs ambiguous glyph interpretation
- Edge counting: edge_density metric vs impossible from ramps
- Spatial queries: bounds + relations vs approximate glyph positions

Run: `python3 bench/eval/judge.py`
