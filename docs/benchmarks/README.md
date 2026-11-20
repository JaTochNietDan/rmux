# Initial engine baseline

## Inline cell text

The snapshot cell representation now uses `smol_str` 0.2.2, already present in
the dependency graph, keeping up to 23 bytes inline and retaining long graphemes
in heap-backed storage. The UTF-8 decoder passes borrowed valid text directly to
this representation instead of allocating a `String` per populated cell.
Renderer text runs still use owned strings for shaping.

The same eight-engine release fixture recorded 251.6 MiB/s, snapshot p50/p99
0.134/0.148 ms, and unchanged reads averaging 58 ns. [Raw run](engine-eight-pane-inline-cell-text.jsonl).
The earlier eight-engine run recorded 227.2 MiB/s and 0.156/0.186 ms; these are
separate single runs with intervening changes, not a controlled attribution or
statistical speedup claim. Ghostty memory counters are unchanged because they
exclude Rust snapshot strings. No whole-app RSS reduction has been measured.
All 61 terminal tests passed, including inline ASCII/CJK/combining text and
heap-backed long-grapheme preservation; desktop compilation passed.

## Eight-pane follow-up after dirty-state cleanup

November 20, 2026, release build on the macOS host. Both harnesses now accept
eight panes, and their `all`/mise task matrices include 1/4/8/16. Engine idle
sampling now consumes the final unsampled dirty frame before starting its timer;
older idle means included that frame amortized over 1,000 reads per terminal.

Commands, run sequentially:

```sh
mise exec -- cargo run --release -p rmux-terminal --example engine_bench -- 8
mise exec -- cargo run --release -p rmux-terminal --example pty_bench -- 8
```

The [engine run](engine-eight-pane-dirty-cleanup.jsonl) processed 504 MiB at
227.2 MiB/s across eight sequential 160×48 engines. Over 8,192 flood snapshots,
p50/p99 were 0.156/0.186 ms; unchanged snapshot mean was 63 ns. Continuous
scrolling changes the full viewport, so clearing stale dirty flags does not
eliminate full-screen decoding in this fixture. Ghostty page storage totaled
31.25 MiB before compression and 3.59 MiB after, retaining 22,584 history lines.
This excludes Rust allocations, process RSS and GPU resources.

The [PTY run](pty-eight-pane-dirty-cleanup.jsonl) used seven flooding children and
one responder. All 128 requests completed, with input-call p99 0.0047 ms,
response-snapshot p50/p95/p99 1.287/1.354/1.366 ms, and snapshot-batch p99
1.329 ms. Aggregate flood throughput was 117.7 MiB/s. No benchmark child modes
remained in the process listing after completion.

These are single-run observations, not a controlled before/after speedup or
input-to-pixel measurement. The UI, shaping and presentation are absent; native
eight-pane pressure acceptance and Linux/Windows coverage remain open.

Measured executable SHA-256 values: engine
`34ce52e1d2b68fdc31a7fdbcba4867fe4eca5bdf0105d766fd4e073862801b44`;
PTY `23696362e0f7816d371d953c9d94a8e21ff411396e9568c0a38522c2f1cb0554`.

## Dirty-row identity reuse

Follow-up: snapshot extraction now calls `ghostty_render_state_clean` after
consuming every dirty row, clearing both global and per-row flags. Previously,
only the global flag was cleared, leaving initial full-frame row flags set.
The same release fixture now records the following means:

| Terminals | Identical rewrite mean | One changed row mean |
| --- | --- | --- |
| 1 | 3.85 µs | 2.78 µs |
| 4 | 3.32 µs | 2.23 µs |
| 16 | 2.77 µs | 2.11 µs |

[Raw cleanup results](cleaned-dirty-snapshot-reuse.jsonl). Row identity retention
remains 48/48 and 47/48 respectively. A regression test directly checks that the
next engine update exposes only the edited row after consuming the initial
snapshot. All 55 terminal library tests passed, including selection, scrolling,
resize, alternate screens and synchronized output. These are separate single
runs of the same fixture, not native latency measurements or statistical bounds.

