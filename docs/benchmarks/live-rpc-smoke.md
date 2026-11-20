# Native live RPC smoke

## Last-tab source cleanup extension

Recorded 2026-11-20 against the same executable as the cross-window extension
below (`48ba03d778eea1b42d38e0b1b28fe38ced9ca5bfc8a2d9f212dc6cceb145a074`).
`mise exec -- python3 scripts/smoke-live-rpc.py --live-shell` exited 0.

A sixth shell stored `last-tab-世界`, moved its sole tab to an existing window's
workspace, and answered a fresh probe with that value from its original PID.
The empty source window and workspace disappeared; attempts to move back to the
removed workspace returned `-32004`. All six fixture children exited after test
app termination. Passed output now occurs after cleanup, so cleanup failures
cannot leave a misleading passed report. Visual/OS-focus and busy/restore
destination acceptance remain pending.

## Cross-window tab transfer extension

Recorded 2026-11-20. Built with `mise exec -- cargo build -p rmux-desktop`,
then ran `mise exec -- python3 scripts/smoke-live-rpc.py --live-shell`, exit 0.
SHA-256: `48ba03d778eea1b42d38e0b1b28fe38ced9ca5bfc8a2d9f212dc6cceb145a074`.

A tab from a multi-tab pane moved into another window's workspace and back;
surface/process IDs remained unchanged, the destination selected the moved tab,
repeated same-workspace moves were no-ops, and invalid source/destination IDs
returned errors. Existing shell-variable, workspace-transfer, text-read and
cleanup assertions passed. The first run exposed an incorrect ownership lookup
and crashed; the corrected implementation passed this rerun. Last-tab removal,
busy destinations, visual palette selection and OS activation are not covered.

## Background history export extension

Recorded 2026-11-20. Built with `mise exec -- cargo build -p rmux-desktop`,
then ran `mise exec -- python3 scripts/smoke-live-rpc.py --live-shell`, exit 0.
SHA-256: `27df0787b197b1cb66a9a70881d43d75a9f1c922b12f86de8bc52fd3662a92de`.

History reads returned unwrapped Unicode output for all five retained fixture
sessions. Five-byte history exports reported truncation, and an invalid scope
returned `-32602`. Existing viewport, layout, process continuity, stored shell
state and cleanup checks passed. The focused session contention test separately
verified busy history export returns before a held parser lock is released.
Large-history latency, visual behavior and sustained export pressure remain
unmeasured; background formatting still holds the terminal mutex.

## Viewport text read extension

Recorded 2026-11-20. Built through `mise run smoke-live-sessions`; after correcting
the fixture to account for soft-wrapped visual rows, reran the existing binary
with `mise exec -- python3 scripts/smoke-live-rpc.py --live-shell`, exit code 0.
SHA-256: `2ccbbdc286231cd9326e7c94d06391412a1a0528eac97aa9f9d4a24331aa55c3`.

All five moved fixture sessions exposed their Unicode command output through
surface.read_text. Five-byte reads reported bounded truncation; a zero limit
returned `-32602`. The 29-column split demonstrated that the API preserves soft
wraps as row breaks. The focused engine test covered every byte boundary around
CJK and combining characters, row trimming and exact-limit truncation. Existing
input/state/identity/cleanup assertions passed. Sustained reads and lock-contention
latency were not measured.

## Targeted input and shell-state extension

Recorded 2026-11-20 using `mise run smoke-live-sessions`, exit code 0. Debug
executable SHA-256: `13c371fe7a655ca12ce453698359c3bf6bf149cafeb302a84ec51a95a1a7abec`.

Each live fixture shell receives a distinct Unicode value through surface.send_text
and stores it in a shell variable. Probe replies written to fixture files matched
before transfer; after deleting those files and moving both workspaces, fresh
probes returned the same values through the original surface IDs/PIDs. Sending
text did not change discovered workspace/tab selections. Oversized text and stale
targets returned errors. All earlier checks and child cleanup passed. This does
not measure input throughput under output pressure or verify pixel-level scroll
position, OS focus, complex application state or general shell compatibility.

## Live process transfer extension

Recorded 2026-11-20 using `mise run smoke-live-sessions`, exit code 0. Debug
executable SHA-256: `284a34bcf5822c7fdc7633d0610909e81be581748242ad29731c7e5cde09b08f`.

This variant launches private shell scripts that record their own PID and wait
on PTY input. It verifies all five PIDs exist before transfer, remain identical
and live after moving both source workspaces, and have not been reaped according
to discovery. All five processes disappeared after the harness terminated its
own app. All other smoke checks passed. No user shells/configuration were used.
This establishes live process continuity through transfer, not general shell
state preservation, graceful application quit behavior or latency budgets.

