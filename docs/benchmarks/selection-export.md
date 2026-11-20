# Selection export comparison

Measured on Apple M4 Max, arm64, macOS 27.2.1 (26B131), using release builds.
Run with `mise run bench-selection`.

The fixture fills one 100×32 terminal with repeated ASCII/CJK/combining text,
selects all retained history and measures 20 warmed exports per case. It uses
4 and 64 MiB scrollback budgets. Comparisons check stable result content within
each run; both versions returned the same lengths and rejection outcomes.

| Retention budget | Selected bytes | Limit | Two-pass p50 / p95 (µs) | Streaming p50 / p95 (µs) |
| --- | ---: | --- | ---: | ---: |
| 4 MiB | 282,099 | 4 KB, rejected | 785 / 840 | 19 / 24 |
| 4 MiB | 282,099 | 16 MiB, complete | 1,494 / 1,588 | 728 / 749 |
| 64 MiB | 4,787,949 | 4 KB, rejected | 11,200 / 11,353 | 23 / 26 |
| 64 MiB | 4,787,949 | 16 MiB, complete | 23,277 / 23,873 | 12,427 / 12,843 |

[Streaming raw results](selection-streaming.jsonl) use b72bf76.
[Two-pass raw results](selection-two-pass.jsonl) use the same tree and fixture,
with only selection.rs temporarily restored from 6f8d39d. The source was restored
and the streaming binary rebuilt after measurement. Streaming ran first; each
version ran once, so run-order, thermal and background-load effects are not
controlled. Complete-export median time was about 47–51% lower in these samples.

Timings include formatting and result allocation, but exclude dropping the
returned String, lock acquisition, scheduling, clipboard transfer, PTY output,
GPU work and presentation. Retention budgets are not RSS measurements. There is
no multi-pane contention in this fixture. These observations support reduced
selection formatting work; they do not satisfy application latency gates.
