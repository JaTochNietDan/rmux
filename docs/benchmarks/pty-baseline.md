# PTY output-pressure baseline

Run November 19, 2026 on Apple M4 Max / 48 GiB, arm64 macOS 27.2.1, Rust 1.99.1 release profile and libghostty-vt 1.0.1. Reproduce with `mise run bench-pty`. Executable SHA-256: `1fafb73812b10b41d88ab05ed6e14f13375a237d6b968c8ce5ae20561e1081e7`.

The harness starts real PTY sessions using child modes of the same executable, avoiding user shell configuration. One child reads requests and produces distinct response markers. Every other child continuously writes styled Unicode output. Each terminal has 160×48 cells and the normal 4 MiB scrollback budget. After readiness and 200 ms warm-up, the harness sends 128 requests, separated by 5 ms. For each response probe it extracts snapshots from every busy pane before inspecting the responder snapshot. A 100 µs sleep separates unsuccessful probes; a two-second deadline fails missing responses.

The response marker differs from the input, so local PTY echo cannot count as a response. All flood panes were checked for positive parsed-byte progress during the measurement. A relaxed atomic counter records bytes actually fed into Ghostty after PTY reads, rather than bytes merely offered by the producer.

| Total panes | Flooding panes | Aggregate parsed output | Input call p99 | Response snapshot p50 | Response snapshot p95 | Response snapshot p99 | All-pane snapshot batch p99 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | — | 0.052 ms | 0.994 ms | 2.172 ms | 3.826 ms | 1.666 ms |
| 4 | 3 | 254.9 MiB/s | 0.006 ms | 0.584 ms | 0.644 ms | 0.928 ms | 0.852 ms |
| 16 | 15 | 113.6 MiB/s | 0.024 ms | 2.143 ms | 2.369 ms | 2.723 ms | 2.688 ms |

[Raw JSONL](pty-m4-max-2026-11-19.jsonl). Percentiles use nearest-rank selection. All 384 requests received the expected response. No benchmark child processes were found after completion.

These are one-run observations, not performance acceptance results. The one-pane case was slower/noisier than the four-pane case; this does not establish that adding panes improves latency. Machine activity, scheduling and run order were not controlled, and run-to-run variance is not established.

“Input call” starts at `Session::input` entry and includes queue admission and return-to-live viewport handling. It excludes native key dispatch, key encoding and writer drain delay. “Response snapshot” ends when the expected marker is found in an owned engine snapshot, and includes PTY round trip, parsing, busy-pane snapshot contention and harness polling. Neither metric measures input-to-pixel latency. The GUI, glyph shaping, row cache, rendering and presentation are absent; the earlier rendering-cache change is not benchmarked by this harness. Aggregate output throughput is also affected by PTY scheduling and snapshot work, so it is not comparable directly with the parser-only benchmark.

Next relevant measurements are controlled GUI redraw/presentation instrumentation and responsiveness while interacting with the actual app under output load. Linux and Windows runs remain pending.

## November 20 refresh

Current release engine on the same verified M4 Max / 48 GiB machine. Executable
SHA-256: `ff244f01f7872a0607ec37d5866ea62724cd68d8d357807e8f5246ac016433d8`.
Same harness and workload; one 1/4/8/16-pane sequence followed by two extra
16-pane runs to inspect the higher tail. [Raw results](pty-m4-max-2026-11-20.jsonl)
are in execution order. All 768 requests completed; every flooding producer made
progress, and no benchmark echo/flood children remained after completion.

| Panes | Input call p99 | Response snapshot p50 | Response snapshot p95 | Response snapshot p99 | Snapshot batch p99 |
| --- | --- | --- | --- | --- | --- |
| 1 | 0.013 ms | 0.949 ms | 0.981 ms | 1.138 ms | 0.747 ms |
| 4 | 0.007 ms | 0.630 ms | 0.671 ms | 0.682 ms | 0.663 ms |
| 8 | 0.006 ms | 1.259 ms | 1.317 ms | 1.331 ms | 1.316 ms |
| 16, run 1 | 0.019 ms | 2.444 ms | 3.924 ms | 5.662 ms | 5.633 ms |
| 16, run 2 | 0.013 ms | 2.445 ms | 3.939 ms | 4.249 ms | 4.222 ms |
| 16, run 3 | 0.017 ms | 2.440 ms | 3.917 ms | 4.865 ms | 4.840 ms |