## Workspace transfer extension

Recorded 2026-11-20 using `mise run smoke-live-rpc`, exit code 0. Debug executable
SHA-256: `dceee4ef5d488832d9bf867b2cbdabe598b634d20d7e742b36f0274dde279f14`.

Moved a multi-pane workspace into the second window, repeated the same-owner
request, then moved the source's last workspace. All five terminal IDs and the
original split tree survived; workspace count stayed constant; the empty source
window disappeared; a moved terminal remained targetable. A missing destination
returned `-32004`. The earlier smoke assertions also passed. This fixture uses
immediately exited shells and does not establish running PID/state preservation,
visual focus, activation, or transfer latency.

## Workspace rename extension

Recorded 2026-11-20 using `mise run smoke-live-rpc`, exit code 0. Debug executable
SHA-256: `bcc79e94afa8d111259365dd5e141e8d9fb32b9cc769c514248bbf2a9b9e18e0`.

Added inactive workspace renaming with a Unicode name containing whitespace and
a control character. The returned normalized name matched discovery afterward,
and the selected workspace did not change. Control/whitespace-only names returned
`-32602`; unknown workspace IDs returned `-32004`. All preceding two-window,
layout, focus-state, zoom, identity, notification and EOF checks also passed.
This build includes the JSON ownership and ID lookup allocation cleanup; no
allocation, RSS or latency comparison was made.

## Cross-window extension

Recorded 2026-11-20 using `mise run smoke-live-rpc`, exit code 0. Debug executable
SHA-256: `66a6be8c6090e5681b21fdc57b51e1861640cf240192dabc70d268a70a425ee0`.

The fixture now creates a second window with activate=false, verifies its returned
ID and workspace/surface ownership, then targets a surface in each window and
checks their selected workspace IDs. Original terminal IDs remain present.
Malformed activation parameters return `-32602`. These and all earlier layout,
zoom, selection, stale-ID, notification and EOF checks passed. The fixture cleans
up its own two-window process. It does not observe OS focus, visual rendering,
window-creation latency or shell-start failure handling.

## Zoom state extension

Recorded 2026-11-20 using `mise run smoke-live-rpc`, exit code 0. Debug executable
SHA-256: `69b2aee06c21db688e2a820a42b26d1132f5f0bc0b0a2c0a3da601bfdb5269f3`.

Added native checks for explicit pane.zoom.set: required parameter validation,
missing pane errors, repeated zoom-in, clearing an unrelated pane leaving current
zoom untouched, surface focus moving zoom to a sibling, and unzoom preserving
the entire layout tree and all terminal IDs. All checks and the existing harness
checks passed. These inspect app state through RPC, not pixels or GPU timings.

## Multi-workspace extension

Recorded 2026-11-20 using `mise run smoke-live-rpc`, exit code 0. Debug executable
SHA-256: `f98f6245c022ab2c7341042e3f5a4c5212996a478b4d71d71d2123b59805f3c2`.

The fixture now restores two workspaces, three panes and four tabs through
`--rpc-stdio-live --layout`. It verifies split axis/weights and ownership IDs,
selects an initially inactive tab, another pane and the other workspace, then
checks selected states and unchanged terminal IDs. It also retains the stale-ID,
notification alignment and stdin-EOF checks below. All passed. Six startup parser
tests also passed, including live mode with layout and literal command arguments.

This is one native window. Cross-window changes, zoom, visual focus, moved tabs,
latency, running-shell cleanup and other platforms remain outside its coverage.

## Initial single-pane check

Recorded 2026-11-20 on the local macOS host, debug profile. Built using
`mise exec -- cargo build -p rmux-desktop`; ran
`mise exec -- python3 scripts/smoke-live-rpc.py`. Both exited successfully.
Executable SHA-256: `a087c1d995a08f3132f9d6d8288c907932709842fc4e4397d67596f50e89f2d5`.

The harness starts one native window with temporary default settings and
`/usr/bin/true` as its terminal command. It talks to the actual GPUI app through
stdin/stdout, then terminates only the process it created. No tools were installed.

Passed checks:

- Live capabilities and window/workspace/pane/surface listings.
- Parent IDs and split-tree pane ID agree with live terminal ownership.
- Workspace selection and surface focus return success.
- Unknown workspace/surface IDs return `-32004`.
- A focus notification produces no response and the next ping remains aligned.
- Closing stdin leaves the GUI process running.

This is a single-pane integration check, not a visual or performance measurement.
It does not prove cross-window switching, selection of a different tab, focus
appearance, zoom transitions, moved-tab targeting, process cleanup under running
shell workloads, or Linux/Windows runtime behavior. Use `mise run smoke-live-rpc`
to reproduce when a usable GUI session and Python 3 are already available.
