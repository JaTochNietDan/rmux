# rmux performance acceptance plan

Manual process-tree sampling baseline (`22dc885` plus harness/shared-index change,
macOS arm64, debug profile): at 1/8/32 shells, warm medians were
29.025/27.865/29.464 ms after sharing parent-child indexes across terminal roots,
versus 37.818/36.553/44.938 ms before. Each variant has one cold sample and five
warm samples per size; [raw summaries](benchmarks/process-trees-debug.jsonl).
Run `mise run bench-process-trees` to repeat. This includes two host process-table
snapshots, ownership checks and RSS/CPU sampling, but excludes UI frames and
fixture creation. Fixtures are idle shells with no children, exercising the
multi-root lookup cost, not deep/wide trees or the 4,096-process cap. Host activity
can affect these sequential runs; the one-shell change also shows timing variance.
This is not release-build, memory-allocation or cross-platform acceptance evidence.
Each parent-child index is now built once per refresh instead of once per root;
aggregation also streams samples without a temporary vector per tree.

Local file-loader baseline (`1794455` plus the benchmark harness, macOS arm64,
debug profile): 40 reads of a freshly generated 2,048-entry directory measured
2.486 ms p50 / 2.608 ms p95. A 128 KiB Unicode fixture, capped to a 64 KiB read
and 512 preview lines, measured 0.085 ms p50 / 0.095 ms p95 and retained 3,584
bytes of line text. [Raw samples summary](benchmarks/file-loaders-debug.jsonl).
Run `mise run bench-file-loaders` to repeat. This is a local, mostly warm-cache
baseline, excludes fixture creation, UI construction/rendering, allocator
overhead and clipboard writes, and is not a release-build or slow-filesystem
acceptance result. Retained text bytes are not total memory usage.

New-tab PTY creation runs in a background task, with one pending launch per
pane and cached directory inheritance. PTY creation plus initial frame/color preparation run off the
UI thread for new tabs. Settings changes during launch trigger background
re-preparation before the view is attached. Subsequent focus/layout work may
still access the parser. Splits/workspace startup paths remain separate work; native startup
latency and rapid create/close acceptance are still pending.

Find selection seeding uses a nonblocking parser-lock attempt and a streaming
4 KB export. Oversized/truncated or multiline selections fall back to the
remembered query. Streaming avoids the previous full-selection size probe
before enforcing the limit; this bounds emitted text rather than guaranteeing
a particular formatting latency.
Complete selection copying also uses this streaming formatter, avoiding a full
size-probe pass and a second copy from a temporary byte buffer. Its 16 MiB
default limit and rejection of partial selections remain in place; clipboard
export continues to run in the background. Superseded exports abandon parser
lock contention through a 1 ms retry backoff; this is not an idle timer or a
guarantee of cancellation latency. Formatting already in progress is not interrupted. A focused
[selection-export comparison](benchmarks/selection-export.md) records engine-only
timing improvements and its measurement limitations.

Prepaint now uses opportunistic snapshot access: a busy parser keeps the cached
frame and schedules a display-frame retry. `snapshot_busy` counts these misses
when diagnostics are enabled. No retry is scheduled after a successful snapshot.
Resize also attempts parser access without waiting: contention leaves dimensions
unchanged and retries the latest layout size next frame. `resize_busy` records
these deferrals. Actual PTY resizing and Ghostty reflow still execute synchronously
after acquiring access; this change removes lock waiting, not reflow cost. Other
engine interactions remain separate performance work.

Output metadata updates also use opportunistic access. Contention preserves
revision cursors and retries within the existing per-terminal task after 8 ms;
there is no retry timer while idle. `metadata_busy` counts misses. Program-status
details and render-delay state are read with the same metadata lock to avoid
immediate blocking reacquisition. Synchronized-output deadline callbacks use a
nonblocking render-delay query and retry only on contention.

