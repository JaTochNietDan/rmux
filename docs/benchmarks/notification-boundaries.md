# OSC boundary parsing cost

Release-mode measurements on the local Apple Silicon Mac with Ghostty 1.0.1.
The same executable compares the pre-OSC-99 raw write path with the current
boundary-aware path. Three samples alternate order, with 32 warmup chunks and
1024 measured chunks per terminal. Results are median aggregate MiB/s. This
measures parsing only: no PTY, frame extraction, GPU presentation or UI latency.

| Workload | Panes | Previous path | Current path | Change |
|---|---:|---:|---:|---:|
| plain | 1 | 983.8 | 934.3 | -5.0% |
| ansi_unicode | 1 | 330.7 | 332.0 | +0.4% |
| shell_metadata | 1 | 440.9 | 318.8 | -27.7% |
| plain | 8 | 1205.9 | 1191.9 | -1.2% |
| ansi_unicode | 8 | 329.2 | 329.0 | -0.1% |
| shell_metadata | 8 | 428.0 | 318.7 | -25.5% |

OSC-heavy traffic pays for extra boundary calls; this synthetic workload emits
two OSC sequences per line. Its 25–28% throughput reduction remains an optimization
target. Normal ANSI/Unicode output was within 1% in this run. Plain-text results
varied by pane count. These observations do not prove a UI latency budget.

Engine resident bytes matched for every paired sample (roughly 4 MiB per terminal).
Those counters exclude Rust wrapper allocations and process-wide RSS.

Two fast-path experiments scanned for potential OSC-start bytes before selecting
the old bulk API. Scalar scanning slowed plain output by 8–11%; SIMD-backed
scanning gave no consistent gain over the existing boundary path across workloads.
Both experiments were removed. No performance claim relies on their results.

[Raw accepted-path samples](notification-boundaries.jsonl)

Reproduce:
```sh
mise exec -- cargo test --release -p rmux-terminal notification_stream_boundary_throughput -- --ignored --nocapture
```

## Conservative prefix fast path

A subsequent optimization selects the original bulk API only when no OSC is
being captured, a chunk has no `9`, and it does not end in a possible OSC
introducer byte. All other chunks retain boundary parsing. This excludes complete
and split literal `99;` prefixes without adding a second VT parser.

The same alternating three-sample comparison after this change:

| Workload | Panes | Previous raw path | Optimized path | Change |
|---|---:|---:|---:|---:|
| plain | 1 | 1073.5 | 1036.5 | -3.5% |
| ansi_unicode | 1 | 332.2 | 329.4 | -0.9% |
| shell_metadata | 1 | 431.4 | 427.9 | -0.8% |
| plain | 8 | 1204.3 | 1174.0 | -2.5% |
| ansi_unicode | 8 | 330.8 | 331.0 | +0.0% |
| shell_metadata | 8 | 436.8 | 442.7 | +1.3% |

[Optimized raw samples](notification-prefix-fast-path.jsonl). Engine-resident
bytes still match in every pair. The shell-metadata fixture contains no digit 9,
so it exercises the shortcut; metadata containing that digit still takes the
slower path. These measurements do not establish a universal overhead reduction.
Differential tests compare notification revisions/content and screen text against
the boundary-only path at every split of mixed streams, including cancelled and
UTF-8 sequences and OSC-like content inside DCS.