November 20, 2026 release run: `mise exec -- cargo run --release -p rmux-terminal
--example dirty_snapshot_bench` (one command). [Raw results](dirty-snapshot-reuse.jsonl).
At 160×48, 2,000 rewrites per terminal across 1/4/16 sequential terminals:

| Terminals | Identical rewrite mean | One changed row mean |
| --- | --- | --- |
| 1 | 130.3 µs | 122.6 µs |
| 4 | 125.0 µs | 124.7 µs |
| 16 | 126.6 µs | 126.2 µs |

Identical rewrites retained all 48 row identities; changed-row writes retained
47/48 (97.9%). This proves that unchanged rows remain eligible for renderer cache
reuse. Dirty extraction still decodes/compares cells and allocates temporary row
data. Timing includes engine frame extraction only, excluding writes, row-count
inspection, shaping, PTY and GPU. One run, no latency percentiles or full-build
before/after comparison; native performance improvement remains unproven.

## Shared snapshot costs

Follow-up: allocating the UTF-8 scratch buffer only when a dirty row exists
removes its allocation from unchanged frame reads. A subsequent run recorded
64–83 ns unchanged-frame means; four-terminal fixtures measured 67/66/66 ns at
24/48/120 rows. [Raw follow-up](lazy-scratch-snapshot-costs.jsonl). These are
separate single runs, not controlled statistical evidence of a fixed speedup.

November 20, 2026, release build on the same macOS host. Run
`mise exec -- cargo run --release -p rmux-terminal --example snapshot_bench`.
[Raw results](shared-snapshot-costs.jsonl) cover 1/4/16 sequential terminals at
160 columns and 24/48/120 rows, with 20,000 iterations per terminal after warmup.

| Four-terminal fixture | Unchanged frame mean | Frame clone mean | Synthetic vector-copy mean |
| --- | --- | --- | --- |
| 24 rows | 81 ns | 4 ns | 62 ns |
| 48 rows | 79 ns | 4 ns | 100 ns |
| 120 rows | 81 ns | 4 ns | 208 ns |

Across all fixtures unchanged-frame means were 72–99 ns and clone means 3–5 ns.
The synthetic comparison clones row-handle and selection vectors to isolate the
removed operation; it is not an older rmux build. One run, fixed ordering, integer
means, no percentiles or allocation instrumentation. Excludes changing output,
PTY scheduling, shaping, paint, GPU presentation and user input latency. Results
support bounded snapshot-copy cost, not a whole-app speedup claim.

## Scrollback budget comparison

November 20, 2026, same host and release fixture, four sequential engines per run.
[Raw results](engine-scrollback-budgets.jsonl). The benchmark now accepts a second
argument for MiB per terminal (default 4), for example
`mise exec -- target/release/examples/engine_bench 4 1` after building the example.

| Budget per terminal | Total retained history lines | Total engine resident MiB | After compression MiB | MiB/s | Snapshot p99 |
| --- | --- | --- | --- | --- | --- |
| 0 MiB | 0 | 1.56 | 1.56 | 174.4 | 0.135 ms |
| 1 MiB | 1,756 | 3.13 | 1.59 | 267.8 | 0.134 ms |
| 4 MiB | 11,292 | 15.63 | 1.80 | 267.1 | 0.135 ms |
| 16 MiB | 47,052 | 62.50 | 2.57 | 263.7 | 0.138 ms |

Zero retention was slower in this single run despite using less storage; these
results do not justify calling it the fastest setting. Compression benefits here
reflect repetitive styled text. Resident counters cover Ghostty page storage,
not total application RSS, Rust snapshots, fonts or GPU memory. Timings exclude
PTY and UI work. No idle CPU or interactive-latency claim follows from this table.

Measured November 19, 2026 on Apple M4 Max, 48 GiB memory, arm64 macOS 27.2.1 (26B131), Rust 1.99.1 release profile, libghostty-vt 1.0.1. This is a single local baseline, not an acceptance gate or cross-application comparison. Background machine activity was not controlled. No display settings apply to this headless workload.

