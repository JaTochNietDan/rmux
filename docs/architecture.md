# rmux cross platform architecture

Proposed design: a Rust application around **libghostty-vt**, with one shared desktop UI/rendering approach and small platform adapters. This preserves Ghostty's terminal processing while accepting responsibility for drawing, text integration, PTY ownership and application behavior.

Ghostty's full `ghostty.h` embedder API is internal and macOS-oriented. Its public VT library supplies terminal state and incremental render snapshots rather than a complete cross-platform GPU widget. A Rust frontend does not automatically inherit Ghostty's renderer or every cmux Ghostty patch. [Internal header](https://github.com/ghostty-org/ghostty/blob/main/include/ghostty.h), [VT rendering guide](https://lib.ghostty.org/vt/guide-rendering.html)

## Proposed components

| Component | Responsibility | Boundary |
| --- | --- | --- |
| rmux-core | Workspace/split model, stable IDs, commands and events | No GUI framework types |
| rmux-terminal | Safe Ghostty wrapper, terminal state and snapshot lifecycle | Minimal reviewed unsafe FFI |
| rmux-render | Text layout, glyph cache, terminal drawing and damage handling | Share graphics resources within a device where supported |
| rmux-desktop | Window shell, sidebar, tabs, palette and focus | GPUI is the first prototype candidate, not a settled dependency |
| rmux-platform | PTY/ConPTY, IME, clipboard, accessibility, menus and notifications | Explicit per-OS adapters |
| rmux-control | Typed RPC, command routing, event subscriptions and CLI | Bounded messages, caller scope, cancellation |
| rmux-session | Versioned snapshots, restore and resume policy | Layout restoration separate from process persistence |
| rmux-browser | Browser host and automation adapter | Lazy helper lifecycle and profile isolation |
| rmux-remote | SSH, tmux and other remote ownership adapters | Disconnect separate from termination |
| rmux-agents | Provider hooks, lifecycle, history, messaging and resume | Capabilities declared per provider |

These are logical boundaries; do not create a large crate hierarchy before the prototype demonstrates the need.

## Terminal data flow

PTY bytes enter the terminal engine on a bounded worker path. Visible surfaces request incremental snapshots. Rendering reads the snapshot after the terminal-state lock is released. User input goes directly through input encoding to the PTY writer, independent of sidebar metadata and transcript indexing. Output and input preserve ordering under backpressure.

Ghostty exposes a two-phase snapshot update so the terminal lock can be released before finishing snapshot work. Use that API as intended and profile lock duration. Do not assume incremental state updates imply incremental GPU presentation automatically. [Rendering guide](https://lib.ghostty.org/vt/guide-rendering.html)

The UI thread owns layout, focus and platform GUI operations. Background work sends compact metadata changes. Stable surface identities survive view movement; the visual pane is not the process owner. Hidden surfaces continue processing output but should not continuously submit frames.

## Browser decision required before P3

| Approach | Benefit | Cost or unresolved issue |
| --- | --- | --- |
| Native webview adapter per OS | Can reuse an OS web runtime where available | Engines and automation APIs differ; Linux distribution/runtime packaging and platform capabilities need validation |
| Bundled common browser engine | More consistent DOM/automation behavior | Larger distribution, helper memory and security/update responsibilities |
| External browser connection | Useful integration option | Does not satisfy embedded browser-pane layout parity |

Prototype terminal/browser composition, focus, clipping, drag overlays, resizing, screenshots, accessibility, profiles and remote proxying before selecting the backend. Browser automation method declarations must be tested against each actual engine. Browser processes count in product memory measurements.

## Experimental browser window

Browser → Toggle Web Inspector, Cmd+Option+I, and the `{}` toolbar control toggle
WRY's native inspector for the active browser window. The toolbar control is
unavailable before a page exists; menu invocation then gives a status hint. The
optional browser dependency enables WRY's devtools feature explicitly so this is
available in release builds with `browser`, too. Closing the host requests inspector
closure before dropping the webview. WRY 0.57.3 implements this on macOS through
private WebKit inspector selectors. Native inspector interaction, detached-window
cleanup, console/error capture and browser-pane routing remain unverified or open.

The optional macOS browser includes an explicit-submit Find bar. Cmd+F opens it;
Enter/Cmd+G finds the next match, Shift+Enter/Cmd+Shift+G finds the previous one.
Aa toggles case sensitivity and Escape closes the field and returns focus to the
page. Searches wrap through WebKit's native
[find API](https://developer.apple.com/documentation/webkit/wkwebview/findstring%3Awithconfiguration%3Acompletionhandler%3A?language=objc),
with no DOM copying or injected search script. One native request may be outstanding;
results share the bounded metadata wakeup and are ignored after query changes,
close, navigation or process failure. The bar reports Match found/No matches;
WebKit's result does not provide a total count. Native selection highlighting,
focus, keyboard dispatch and visual acceptance remain pending. Closing the bar
leaves WebKit's current text selection in place. Optional objc2 bindings reuse
versions already in WRY's dependency tree and stay outside default builds.

`mise run browser` enables the optional macOS `browser` feature in rmux. File →
New Browser Window (Experimental), also in the command palette, opens a separate
native window with an address field, Back/Forward/Reload/Go controls and a committed
URL/loading status line. The compact toolbar shares rmux’s 28-pixel controls and shortcut-aware tooltips,
with a bordered address field. Enter and the Go control submit through the same
path. The separate zoom percentage resets to actual size when clicked; truncated
status/URL text is available in a tooltip. Native tooltip overlay/clipping and
visual acceptance are pending. Enter submits the address. Bare localhost names (including `.localhost`) and IPv4/IPv6
loopback addresses default to HTTP, including explicit ports. Other bare hosts
default to HTTPS. Explicit HTTP/HTTPS schemes remain unchanged; private LAN
addresses require an explicit HTTP scheme if their server does not support HTTPS. The address field retains
the navigation draft; the status line reports the page-load URL after redirects.
Browser-context defaults are Cmd+L (select the loaded address), Cmd+R (reload),
Alt+Left/Right (history), and Cmd+=/Cmd+-/Cmd+0 (page zoom in/out/actual size).
Zoom uses discrete 25–500% steps per browser window, appears in the status line,
and also has Browser menu actions. A choice made before first navigation is
applied when the native view is created; native errors retain the previous setting. The native Browser menu routes the same actions to the
active browser window; other window types are unaffected by those menu actions.
Escape in the address field restores the loaded URL and returns native focus to
the page. Back/Forward buttons dim and no-op when history is unavailable. Reload
is disabled before the webview exists. Shortcut dispatch from a focused WKWebView
and native focus transfer remain unverified.
Terminal context menus include **Open Link in rmux Browser** in browser-enabled
builds. The action uses the frozen hyperlink target from menu creation and opens
an experimental window with that address. Native creation/navigation waits for
measured browser bounds. Unsupported, credentialed, control-containing and
overlong link targets are disabled; opening failures appear in the terminal's
existing error display. Ordinary Open Link and modifier-click retain their
external-browser behavior.
The webview is created on first navigation and uses an ephemeral profile. Closing
the window drops its owner. Page-load and document-title callbacks coalesce through one queued wakeup, retaining
the latest value of each independently. UI publication is limited to one batch per
100 ms during bursts, with no polling timer while idle. Unchanged metadata does
not request a redraw; the page itself still owns its native rendering workload. Native window titles include the page title
and rmux identity; page titles are capped at 256 Unicode characters with control
characters replaced by spaces. A new load clears the previous page title. macOS content-process termination
sets a visible recovery message, clears the stale title and gives the reload
control a recovery tooltip. Recovery requires a new load-start event; late completion
callbacks cannot clear the failure state. There is no automatic reload loop.
Native process termination/reload behavior still requires runtime acceptance.

The browser content now lives in its own GPUI entity; the standalone window wrapper
owns window title/close policy and subscribes to content metadata. This separates
view ownership from window policy for future pane integration. The current pane
model still stores terminal entities directly, so mixed surface tabs, focus routing,
native-view visibility/clipping and transfer/restore semantics remain to be implemented.

This remains experimental: native interaction is unverified, windows are not yet
workspace surfaces, and browser windows are omitted from session snapshots.
Only HTTP/HTTPS and about:blank navigation are allowed; embedded URL credentials,
external protocol launches, popups and downloads are unsupported and display a
concise status message when attempted. Page-initiated navigation uses the same
4096-byte/control-character/credential limits as terminal links; feedback does
not echo credentials. New navigation clears prior blocked-operation notices. Persistent
profiles, authentication workflows, permissions, error-page UX, native shortcut acceptance,
search-engine queries, split integration and automation remain work items. No
browser code or runtime is enabled in a default build. The macOS feature build and
focused URL policy test pass; that does not prove interactive browsing acceptance.

## Browser CI

The Experimental browser GitHub Actions workflow runs on relevant main-branch
pushes, pull requests and manual dispatch. It uses the pinned mise toolchain on
macOS, runs focused browser, row-cache and diagnostics tests, links the composition probe, and rejects WRY
in the default dependency tree. Compilation caches are separate from release
caches, active runs finish and only the latest pending run is retained, and no signing secrets or publish
permissions are involved. This is build/policy coverage, not interactive browser
or cross-platform acceptance.

## Native browser composition probe

`mise run browser-probe` builds and opens the optional macOS GPUI/WRY example.
The normal app has no WRY dependency enabled. The probe starts without a webview;
Create allocates an ephemeral WKWebView containing a local HTML input/click
fixture, Release drops it, and Change sidebar width moves/resizes its child-view
bounds. Window resizing also updates bounds in the GPUI layout pass. External
navigation is rejected in this fixture. Closing the window drops the host.

The macOS debug example builds successfully with pinned WRY 0.57.3. Default
`cargo tree` excludes WRY. Runtime composition, focus/IME, coordinate correctness,
helper-process reclamation, accessibility and concurrent-terminal performance
are **not verified**: the desktop was locked during implementation. This probe
is not a browser pane, an omnibar, a profile implementation or backend acceptance.

The next decision gate is to exercise this fixture alongside busy rmux terminals,
then embed a child in the actual split tree and test clipping, overlays and focus.
Record process memory before Create, after Create, after Release and after closing;
dropping the Rust handle alone does not prove that OS helpers release memory.
WRY's child-window API supports macOS/Windows/X11, but not Wayland directly;
its Linux GTK composition path requires separate investigation with GPUI. Do not
ship a macOS-only result as cross-platform browser parity. Windows validation
remains deferred per the user's instruction.
[WRY child-webview API](https://docs.rs/wry/0.57.3/wry/struct.WebViewBuilder.html#method.build_as_child)

## Cross platform obligations

| Area | macOS | Linux | Windows |
| --- | --- | --- | --- |
| Shell host | Unix PTY and shell integration | Unix PTY and shell integration | ConPTY plus PowerShell/cmd; WSL is an explicit separate mode |
| Window/input integration | Native menus, IME and accessibility | Wayland/X11, input methods and accessibility | Native IME, DPI and accessibility |
| IPC | Permission-restricted Unix socket | Permission-restricted Unix socket | Authenticated local IPC adapter, likely named pipes; retain shared RPC schema |
| Live local tmux/zellij ownership | Verify installed tool/version | Verify installed tool/version | WSL/remote adapter or a future native owner; do not label this native parity by default |
| SSH/Mosh/ET | Probe available binaries and behavior | Probe available binaries and behavior | Verify native/WSL availability per transport; no blanket support claim |
| Browser | Backend capability tests | Backend and packaging tests | Backend capability tests |
| Computer use | Separate OS permission helper | Desktop/session-specific automation adapter | Windows UI/input adapter |
| iOS Simulator | Local macOS-only adapter | Unavailable locally; possible explicit remote-host interface | Unavailable locally; possible explicit remote-host interface |
| Installer/update/policies | Platform signing and management | Distribution-specific packaging | Platform signing and management |

## Compatibility contracts

Support rmux's own names first. Define any cmux CLI/API compatibility layer explicitly, including aliases, identifier format, default target resolution, JSON/error schema, event semantics and focus behavior. Do not overwrite an installed cmux executable or claim existing cmux hooks work unchanged.

Ghostty configuration compatibility needs a per-key mapping. Fonts, colors and cursor settings are not equivalent to renderer shaders, platform effects or host action bindings. Maintain supported/translated/unsupported categories and actionable diagnostics.

Agent integrations need separate flags for launch, state reporting, notifications, session resume, history search, fork, Feed approvals, messaging and hibernation. Running an agent in a terminal establishes none of the richer capabilities. The pinned hook guide includes Claude Code, Codex, Grok, OpenCode, Pi, OMP, Campfire, Amp, Cursor CLI, Gemini, Kiro, Rovo Dev, Copilot, CodeBuddy, Factory, Qoder, Kimi, Muse and Antigravity, with additional Hermes references elsewhere. Resolve that discrepancy when building the adapter matrix. [Hook guide](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/agent-hooks.md)

## Upstream implementation lessons

cmux documents its own Ghostty fork changes: output ordering and backpressure, bounded mailbox turns, hidden renderer reclamation, teardown lifetimes, CJK shaping/fallback, resize presentation and graphics-state bounds. Use these as regression scenarios; do not assume upstream libghostty-vt contains identical host behavior. [Fork notes](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/docs/ghostty-fork.md)

The changelog also identifies expensive tab-strip rendering, blocked socket streams, duplicate restoration, stale remote sizes and agent-state races. A smaller Rust implementation must still handle those lifecycle cases. [Changelog](https://github.com/manaflow-ai/cmux/blob/a76816e4f0788003758b9434ab702068a640fb5d/CHANGELOG.md)
