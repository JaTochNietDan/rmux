# History export duration

Recorded 2026-11-20 using `mise run bench-history`, release build, on the local
macOS ARM64 machine. One uncontrolled run; these are engine-only timings, not
interactive latency guarantees.

Host: macOS 27.2.1 (26B131), arm64. Benchmark executable SHA-256:
`fe4d3472f63ebcfb2bc63872d583015ddf935df555536faf393b7e9e611fa64c`.

The fixture feeds 102,400 lines of repeated ASCII, CJK and combining characters
into a 100×32 terminal with 4 MiB and 64 MiB scrollback budgets. Each limit starts
after incremental compression completes. First-access time includes restoration
of needed compressed pages; the following 20 samples measure repeated exports.
Times approximate the synchronous operation's engine-lock hold duration, excluding
lock acquisition, executor scheduling, RPC serialization, PTY and rendering.

| Retention budget | Retained history rows | Output limit | Actual bytes | Truncated | First µs | Warm p50 µs | Warm p95 µs | Warm max µs |
| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: |
| 4 MiB | 4,519 | 64 KiB | 65,536 | yes | 313 | 196 | 198 | 202 |
| 4 MiB | 4,519 | 1 MiB | 323,049 | no | 1,147 | 861 | 884 | 884 |
| 4 MiB | 4,519 | 16 MiB | 323,049 | no | 1,110 | 781 | 793 | 821 |
| 64 MiB | 77,194 | 64 KiB | 65,536 | yes | 235 | 155 | 172 | 183 |
| 64 MiB | 77,194 | 1 MiB | 1,048,576 | yes | 3,191 | 2,324 | 2,342 | 2,348 |
| 64 MiB | 77,194 | 16 MiB | 5,482,974 | no | 15,906 | 12,550 | 12,750 | 12,801 |

The capped 64 KiB export did not grow with total retained history in this fixture,
consistent with the formatter stopping at the output limit. Unrestricted export
of the larger history approaches a frame interval by itself. Keep the live RPC
cap at 1 MiB (64 KiB default); larger core exports require explicit background
use. Even capped exports can contend with UI engine access. Concurrent output,
many terminals, different content, allocation peaks and actual frame latency
remain unmeasured. No additional performance claims follow from this baseline.