For opt-in renderer counters, launch with `RMUX_RENDER_METRICS` set to an output
JSONL path, then press F12 to append a snapshot of all tabs in the active
workspace. File writes run in the background; another export is ignored while a
write is pending. `cache_releases` and `rows_released` count visibility-driven
cache drops, not bytes freed. Compare these with `rows_rebuilt`, `rows_reused`
and prepaint/paint timing deltas during repeated tab, workspace and zoom changes.
Use OS footprint/RSS sampling separately; these counters do not measure GPU
atlas retention or whole-application memory.
`cursor_glyphs_reused` and `cursor_glyphs_rebuilt` count block-cursor glyph cache
hits and shaping calls. They stay unchanged for hidden cursors, bar/underline/
hollow cursors and concealed or empty covered cells. Compare deltas across
repaints and font/color changes; these counts are not latency measurements.
Engine memory collection uses a nonblocking lock attempt. `ghostty_memory: null`
means that engine was busy or its memory query failed; it never means zero usage.

`snapshot_storage` accounts for the cached Rust frame: cell-vector capacity in
bytes, logical text bytes, inline/heap cell counts and heap text payload bytes.
Inline cells include empty cells. Collection scans existing snapshots only on
explicit export, without locking the terminal or creating a fresh frame. Hidden
tabs may retain older snapshots. Counts exclude allocator/Arc overhead, shaped
glyphs and GPU storage, and do not deduplicate shared allocations across frames;
they must not be summed as process RSS.

The goal is responsive input and bounded resource use under realistic multi-pane workloads. The figures below are **initial engineering targets**, not measurements or guarantees. P0 must identify reference hardware, OS versions, display settings and reproducible fixtures before approving budgets.

An initial [release engine baseline](benchmarks/README.md) is available for 1/4/16 terminals. It isolates parsing and snapshots; none of the end-to-end application gates below are satisfied by it alone.

The first [macOS release app observation](benchmarks/app-baseline.md) covers one and eight idle clean shells, memory attribution and basic shutdown. Its CPU results exceed the provisional idle target; measurement limitations and unresolved memory growth are recorded alongside the raw evidence.

A [PTY output-pressure baseline](benchmarks/pty-baseline.md) covers 1/4/16 sessions with a responsive child alongside up to 15 flooding children. It measures input calls and response snapshots, not GUI input-to-display latency.

[Opt-in renderer diagnostics](benchmarks/render-diagnostics.md) now include native
one/eight-pane captures with zero additional terminal paints over roughly 92
seconds and approximately 0.02% frontend CPU over nested 60-second samples.
These later observations are below the provisional idle target, but uncontrolled
desktop conditions prevent attributing the difference from the original run to
a particular change. Output-pressure GUI latency and cross-platform gates remain
open.

## Measurements

| Metric | Definition | Initial gate |
| --- | --- | --- |
| Input dispatch | OS key event received to encoded bytes admitted to the PTY writer | p99 under 2 ms under the defined local workload; measure queue/drain delay separately |
| Visible input latency | Input event to corresponding displayed pixels | Measure end to end; aim for no more than one display interval above the matched baseline at p95 |
| Frame latency | Visible state change to presentation | Target the refresh interval under representative output; 16.7 ms at 60 Hz, 8.3 ms at 120 Hz |
| Workspace switch | Selection event to correct visible frame | Next available frame for an already-live workspace; no process recreation |
| Idle CPU | App and required helpers over a quiet 60-second interval | Initial target below 0.5% of one core with blink/animations disabled; normalize OS conventions |
| Startup | Process launch to responsive window, and separately to usable shell | Record cold/warm medians and p95; set absolute budget after P0, controlling shell startup config |
| Memory | App, graphics allocations where measurable, helpers and workloads | Report fixed cost and incremental visible/hidden pane cost; set absolute budgets after P0 |
| Cleanup | Resource growth after repeated creation/destruction and warm-up | No sustained retained growth; explain cache plateaus, allocator retention and OS delayed reclamation |