Reproduce with `mise run bench-engine`. The source fixture is `crates/rmux-terminal/examples/engine_bench.rs`: 160×48 terminals, 4 MiB scrollback per terminal, styled Unicode lines, 512 warm-up chunks per terminal, then 8192 timed chunks with snapshots every eight chunks. All terminals are processed sequentially on one thread. MiB/s includes those periodic snapshots; it does not imply concurrent-session scaling. Percentiles describe individual snapshot extraction calls.

| Terminals | Input MiB | MiB/s | Snapshot p50 | Snapshot p99 | Unchanged snapshot mean |
| --- | --- | --- | --- | --- | --- |
| 1 | 63 | 256.1 | 0.120 ms | 0.149 ms | 274 ns |
| 4 | 252 | 255.1 | 0.123 ms | 0.143 ms | 276 ns |
| 16 | 1008 | 244.3 | 0.128 ms | 0.151 ms | 300 ns |

Raw results: [JSONL](engine-m4-max-2026-11-19.jsonl).

A separate 16-terminal run under `mise exec -- /usr/bin/time -l target/release/examples/engine_bench 16` recorded peak RSS of 84,279,296 bytes (80.4 MiB) and peak physical footprint of 83,001,776 bytes (79.2 MiB). This includes the benchmark process and filled engine scrollback, **not** GPUI, GPU allocations, PTYs or shell processes. [Raw memory run](engine-m4-max-2026-11-19-memory.txt).

The broader performance plan still requires release UI measurements, real PTY pressure, idle CPU, end-to-end input latency, lifecycle retention, run-to-run variance and matched cmux/Ghostty baselines. The small unchanged-snapshot cost is not evidence of low app idle CPU.

## Ghostty allocation accounting follow-up

The harness now queries screen-page memory after timed work. A local release run
of the same fixture produced the following results:

| Terminals | Ghostty resident pages | MiB/s | Snapshot p99 |
| --- | --- | --- | --- |
| 1 | 3.91 MiB | 258.3 | 0.160 ms |
| 4 | 15.63 MiB | 262.9 | 0.141 ms |
| 16 | 62.50 MiB | 260.2 | 0.141 ms |

Each terminal retained exactly 4,096,000 page bytes, within the configured
4,194,304-byte budget; reserved virtual page bytes matched resident page bytes,
and image storage was zero. Allocation scaled linearly for this filled primary
screen fixture. This is engine-owned storage, not process RSS or whole-app memory.
The new samples are a single run, not evidence of a throughput improvement over
the initial baseline. [Raw results](engine-memory-scaling.jsonl).

## Incremental compression experiment

The harness now runs bounded Ghostty compression steps after timed parsing and
queries resident page bytes again. On this repetitive-output fixture, compression
was supported and retained logical history in the focused engine test.

| Terminals | Before | After | Total compression pass |
| --- | --- | --- | --- |
| 1 | 3.91 MiB | 0.45 MiB | 0.284 ms |
| 4 | 15.63 MiB | 1.80 MiB | 1.115 ms |
| 16 | 62.50 MiB | 7.18 MiB | 4.838 ms |

Each terminal's reported resident storage fell from 4,096,000 to 470,684 bytes
(88.5%). The pass timings aggregate all steps sequentially; they are not maximum
step stalls, decompression latency, or UI response times. Savings depend on history
contents and platform support. Automatic application scheduling is not enabled by
this experiment. [Raw results](engine-compression-scaling.jsonl).

A follow-up instrumented individual compression calls and the first scroll-to-top
plus frame extraction after compression. The fixture needed 11 bounded calls per
terminal. Largest calls were 0.050/0.062/0.063 ms for 1/4/16 terminals; largest
history revisits were 0.143/0.151/0.149 ms. The frame-content check passed after each
revisit. These are observed maxima from one compressible fixture, excluding lock
contention, PTY concurrency, text shaping, GPU work and UI scheduling. They are
not worst-case latency guarantees. [Raw timings](engine-compression-latency.jsonl).
