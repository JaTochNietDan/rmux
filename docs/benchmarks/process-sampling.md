# Process sampling baseline

November 20, 2026, Darwin arm64 / macOS 27.2.1, debug test build, sysinfo
0.31.4. Run with `mise run bench-process-sampling` on Unix.

The fixture starts 1, 8 or 32 `/bin/sh` processes blocked on stdin, then samples
their PIDs plus the test application's PID. Each case records one cold sample
and five warm samples spaced by sysinfo's minimum CPU interval plus 5 ms.
Timing covers `Sampler::sample`, including process refresh and result allocation.
Fixture launch, the deliberate waits, UI rendering and Ghostty sampling are
excluded. Owned fixture processes are killed and reaped on exit, including panic.

| Shells + application | Cold | Warm median | Warm maximum |
| --- | ---: | ---: | ---: |
| 1 + 1 | 0.746 ms | 1.298 ms | 1.392 ms |
| 8 + 1 | 2.289 ms | 4.837 ms | 5.542 ms |
| 32 + 1 | 15.658 ms | 12.052 ms | 13.725 ms |

All cases returned the expected number of processes, unavailable CPU on first
sample, and finite nonnegative CPU on the spaced samples. This checks baseline
availability, not accuracy against a separate CPU profiler.

This is one small debug-build run, not a release performance threshold or a
measurement of input-to-paint latency. Manual sampling runs off the UI thread.
Per-PID refresh prevents dead cached processes from being reported; its scaling
cost should be revisited before adding broad descendant discovery. The 256-shell
cap, active workloads, Linux and Windows remain unmeasured by this fixture run.

## Batched refresh follow-up

The sampler now refreshes the requested PID set in one call. If the returned
count is incomplete, it probes individual PIDs for memory, excludes exited
entries, and resets the CPU baseline. This avoids repeating host process-table
enumeration for every shell in the normal case.

Same fixture and debug profile, a subsequent single run:

| Shells + application | Cold | Warm median | Warm maximum |
| --- | ---: | ---: | ---: |
| 1 + 1 | 0.175 ms | 0.783 ms | 0.983 ms |
| 8 + 1 | 0.356 ms | 0.963 ms | 2.637 ms |
| 32 + 1 | 1.206 ms | 2.722 ms | 4.807 ms |

Baseline/CPU checks and exited-process regression passed. These sequential runs
show lower observed cost but are not a controlled statistical speedup estimate.
The partial-batch path still has per-PID enumeration cost; CPU stays unavailable
until a complete batch establishes a fresh baseline and a later spaced refresh
can compute a delta.