Screen latency must not be inferred solely from an internal dispatch timer. Use high-speed capture or a validated external latency harness when available. Record presentation timestamps separately. Report p50, p95, p99, sample count and run-to-run variance; do not hide tails in averages.

## Workload matrix

| Scenario | Setup | What it should catch |
| --- | --- | --- |
| Quiet baseline | 1, 8 and 32 idle shells; 1 or 8 visible panes | Per-pane idle timers, redraws, watchers and memory cost |
| Output pressure | One producer flooding output while typing into another pane | Input starvation, oversized queues and main-thread parsing |
| Many sessions | 64 background sessions with a defined mixture of idle/output activity | Scheduler fairness and hidden-pane costs |
| Real applications | Editor, process monitor, git UI, build logs and agent CLI | Alternate screen, cursor, mouse and bursty redraw behavior |
| Split interaction | Drag dividers and move tabs during output | Full-tree layout work, stale sizes and dropped input |
| Text coverage | Unicode/CJK/emoji/combining fixtures at multiple DPI values | Shaping cost, cache growth and correctness regressions |
| Scrollback | Fixed byte/line budgets, repeated searches and resize reflow | Duplicated buffers, unbounded search and reflow stalls |
| Graphics | Large/replaced terminal images with explicit limits | Unbounded decoded-image/GPU caches |
| Browser | 0, 1 and 8 tabs; active, hidden and reclaimed states | Startup overhead, helper growth and terminal/browser contention |
| Remote | Delay, packet loss, disconnect/reconnect and host sleep | Retry storms, stale state and input ordering |
| Lifecycle | 100 open/close cycles, repeated app restore, one-hour soak | Leaked tasks, PTYs, descriptors, processes and graphics resources |
| Event consumers | Many subscribers including deliberately slow clients | Socket backpressure starving user input |
| Provider integrations | Quiet agents and activity bursts with indexing/hooks enabled | Polling, transcript scans and child-process overhead |

## Fair comparison

Use the same fonts, DPI, window dimensions, terminal cells, theme, scrollback budget and shell startup for rmux, Ghostty and cmux. Record exact application revisions and enabled features. Native cmux desktop comparison is on macOS; on other platforms compare rmux to available native terminals with matched behavior, not an imaginary cmux binary.

Report at least three memory totals: frontend only; frontend plus its browser/remote/helper descendants; total workload including shells and agents. On macOS use physical footprint in addition to RSS; on Linux report PSS where available; on Windows record private commit/working-set measures. These OS metrics are not directly interchangeable. GPU and shared-memory accounting limitations must remain visible.

An agent using gigabytes is a workload cost even if the terminal frontend is small. Hibernating that agent changes behavior and must be a separately labelled experiment. Browser reclamation also changes reload behavior and must not be quietly enabled only for benchmark runs.

## Regression policy

Store benchmark configuration and machine-readable results by build. Initially flag a repeatable regression greater than 10% in p95/p99 latency or steady memory, then calibrate against measured noise. Reject sustained leaks, terminal corruption, lost input and idle redraw loops regardless of average scores. Do not rerun noisy benchmarks until a desired number appears; investigate variance.

Memory control requires bounded scrollback, images, event retention, notifications, RPC sizes, logs and transcript caches. Backpressure must preserve byte ordering; dropping terminal output is not an acceptable optimization.

## Lessons from cmux

cmux documents git watcher churn and the tradeoff of disabling Git/PR updates. rmux should account for metadata work as part of idle CPU, share watchers where possible and suspend unnecessary refreshes. [Shell integration](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/shell-integration.md), [watcher investigation](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/performance/15067-bash-watcher-churn.md)

Renderer and lifecycle regressions need explicit stress coverage, including hidden surfaces, queued output and destruction during rendering. [Ghostty fork notes](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/ghostty-fork.md)
