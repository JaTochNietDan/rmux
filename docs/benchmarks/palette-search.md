# Palette search computation

Measured November 20, 2026 on Apple M4 Max, Rust 1.99.1, with `rustc -O`.
Search implementation: d19baec; benchmark added after 1cb8110. Raw samples:
[palette-search.jsonl](palette-search.jsonl).

Run `mise run bench-palette-search`. The standalone harness imports the actual
matcher and needs no native window, GPUI build, PTY or new tooling.

| Candidates | Full rescan median, µs/query | Incremental median, µs/query |
| --- | ---: | ---: |
| 128 | 4.190 | 2.152 |
| 1,024 | 25.758 | 14.417 |
| 8,192 | 190.640 | 106.073 |

Each sample averages 32 repetitions of 13 edits, including appended characters,
multiple words, backspace, clearing, and Unicode. Candidate names/paths are
synthetic and intentionally include both matching and nonmatching groups. Three
samples alternate measurement order. Full rescans reproduce the former ranking
and result-allocation path. Both approaches must return identical ordered results
for the complete edit sequence before measurement. Timing excludes palette
construction and warms both paths first.

This fixture reports about 44–49% lower median search computation time. It does
not measure native event handling, row rendering, memory allocations, first-open
latency or p95 input latency. Other query distributions may narrow less and gain
less. Virtual-list performance and visual acceptance remain unmeasured.
