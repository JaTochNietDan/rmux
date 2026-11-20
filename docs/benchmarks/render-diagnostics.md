# Opt-in renderer diagnostics

Set `RMUX_RENDER_METRICS` to an output file before launching rmux. When enabled, F12 appends one JSON line per live terminal in the active workspace. The parent directory must exist. Normal launches do not reserve F12, collect timing samples or write diagnostic files.

Each record includes `ghostty_memory` (or null if querying fails). Primary and
alternate resident bytes count Ghostty screen pages; compressed bytes are already
included. Image bytes are separate. Virtual bytes describe reserved address space,
not resident memory. These counters exclude Rust snapshots, shaping caches, fonts,
GPU resources and the rest of the process, so pair them with process memory data.
Memory queries run only on the requested dump and do not add idle polling.

For a release macOS benchmark bundle:

```sh
mise run bundle-release
RMUX_RENDER_METRICS=/tmp/rmux-render.jsonl mise exec -- sh scripts/prepare-macos-benchmark.sh 8
```

Open the resulting `target/release/rmux-bench-8.app`. Press F12 once after startup settles, leave the window visible and quiet, then press F12 again. Pair these records with the read-only process sampler described in [app baseline](app-baseline.md). For an interactive development launch, `RMUX_RENDER_METRICS=/tmp/rmux-render.jsonl mise run run` enables the same diagnostics.

Each record includes process ID, a process-local pane ID, timestamp, elapsed pane lifetime, cell dimensions, focus/window-active state, parsed output bytes, received output notifications, reused/rebuilt row counts, and prepaint/paint call counts plus total/max CPU durations. Counters are cumulative. Compare records with the same process and pane IDs, subtract totals and counts, and divide the difference in total duration by the difference in count for an interval mean. Lifetime maxima are not percentiles and cannot be differenced to recover an interval maximum.

Prepaint timing covers terminal resize/snapshot work, row-cache updates and cursor preparation. Paint timing covers CPU-side GPUI draw-command preparation and input registration. The row counters distinguish engine-row reuse from rebuilding shaped rows; they do not count GPUI glyph-atlas hits. Cursor/preedit shaping is outside the row-cache counters. Output notifications are coalesced UI notifications, not PTY reads.

No timer polls or flushes these counters. Storage is fixed-size per pane; no growing sample vector or per-frame log is retained. File I/O occurs only on the explicit F12 request, outside the measured prepaint/paint intervals. The dump keystroke and changes in focus can themselves cause redraws, so a small boundary count is not proof of an idle loop.

These counters do not measure GPU execution, compositor presentation, input-to-pixel latency or complete app CPU time. `window_active` records key-window state, not occlusion or visibility across the entire interval. Observe window visibility during a run and report display refresh/DPI and background activity separately.

## First native capture

On November 19 local time (November 20 UTC), a fresh one-pane release bundle
produced two captures 92.357 seconds apart. Both reported three prepaint/paint
calls, one output notification, ten parsed PTY bytes, 64 rebuilt rows and 32 reused
rows. No additional terminal rendering occurred in that interval. The pane and
window were focused/active at both capture boundaries. Screenshots at the
boundaries showed the clean shell prompt; continuous visibility was not measured.

The nested 60-second process sample increased cumulative frontend CPU from 0.91
to 0.92 seconds (approximately 0.02% of one core, at `ps`'s coarse hundredth-second
resolution). RSS increased from 106928 to 106976 KiB (about 104.4 MiB). The direct
shell remained at 0.01 CPU seconds and 2800 KiB RSS. Quit removed both recorded
PIDs. This does not measure physical footprint, GPU memory or lifecycle plateaus.

Binary SHA-256: `bfa2354bb1810210816088403e994abb8ddfeadaeedf94642fc6146241e0012c`.
Built with `mise run bundle-release`, one clean benchmark shell, Metal HUD off,
117×32 cells. Same M4 Max host as the initial baseline. Native control/window
sharing was active; other desktop activity and refresh behavior were uncontrolled.

This observation is below the provisional idle CPU threshold but is not a
controlled before/after result or a multi-platform acceptance run. It weakens the
case for changing GPUI based solely on the earlier high idle sample. GPUI source
contains visible-window display-link callbacks and a one-second presentation tail
after input; neither behavior establishes the cause of the earlier measurement.
Multi-pane measurements, long-run memory, output pressure and native platform
coverage remain open.

Evidence: [render counters](app-m4-max-2026-11-19-instrumented-render.jsonl),
[process samples](app-m4-max-2026-11-19-instrumented-idle.txt).

## Eight-pane capture

The same binary and host then ran eight clean shells in the same window, each
14×32 cells. The two F12 captures were 92.613 seconds apart. Every pane remained
at three prepaint/paint calls, one output notification and 16 parsed bytes: no
additional terminal rendering. The window was active at both boundaries and pane
8 retained focus. Boundary screenshots showed the eight prompts; continuous
visibility and refresh rate were not measured.

During the nested 60-second sample frontend CPU increased from 1.03 to 1.04
seconds, again approximately 0.02% of one core at coarse timer resolution.
Frontend RSS decreased from 113024 to 112560 KiB (110.4 to 109.9 MiB). Each of eight
shells stayed at 0.01 CPU seconds and 2800 KiB RSS. Summed shell RSS was 21.9 MiB,
which can double-count shared pages. Physical footprint was 49.8M initially and
49.9M afterward; startup peak remained 275.4M. Preserve the peak when reporting
these results. This short run cannot establish long-term memory retention, and
the narrow panes are not an output-pressure or full-sized-grid workload.

After sampling, Ctrl+Shift+Tab followed by Cmd+W removed pane 7. A separate metrics
capture contained pane IDs 1–6 and 8, now 16 columns wide, with pane 8 focused.
The shell PID for pane 7 disappeared while the other seven remained. Cmd+Q then
removed the frontend and all recorded shell PIDs. This verifies this basic native
focus/close path, not arbitrary descendants or repeated lifecycle retention.

Evidence: [eight-pane render counters](app-m4-max-2026-11-19-instrumented-8-render.jsonl),
[process samples](app-m4-max-2026-11-19-instrumented-8-idle.txt),
[initial footprint](app-m4-max-2026-11-19-instrumented-8-vmmap.txt),
[ending footprint](app-m4-max-2026-11-19-instrumented-8-end-vmmap.txt),
[post-close counters](app-m4-max-2026-11-19-pane-close-render.jsonl).
