# Trying rmux

For packaged previews, visit [GitHub Releases](https://github.com/rmuxapp/rmux/releases).
On Apple Silicon macOS, open the DMG and drag rmux.app into Applications. On Linux
x86-64, extract the tar.gz and run `./rmux` from its directory (Ubuntu 24.04-compatible
runtime libraries and a graphical session are required). On Windows x86-64, extract
the ZIP completely and run `rmux.exe` with `ghostty-vt.dll` alongside it. Windows
packages are unsigned; Linux/Windows native UI acceptance remains incomplete.
Only macOS currently includes Sparkle automatic updates.

The commands below are for building from source on macOS.

From the project directory, run `mise run open-app`. This builds the optimized
macOS bundle at `target/release/rmux.app` and opens it. You can also open that
bundle directly in Finder after it has been built. It is locally ad-hoc signed,
not notarized or installed into Applications. No new tools are installed by this
task. Quit an older running rmux before opening the refreshed bundle.

## Everyday controls on macOS

| Task | Shortcut |
| --- | --- |
| Command palette and workspace/terminal search | Cmd Shift P |
| New workspace / window | Cmd N / Cmd Shift N |
| New tab / searchable tab picker | Cmd T / Cmd Shift T |
| Split right / below | Cmd D / Cmd Shift D |
| Focus adjacent pane | Cmd Ctrl arrow |
| Zoom focused pane | Cmd Shift Enter |
| Next / previous workspace | Cmd Option Down / Up |
| Show/hide sidebar | Cmd B |
| Settings panel | Cmd , |
| Find terminal text | Cmd F |
| Next unread terminal | Cmd Shift U |

The palette can search terminal titles and cached working directories across
open windows. Workspace groups, tab transfers, notifications, layout/session
save and restore, settings reload and diagnostics are available through menus
or the palette. Grouping and transfers retain running shells. Restoring a saved
session starts fresh shells; it does not resurrect the old processes.

Configuration is optional at `~/.config/rmux/config.json` (or the XDG config
directory). No config file is created automatically. The Settings panel provides common
appearance controls and an explicit Save button; other options remain in the
JSON editor. Font defaults update terminals matching the previous defaults; custom values remain. The
palette's Open Settings File action provides access to all options.

## Current delivery state

This is an early terminal-workspace preview, not an accepted P0–P2 milestone.
There is substantial implementation beyond P1, but acceptance is separate.

| Area | Implemented | Remaining milestone work |
| --- | --- | --- |
| Terminal foundation | Ghostty state, GPUI rendering, Unix PTY and ConPTY code, incremental output | Cross-platform runtime, IME/DPI/Unicode coverage and input-latency gates |
| Workspace | Windows, sidebar/groups, tabs, nested splits, transfers, zoom, search and palette | Native gesture/focus acceptance, measured cmux visual comparison and accessibility |
| Recovery | Layout/session save/restore and opt-in crash checkpoints | Native dialogs/OS quit coverage and broader platform acceptance |
| Automation | Live stdio/Unix socket RPC, standalone rmuxctl, stable entity IDs and workspace/terminal operations | Caller scope, Windows transport, events and complete command semantics |
| Daily desktop | Config loading/reload, shell integration, unread UI and diagnostics | Full settings editor, key tables, native notifications, PR/port metadata and remaining shell integration |

Current ARM64 Linux desktop and CLI cross-checks pass. A focused session runtime
refresh passed 16 tests in the existing Linux container; this does not validate
the Linux desktop UI. See [runtime evidence](benchmarks/linux-arm64-session-tests-2026-11-20.txt).

Windows native validation is intentionally deferred. Native visual inspection on
the development Mac is currently unavailable while its session is locked. This
does not prevent builds or isolated RPC smoke fixtures, but those do not prove
visual quality or OS focus behavior. Browser, remote, agent and cloud phases
remain later parity obligations in the full roadmap.

## Focused release check

`mise run smoke-release` exercises the bundle's actual executable with temporary
configuration and owned shell fixtures. It checks live terminal operations and
cleanup. It does not inspect pixels or measure input-to-paint latency.

Next implementation batches should close the daily-workspace acceptance gaps
and extend the socket/CLI capability with command targeting and caller scope. No phase should be marked accepted solely because
the app builds or a narrow fixture passes.

## Automation from another terminal

The macOS bundle includes `Contents/MacOS/rmuxctl` beside the app executable.
It is not installed globally and does not change your PATH. To opt into socket
control, launch the app executable directly with `RMUX_SOCKET_DIR` set to a new
absolute directory whose parent exists. For example, with the release bundle:

```sh
RMUX_SOCKET_DIR="$TMPDIR/rmux-demo-$$" target/release/rmux.app/Contents/MacOS/rmux
```

The app logs the socket path. From another terminal, use that exact path:

```sh
target/release/rmux.app/Contents/MacOS/rmuxctl --socket /path/from/app/rpc.sock workspace.list
```

The client supports the current rmux JSON-RPC methods, not the complete cmux CLI.
`mise run smoke-bundle-socket` checks the debug bundle's app and client together
using temporary configuration and a separately owned test process.
