# Initial macOS release app baseline

November 19, 2026, Apple M4 Max / 48 GiB, arm64 macOS 27.2.1 (26B131). Rust 1.99.1, GPUI 0.2.2, libghostty-vt 1.0.1. Release binary SHA-256: `da88e92ba3ecbd4eb6890dcd137830fef6cabdf4b4f7f09e7d393bae1ed85e16` (6.3 MiB as reported by `ls -lh`).

Built using `mise run bundle-release`. Separate benchmark bundles were prepared using `mise exec -- sh scripts/prepare-macos-benchmark.sh 1` and `8`, then launched through native app control. Each showed the expected number of visible shell prompts. Shell workload: `/bin/sh -i`, `ENV=/dev/null`, no user startup scripts, no output producer. Metal HUD disabled for these bundles only. Cursor blink is not implemented; no blinking timer was active.

Window requested size was 1000×680 logical units; screenshots were 2000×1424 including window chrome (2× scale). Host main display reports 144 Hz, with a separate 120 Hz display also connected. Actual presentation cadence and continued visibility/focus throughout sampling were not instrumented. Computer-control window sharing was active. Other desktop activity was not controlled. These limitations prevent treating this as a controlled acceptance run.

## Observations

| Metric | 1 pane | 8 panes |
| --- | --- | --- |
| Frontend CPU time over 60 seconds | 0.53 seconds | 1.02 seconds |
| Equivalent CPU, one core = 100% | 0.88% | 1.70% |
| Final 30-second interval | 0.90% | 0.90% |
| Frontend RSS, initial → final | 94.5 → 94.5 MiB | 100.4 → 107.7 MiB |
| `vmmap` physical footprint | 52.8M at initial observation | 57.0M initially; 74.5M after sampling |
| `vmmap` recorded peak footprint | 283.3M | 287.5M initially; 309.0M later |
| Direct shell RSS total | 2.7 MiB | 21.8 MiB |

CPU comes from cumulative `ps time` differences, not instantaneous `%CPU`. RSS is KiB converted to MiB. Footprint values retain `vmmap`'s units/rounding. Shells recorded no CPU change at `ps`'s hundredth-second resolution. Shell RSS totals can double-count shared pages and are not physical-footprint totals. Graphics attribution is incomplete: IOSurface residency was 21.0M initially and 31.5M in the later eight-pane snapshot. This is not an independently measured total GPU allocation.

The startup peaks and later growth must remain visible. Do not infer a steady per-pane memory cost from these samples. The provisional <0.5% idle CPU gate is **not met by this observation**; a controlled run with warm-up, explicit visibility and redraw/presentation counters is needed before attributing or optimizing the overhead. GPUI's visible-window display-link behavior is a candidate for investigation, not a demonstrated cause.

A separate three-second stack sample after the idle interval showed the main thread waiting in the native event loop and PTY workers waiting on I/O or channels. It did not establish which work accounts for the preceding CPU time. No dependency changes were made based on this inconclusive sample.

Normal quit removed the measured frontend processes and their direct clean-shell children in both runs. This checks basic shutdown, not descendant process-group handling or long-running lifecycle retention.

## Evidence

- [One-pane idle samples](app-m4-max-2026-11-19-1-pane-idle.txt), [initial memory](app-m4-max-2026-11-19-1-pane-vmmap.txt)
- [Eight-pane idle samples](app-m4-max-2026-11-19-8-pane-idle.txt), [initial memory](app-m4-max-2026-11-19-8-pane-vmmap.txt), [later memory](app-m4-max-2026-11-19-8-pane-end-vmmap.txt)
- [Post-interval stack sample](app-m4-max-2026-11-19-idle-stacks.txt), binary-image listing omitted

Input-to-display latency, output-pressure responsiveness, 16-pane UI behavior, startup timing, repeated lifecycle retention, Linux and Windows remain unmeasured.