Sixteen-pane p99 ranged 4.249–5.662 ms across these short runs, higher than the
November 19 single-run value. Scheduling/environment were not controlled enough
to attribute that difference to a code regression. The close match between
response and all-pane snapshot tails identifies snapshot collection as a useful
area for investigation, not proof of a particular bottleneck. This benchmark
still excludes GPUI and cannot validate input-to-pixel latency.

Following source inspection, desktop prepaint now tries snapshot access without
waiting and retains the last coherent frame on contention, scheduling a next-frame
retry. Opt-in renderer metrics expose `snapshot_busy`. This desktop change is
**not measured by the results above**; native output-pressure/presentation and
resize behavior still require acceptance. Resize and other engine access paths
can still block, so the app is not claimed to be lock-free or lag-free.

## Keyboard dispatch into an output-flooding pane

`mise run bench-keyboard` measures 128 key press, release and native-text routing
calls while the targeted terminal itself parses continuous colored Unicode output.
All 1/4/8/16 panes produce output, unlike the original benchmark's quiet responder.
The harness waits for its owned children to exit before moving to the next size.
Raw results are in `pty-keyboard-same-pane.jsonl` (local macOS release build).

| Flood panes | Key press p99 | Largest press | Release p99 | Native-text routing p99 |
| --- | --- | --- | --- | --- |
| 1 | 0.216 ms | 0.244 ms | 0.103 ms | 0.029 ms |
| 4 | 0.022 ms | 0.037 ms | 0.013 ms | 0.0003 ms |
| 8 | 0.016 ms | 0.018 ms | 0.018 ms | 0.0005 ms |
| 16 | 0.028 ms | 0.153 ms | 0.0003 ms | 0.0006 ms |

This is one short sample per size, not a worst-case bound. Scheduling and output
throughput vary with pane count; lower values at more panes are not evidence of
better scaling. There is no GUI rendering, input-to-presentation measurement,
Kitty keyboard mode transition, IME composition or child consumption timing here.
The encoder still shares the parser mutex; these measurements establish a
regression baseline, not a nonblocking guarantee. No locking change was made.

## Focus dispatch into an output-flooding pane

`mise run bench-focus` enables application focus reports and measures 128 focus-in
and focus-out calls into a busy terminal for each 1/4/8/16-pane run. Every pane
emits colored Unicode output and a working-task OSC report per output chunk.
Focus-in includes the UI's follow-up status-summary read. The optional full
status-record read is timed separately. Owned children finish before the next run.
Raw results: `pty-focus-same-pane.jsonl`, local Darwin arm64 release build using
engine source at `a8ca544` plus this benchmark addition.

| Flood panes | Focus-in p99 | Largest focus-in | Focus-out p99 | Status details p99 |
| --- | --- | --- | --- | --- |
| 1 | 0.175 ms | 0.184 ms | 0.083 ms | 0.075 ms |
| 4 | 0.027 ms | 0.030 ms | 0.0028 ms | 0.0020 ms |
| 8 | 0.015 ms | 0.016 ms | 0.0006 ms | 0.0010 ms |
| 16 | 0.043 ms | 0.049 ms | 0.0004 ms | 0.0016 ms |

This short sample measures dispatch/encoding and lock acquisition, not a worst-case
bound. It has one working-task record per terminal, no GUI/OS focus switch,
rendering, key-release cleanup, application consumption or presentation timing.
The child generates output rather than reading/acknowledging focus reports.
Scheduling and aggregate output throughput differ across runs, so smaller timings
at higher pane counts do not establish improved scaling. No focus-locking change
was made based on this sample; synchronous focus handling still has no nonblocking
guarantee. Other UI-thread lock paths and visual input latency remain separate work.
