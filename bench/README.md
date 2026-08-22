# Agent Evaluation Harness (Phase 7)

Benchmarks **task accuracy per unit of context + interaction cost** — the
plan's key metric. The harness measures what each mode *costs*; an LLM judge
(Phase 7 evaluation runs) scores *accuracy* against the captured context.

## Layout

```
bench/
├── fixtures/          # 001-ui.png, 002-diagram.png, 003-button.png, 004-screenshot.png
├── cases.jsonl        # {id, image, task, type} — types: spatial, structure, detail,
│                      # geometry, counting, negative
├── run-case.sh        # one case × one mode → one JSON line (capture format)
├── agent-eval.sh      # full matrix: all cases × all modes → results/
└── results/           # <mode>.jsonl per mode + summary.json cost table
```

## Run modes

| Mode            | What the "agent" sees                          |
|-----------------|------------------------------------------------|
| `baseline`      | nothing (no visual input)                       |
| `ascii`         | `ae render --renderer ascii --width 80`         |
| `blocks`        | `ae render --renderer blocks --width 80`        |
| `ae-inspect`    | `ae inspect` JSON (+ text render)               |
| `ae-progressive`| `inspect` JSON → follow first region via `region` |
| `ae-negative`   | `geometry` JSON only — proves OCR absence       |

## Usage

```bash
cargo build --release -p ae-cli
cd bench
./agent-eval.sh ../target/release/ae      # full matrix → results/
./run-case.sh 001 ae-inspect              # single case, single mode
```

## Capture format (one JSON line per run)

```json
{"case":"001","mode":"ae-inspect","answer":"…","task":"…",
 "ae_calls":["inspect","inspect-text"],"ae_tokens_used":2047,
 "tool_calls":2,"bytes_transferred":8190,"duration_ms":16}
```

## Philosophy

> Don't benchmark to prove `ae` is good. Design benchmarks that can also
> prove `ae` is NOT useful.

If `ae + agent ≈ ascii + agent` on Phase 7 scoring, cut agent features and
keep rendering only. The `negative` case type exists to verify `ae` never
pretends to extract text it cannot see.
