# rmux

**Merge Workspace into…** in the menu or command palette searches other
workspaces across this app’s windows, labeled by window. Enter moves the source's existing panes into
the destination, preserving both nested split trees with the source on the right,
then removes the source workspace entry. Destination labels, pin/group membership
and metadata remain; source workspace metadata is discarded. Pane zoom clears so
both trees are visible. Escape cancels. The custom shortcut action is
`merge_workspace`. Busy/stale destinations and excessive split depth reject before
transfer. Cross-window merges rebind terminal input/focus to the destination; an
empty source window closes. Native session/focus/group acceptance remains pending.

CLI: `rmuxctl merge-workspace --workspace SOURCE_ID --target-workspace DEST_ID`
uses the same guarded transfer path. It selects the destination inside the
app; add `--activate` to activate the OS window. Raw RPC is `workspace.merge`
with `workspace_id`, `target_workspace_id` and optional `activate`. Both IDs are
required; self-targets, unknown fields and stale targets fail without transfer.
Cross-window merges also reject destinations that would exceed 256 terminals. This is an rmux extension; no corresponding merge command appears in
the pinned cmux CLI/RPC inventory.

On macOS, **Copy** files in Finder and **Paste** into a focused terminal to insert
quoted absolute paths. Finder's file-URL metadata takes precedence over filename
text or image representations. This reads paths only, without opening files or
uploading them, and uses the same 256-file/64 KiB limits as file drops. Ordinary
text and image paste remain available when file metadata is absent. Linux
copied-file clipboard formats and native Finder paste acceptance remain pending.

**Settings → Terminal background opacity** adjusts the default terminal background
from 0–100% in 5% steps. **Blur behind terminal** requests platform blur below
100% opacity; availability depends on your OS/compositor. Text, explicitly colored
cells, tabs and sidebar remain opaque. Changes apply live across windows; use
**Save these settings** to retain them. JSON keys are `background_opacity` (integer
percentage, default 100) and `background_blur` (boolean, default false).
Native visual and compositor-performance acceptance remains pending.

On macOS/Linux, **Paste** also accepts clipboard images when no text representation
is present. rmux writes the original image bytes to a private temporary file in
the background, then pastes its quoted local path without Enter. Files remain
available until their terminal closes, including when the terminal moves between
windows. Limits are 16 MiB per image and 32 files / 64 MiB across the app. Closing
the owning terminal removes its files and frees the quota. If the terminal is no longer focused when
preparation finishes, delivery is canceled; a newer text paste or file drop cancels a
pending image paste. The **Preparing image paste…** badge exposes a cancel button;
**Cancel Queued Paste** also cancels preparation, and typing cancels it before
sending the new input. Automation mutations report busy during preparation.
Canceling delivery does not interrupt an in-flight disk write; its unused file
is removed when that bounded write finishes. This does not upload to remote hosts. Image files live under `~/Library/Caches/rmux/pasted-images` on macOS, or
`$XDG_CACHE_HOME/rmux/pasted-images` (default `~/.cache`) on Linux. A later image
paste scans at most 512 entries and removes unlocked rmux image files left after
a crash; active images stay locked. No idle cleanup timer runs. Older images in
the system temporary directory are not swept. Native clipboard acceptance and
forced-termination recovery testing remain pending.

On macOS/Linux, drop files into a **focused terminal** to insert their absolute
paths using POSIX shell quoting. A space follows each path; no Enter is sent.
Close terminal overlays first. Drops support at most 256 paths and 64 KiB of
quoted text; invalid UTF-8 or control characters are rejected. This inserts local
path text only—it does not upload files, read images or detect a remote shell.
Windows file-drop quoting is not implemented yet.

In **Listening Ports**, select a row and press **O**, or click its external-link
button, to open it as HTTP in your default browser. Enter still copies the raw
listener address. Wildcard bindings (`0.0.0.0` and `::`) open through IPv4/IPv6
loopback. This is an explicit HTTP action, not protocol detection; for HTTPS or
other protocols, copy the address instead. Scoped IPv6 addresses remain copy-only.
Results are snapshots: use **R** to refresh after a server restarts.

Choose **Move Workspace to Window…** from the menu or command palette to search
other windows by their active workspace name. The picker shows workspace counts
and whether a destination is busy. Enter moves the originating workspace and
activates the destination; Escape returns to the original focus. Closed or busy
destinations are checked again when selected. The custom shortcut action is
`move_workspace_window_picker`; the existing next-window command remains available.

`rmuxctl capture-pane --surface SURFACE_ID` returns visible text in a JSON
response. Add `--scrollback` for retained history, or `--lines 20` for the last
20 logical screen lines (which implies scrollback). Counts must be 1–65536.
Soft-wrapped rows are joined and unused trailing screen rows are omitted.
Interior blank lines and line breaks produced by explicit spaces are preserved. Output is capped at 1 MiB: check
`result.truncated` before treating it as complete. For an oversized tail, returned
text is the prefix of that requested tail range. Capture preserves selection,
focus and scroll position; a busy parser returns a retryable error. Explicit
surface IDs are required; implicit targets and workspace/window aliases remain
pending. Raw `surface.read_text` also accepts `lines` with `scope: "history"`.

`rmuxctl pipe-pane --surface SURFACE_ID --command 'wc -l'` captures retained
terminal history once and sends it to a local `/bin/sh` command as stdin. Quote
the command as one argument; a trailing quoted command is also accepted. The
command runs in the CLI's working directory and inherits its stdout/stderr.
Unlike other rmuxctl commands, this returns command output and its exit status,
not a JSON envelope. Capture is capped at 1 MiB; truncation or RPC failure prevents
execution. The CLI waits for the command, with ordinary shell interruption for
long-running consumers. There is no future-output stream or app-side subprocess.
Windows shell execution, implicit workspace/window selectors and JSON-wrapped
command output are not implemented.

A focused local macOS/Linux terminal shows **Input hidden** when canonical line
input has echo disabled. Raw-mode TUIs do not trigger this badge. It samples on
output, focus and input events, so silent mode changes may remain stale until
the next event. This is a local echo-state indication, not a guarantee that every
password prompt is recognized; remote/raw-mode prompts and Windows ConPTY
detection remain unsupported. No input content is inspected for this indicator.

**Enter Copy Mode** (palette/menu, custom action `copy_mode`) opens a stable
history snapshot while the terminal keeps running. Use ↑/↓ or j/k to move,
Home/End to reach either end, and Page Up/Down to move approximately one visible page. Hold Shift while
moving, or press Space/v to anchor a range. Click selects a line; Shift-click
extends. Enter/y or the platform Copy shortcut copies selected logical lines;
Select All selects the snapshot. A copy status reports the byte count; empty
selections leave the clipboard unchanged. Large selections are prepared in the
background; a newer copy elsewhere takes precedence over a pending copy. Closing
copy mode cancels pending clipboard delivery. The header shows the current line.
Escape closes it. Paste is disabled in this
view. The snapshot includes at most the first 8 MiB and 65,536 lines; truncation
is labeled. Long displayed lines are clipped at 4,096 bytes, but copying includes
their retained full text. Press / or Command-F (Control-F on Linux/Windows) to
search the snapshot. Enter submits a case-sensitive literal query; Shift-Enter
searches backward. n and Shift-N jump between matching lines with wraparound.
Search works on the full retained text, including clipped portions of long lines.
Escape in the search field returns to navigation. Press c to toggle character
selection; ←/→ or h/l also enter character mode. Shift-movement or Space/v anchors
a character range; Enter copies it. Navigation and copy preserve complete Unicode
graphemes, including combined emoji and accents. Vertical movement retains the
desired terminal-cell column across wide characters and short lines. Press c to return to line mode;
clicking a row or Select All also returns to line mode. Long-line display follows
the character cursor without shaping the entire line. Press r for rectangle mode,
then anchor with Space or Shift-movement and move across rows/columns. Rectangle
copy uses terminal-cell columns, includes whole graphemes touched by either edge,
and joins rows with newlines without padding short lines. r returns to character
mode; c returns to line mode. Named buffers can be managed through the CLI/API:

```sh
rmuxctl set-buffer --name notes 'text to retain'
printf 'first line\nsecond line\n' | rmuxctl set-buffer --name notes -
rmuxctl list-buffers
rmuxctl paste-buffer --surface SURFACE_ID --name notes --bracketed
rmuxctl buffer.get '{"name":"notes"}'
rmuxctl buffer.delete '{"name":"notes"}'
```

Omitting a name uses `default`. Omitted text or `-` reads UTF-8 stdin exactly,
including trailing newlines; an explicit empty string stores empty text. Stdin
reads are bounded, and JSON serialization must fit the request limit. Buffers are shared by windows in one rmux process,
independent of the clipboard, and disappear when the app exits. Limits are 32
buffers, 1 MiB per buffer and 8 MiB total text; over-limit writes fail without
eviction. The existing 64 KiB JSON request limit further constrains CLI writes
(including JSON escaping). Lists expose names and byte counts, not contents.
`buffer.get` and `buffer.delete` are rmux RPC extensions. `paste-buffer` requires an explicit surface ID and retains the buffer after
queueing. It preserves terminal focus and scroll position. Raw delivery is capped
at 64 KiB; `--bracketed` uses the terminal's paste encoding (including bracketed
markers when requested by the application), up to 1 MiB. Busy/exited terminals
and full queues report errors without automatic retry. RPC equivalent:
`buffer.paste` with `surface_id`, optional `name`, and optional `bracketed` boolean.
In copy mode, click **Save Buffer…** or press b. Enter a name and press Enter to
save or replace it; Escape cancels the name field. Line, character and rectangle
selections are supported, up to the 1 MiB buffer limit. Formatting happens in the
background without touching the clipboard. If any buffer changes during the
save, rmux reports a conflict and asks you to retry; it does not overwrite newer
state. Closing copy mode cancels pending save delivery.

After returning to a terminal, use **Paste Named Buffer…** in the command palette
or native menu (custom action `paste_buffer`). Search by name, select with arrows,
and press Enter to queue terminal-aware paste into the terminal that opened the
picker. Names and byte counts are shown without previewing contents. A buffer
change while the picker is open requires reopening it; a closed or moved target
is never replaced with an unrelated terminal. Paste preserves the viewport and
does not consume the saved buffer. **Delete Named Buffer…** in the palette/menu
(custom action `delete_buffer`) searches the same names and sizes; Enter deletes
the chosen buffer and releases its storage. If buffers changed while the picker
was open, reopen it before deleting.

**Choose Terminal Font…** searches installed font families for the current
terminal. Type to filter, use arrows and Enter to choose, or Escape to cancel.
Monospaced families suit the terminal grid best. The picker preserves text size;
**Use Configured Font** restores the configured family and size. The custom
action is `choose_font`. To change the default, use **Default font family** in
Settings or **Choose Default Font…** in the palette/menu (`choose_default_font`
for custom shortcuts). It updates terminals using the previous default family
and preserves per-terminal overrides and text size. Click **Save these settings**
to keep the default across launches.

Use **Show Listening Ports** in the palette or menu to inspect a snapshot of local
TCP listeners owned by the current terminal’s shell and descendants. Refresh
manually and copy an address from its row. In the panel, ↑/↓ or Home/End select,
Enter or Command-C (Control-C on Linux/Windows) copies, R refreshes, and Escape
closes and returns to the terminal. **Close Listening Ports** is also in
the palette; custom actions are `listening_ports` and `close_listening_ports`.
The panel does not discover remote-host ports or assume that a listener serves
HTTP. Results can change immediately as processes and sockets come and go. A
refresh notice appears if process ownership changed during the scan. Closing
the panel cancels remaining scan work between OS reads; an OS call already in
progress can finish first. If an immediate refresh reports another scan running,
retry after it releases the scan slot.

**Desktop notifications** in Settings enables optional OS banners for unfocused
terminals. Save settings to keep the choice (`"native_notifications": true` in
JSON); the default is off. **Show desktop banners** chooses between an unfocused
terminal and the entire app being in the background (`"notification_focus":
"unfocused_terminal"` or `"background_app"`). On macOS, use the bundled app and allow Notification
Center permission when prompted. Delivery failures appear in the terminal.
Banners are limited to one per second and a bounded queue; the in-app notification
history remains available independently. Turning the setting off skips queued
requests but does not recall a banner already submitted to the OS. On macOS and Linux, banner clicks route to the terminal’s current workspace/window unless
a modal interaction is active. Click listeners are capped at 16 and expire after
two minutes. Linux delivery depends on the desktop notification server supporting
actions; rmux requests banner expiry and bounds its own listener even when the
server ignores it. Native click behavior still needs validation. Windows click
routing, installer registration and platform acceptance remain pending.

In terminal Find, **Alt+Up** recalls older searches and **Alt+Down** moves toward
newer searches, then restores your draft. Enter (or Shift+Enter) remembers the
query while navigating matches; closing Find also remembers its current query.
History is local to each terminal and kept only in memory: up to eight distinct
queries and 16 KiB of retained query text, with a 4 KiB limit per query. Editing a
recalled query starts a new draft. History navigation leaves active IME composition
alone, and does not replace an oversized draft that cannot be searched.

Right-click terminal content for Copy, Paste, Find, selection, directory actions
and Clear Scrollback. Use Shift-right-click when a terminal app handles mouse
input. The palette’s **Terminal Actions** command opens the same menu; custom
shortcuts can bind `terminal_menu`. Arrow keys navigate, Enter activates, and
Escape returns focus to the terminal. Terminal, tab, workspace and group context menus show the current shortcut
beside each bound action, including custom key sequences. In short windows,
menus scroll and keyboard navigation brings the selected action into view.

Command-click (Control-click on Linux/Windows) opens terminal HTTP(S) links.
Hold that modifier while hovering to see the link target in a tooltip and a pointer
cursor for supported HTTP(S) links. Unsupported OSC 8 targets are labeled and do
not gain the pointer cursor. Hover lookup is cached per cell/revision and skips a
busy parser; move the pointer again to retry. Output changes clear old targets.
Link previews fit the window width and truncate long addresses with an ellipsis.
The terminal context menu also offers Copy Link Address and Open Link for the
link under the menu's opening position. It captures that target when opened, so
later terminal output cannot change the chosen address. Open Link is enabled
only for supported HTTP(S) targets; copying preserves the original address.
If a terminal program owns right-click, use Shift-right-click for the local menu.
If output parsing is busy, rmux reports a retry message instead of waiting; click
again after output pressure eases.

If Paste reports that the terminal is busy, no paste bytes were queued; retry
the action after output pressure eases. A successful retry clears the message.
Clipboard pastes retain the 1 MiB input limit and ordered bracketed delivery.
Accepted clipboard paste text is encoded on the PTY writer thread, keeping
subsequent keystrokes behind it. Asynchronous encoding/write failures appear as
terminal errors; accepted input is not a guarantee that the child consumed it.
A compact Paste pending overlay stays visible until the writer releases the
paste slot; its close control requests cancellation.
Cancel Queued Paste in the Edit menu/palette (custom action `cancel_queued_paste`)
discards paste payloads when cancellation is observed before writing starts.
It does not recall bytes already being written or interrupt their closing
bracketed-paste delimiter. The pending slot is released when the writer reaches
and discards the canceled message; ordinary queued keystrokes remain intact.

Closing the Terminal Memory panel releases its cached process samples. Reopening
starts a fresh CPU baseline, so refresh again after the baseline interval to see
CPU percentages. A sample already inside an OS query may finish before cleanup.

Workspace directory subtitles include the Git branch or detached HEAD when
available. Discovery runs in the background on shell directory reports. A shared
native filesystem watcher refreshes branch badges on HEAD changes, including
external checkouts, without running Git or polling while idle. Filesystems that
do not deliver native events still rely on shell reports or manual refresh.
PR and listening-port metadata are not yet implemented.
The palette's Refresh Git Branch for Current Terminal action refreshes explicitly.
Repeated shell directory reports refresh Git even when the path stays unchanged;
the included zsh integration reports its directory at each prompt. `surface.list`
exposes the cached value as `git_branch` (null when unavailable).

A working Rust/Ghostty terminal-workspace prototype, with a locally runnable
macOS release build. Start with [Trying rmux](trying-rmux.md) for launch,
essential shortcuts and the current milestone gaps. Run `mise run open-app`
from this project to build and open it. Full cmux parity remains in development.

Open Settings with Cmd+, on macOS (Ctrl+, elsewhere), the app menu or palette.
The side panel adjusts text-size defaults, terminal padding/theme, unfocused
cursor, copy-on-select and close confirmation. Choose Active tasks, Always or Never
for close confirmation. Active tasks also requests confirmation when terminal
state is busy, so closing does not wait for output parsing. An `RMUX_CONFIRM_CLOSE`
override remains authoritative
and is not saved into your config. Active tasks uses reported task status. Changes apply for the session; Save persists those
fields while preserving other configuration options. Font defaults update matching terminals live while retaining custom size/family
values. Reset Font Size returns the focused terminal to the size default. The editor link exposes the remaining settings. Tab/Shift-Tab or up/down navigate
the panel; left/right adjust, Enter/Space activate, and Escape closes it. Opening
the panel moves keyboard focus out of the shell; closing restores its prior focus
when the panel still owns focus.
Open Settings panels follow shared changes across windows. While a save, reload,
or settings-file open is running, Save shows “Working with settings…” and file
actions are temporarily unavailable. Controls remain editable; saving an older
snapshot reports when newer session changes still need saving.

Settings → **New workspace position** controls ungrouped workspace creation:
**First**, **After current**, or **Last** (the default). The JSON field is
`new_workspace_placement`, accepting `top`, `after_current`, or `end`. This applies
to the New Workspace action, configured commands, and RPC workspace creation.
Background RPC creation preserves the selected workspace even when indices move.
Pinned workspaces remain above unpinned ones; placement after a grouped origin is
after that group's sidebar block. If the origin is pinned, the new unpinned
workspace becomes the first row below the pinned section. Group members retain the separate New group
workspace setting. Generated empty-group anchors retain their existing append
behavior. Save these settings to persist your choice.

Custom palette commands can be configured in `config.json`:

```json
{
  "commands": [
    {"name": "Tests", "argv": ["mise", "exec", "--", "cargo", "test"], "cwd": "/absolute/project/path", "env": {"RUST_BACKTRACE": "1"}}
  ]
}
```

From a shell or launcher, use `rmux --command "Tests"` to start the configured
command in a new rmux window. This uses the same literal argv, child environment
overrides and configured `cwd`; without `cwd`, it inherits the launcher's current
directory. Names are exact and case-sensitive. This explicit launch bypasses
automatic session recovery and does not overwrite its saved file. Do not combine
`--command` with `--preset`, layout/session options, `-d`, or a program after `--`;
set the directory and arguments in the command definition instead.

After Reload Settings, choose **Run Tests** in the palette. For a list limited to
your saved commands, use **File → Run Configured Command…**, or assign the
`custom_command_picker` shortcut action. Search matches names and the displayed
directory/argument preview; environment values are omitted. Each invocation
starts a dedicated workspace with its own terminal; it does not inject text into
an existing shell. Arguments are literal, including spaces and empty arguments;
use an explicit shell executable only when shell interpretation is intended.
Without `cwd`, the selected workspace's remembered terminal directory is used
when available. Limits are 32 unique command names, 64 argv entries and 16 KiB
of argument text per command, within the config's overall 64 KiB limit. Optional
`env` overrides inherited/default variables for that child only; it supports up
to 64 portable variable names and 16 KiB of names/values, including empty values.
Only one custom command may be starting per window; repeat activation while it
is pending is rejected with a notice. Startup
runs off the UI thread; failures appear in the notice area. These definitions
are retained when saving appearance settings. Environment packs and the complete upstream command configuration contract
remain pending; shortcut overrides are described below.

Shortcut overrides can be added alongside custom commands:

```json
{
  "keybindings": [
    {"key": "ctrl-alt-y", "action": "settings", "context": "window"},
    {"key": "ctrl-k ctrl-t", "command": "Tests"},
    {"key": "ctrl-shift-p", "action": "disabled"}
  ]
}
```

The `Tests` command must exist in `commands`. Bindings default to `terminal`
context, so they do not override text editors; `window` makes a binding global.
The first stroke must use Ctrl/Cmd/Alt or F1–F24; one- and two-stroke bindings are
supported. The second stroke may be unmodified, for example
`{"key":"ctrl-k t","action":"new_tab"}`. Bare `t` remains normal typing.
Incomplete chords use GPUI's one-second timeout and replay handling; a leader
can therefore delay its ordinary terminal behavior. A small overlay shows the
pending keys while rmux waits for the next stroke; it does not take keyboard
focus or resize the terminal. The limit is 128 bindings. Duplicate normalized keys in the same
context, unknown key names/actions/commands and definitions with more than one
target fail config validation. A single stroke and a two-stroke chord cannot
share a prefix in the same custom context (for example `ctrl-k` and
`ctrl-k ctrl-t`); two different chords may share their first stroke. Error messages
identify the conflicting sequences. Function keys use F1–F24, without zero padding. Reload Settings replaces prior overrides and restores
removed bindings to defaults. Later overrides win within the same context;
a more specific terminal context takes precedence over a global binding.
`disabled` removes the matching app binding, allowing normal event handling.

A binding may specify `text` instead of `action` or `command` to send exact UTF-8
bytes to the focused terminal, for example:

```json
{"keybindings":[
  {"key":"ctrl-k g","text":"git status"},
  {"key":"ctrl-k u","text":"\u0015"}
]}
```

The first inserts `git status` without pressing Enter; the second sends Ctrl-U.
JSON `\r` explicitly sends Return, and `\u001b` sends Escape. These are raw input
bytes, not bracketed paste: no newline, escaping or shell expansion is added by
rmux. Text bindings require terminal context and 1–16,384 UTF-8 bytes. They do not
send while a search/copy/ports overlay or confirmation prompt is active. Existing
leader-key, override and reload rules apply. Ghostty's configuration syntax and
named key tables are not yet imported.

The palette shows shortcuts resolved for its originating focus context. Display
labels use macOS modifier symbols (for example ⇧⌘P) or Ctrl/Alt/Shift names on
other platforms. Pending chords use the same notation. Configuration still uses
GPUI strings such as `cmd-shift-p`; displayed labels are not configuration syntax.

Clipboard and lifecycle action names are `copy`, `paste`, `select_all`,
`clear_scrollback`, `close_tab`, `close_workspace`, `close_window`, and `quit`.
These invoke the same handlers as the menus, including close confirmations and
clipboard behavior. A `close_tab` binding closes the active tab, removing an empty
pane/workspace as usual. For example:

```json
{"keybindings":[
  {"key":"ctrl-k c","action":"copy"},
  {"key":"ctrl-k v","action":"paste"},
  {"key":"ctrl-k w","action":"close_tab"}
]}
```

Session/configuration actions include `save_layout`, `open_layout`, `save_session`, `open_session`,
`open_settings_file`, and `save_appearance`. These retain the existing dialogs and
save behavior. Diagnostics actions include `terminal_memory`,
`close_terminal_memory`, `copy_terminal_memory`, `cycle_terminal_memory_sort`,
`refresh_git_context`, `toggle_program_status`, and `dismiss_all_notifications`.
Use `"context":"window"` for a binding that should also work while a window panel
has focus; bindings otherwise require terminal context. No new defaults are added.

Supported action names: `new_workspace`, `new_window`, `new_tab`, `split_right`,
`split_down`, `split_left`, `split_up`, `focus_left`, `focus_right`, `focus_up`, `focus_down`,
`next_workspace`, `previous_workspace`, `last_workspace`, `last_terminal`,
`next_pane`, `previous_pane`, `next_tab`, `previous_tab`, `tab_picker`,
`move_tab_left`, `move_tab_right`, `move_workspace_up`, `move_workspace_down`,
`previous_prompt`, `next_prompt`, `last_prompt`, `scroll_to_top`, `scroll_to_bottom`,
`find_older`, `find_newer`, `increase_font_size`, `decrease_font_size`,
`reset_font_size`, `use_configured_font`,
`palette`, `toggle_zoom`, `toggle_sidebar`, `settings`, `notifications`,
`next_unread`, `mark_all_read`, `find`, `reload_settings`, and `disabled`. Graphical binding editing,
full conflict explanations and complete upstream shortcut coverage remain pending.
Numbered navigation is also configurable: `select_tab_1`…`select_tab_9`,
`select_pane_1`…`select_pane_9`, and `select_workspace_1`…`select_workspace_9`.
For example, `{"key":"ctrl-k 3","action":"select_tab_3"}` selects the third
tab in the focused pane. These dispatch the existing numbered commands and keep
their ordering and missing-target behavior; zero, leading zeros and numbers
above nine are rejected.

Layout and workspace editing also support these action names:
`equalize_panes`, `swap_pane_next`, `break_tab_right`, `break_tab_down`, `break_tab_left`, `break_tab_up`,
`join_pane_next`, `move_tab_next_workspace`, `detach_tab`, `detach_workspace`,
`move_workspace_next_window`, `move_workspace_window_picker`, `rename_tab`, `rename_workspace`,
`toggle_workspace_pin`, `edit_workspace_description`, `edit_workspace_icon`,
and `edit_workspace_color`. These reuse menu handlers, preserving live terminal
sessions during movement and existing split limits. They have no additional
default bindings; choose keys in your configuration. For example:

```json
{"keybindings":[
  {"key":"ctrl-k ctrl-e","action":"equalize_panes"},
  {"key":"ctrl-k ctrl-j","action":"join_pane_next"},
  {"key":"ctrl-k ctrl-d","action":"detach_tab"}
]}
```

Directional swaps are available as **Swap Pane Left/Right/Up/Down** in the View
menu and command palette, or through `swap_pane_left`, `swap_pane_right`,
`swap_pane_up`, and `swap_pane_down` custom bindings. They choose the neighboring
pane using the same geometry as directional focus, exchange entire panes
(including all tabs), and retain focus on the moved terminal. Split proportions
stay fixed. At a layout edge, or while zoomed, they do nothing. No default keys
are assigned.

**Join Pane Left/Right/Up/Down** moves all tabs from the focused pane into the
neighbor in that direction, then removes the empty split. Existing destination
tabs come first, followed by source tabs in their original order; the source's
active terminal stays selected. Other tabs retain their unread state. The
custom actions are `join_pane_left`, `join_pane_right`, `join_pane_up`, and
`join_pane_down`. Like directional swaps, these do nothing at a layout edge or
while zoomed, and have no default keybindings.

For example, bind `last_terminal` to return to the previous live terminal across
panes and workspaces, or `last_prompt` to jump to the newest retained shell prompt:

```json
{"keybindings":[
  {"key":"alt-backspace","action":"last_terminal"},
  {"key":"alt-home","action":"last_prompt"}
]}
```

These actions use the same handlers as the menus and palette. Terminal-specific
navigation needs terminal focus; keep the default `terminal` context for those
bindings. Reordering preserves the existing live sessions.

Workspace groups are available from the File menu and command palette: create
an empty group (Ctrl-Cmd-G on macOS, Ctrl-Alt-G elsewhere), group the current
workspace, rename, pin or ungroup. Headers represent the anchor terminal;
their chevron collapses members and `+` adds a workspace. New workspaces within
a group inherit the anchor's cached working directory by default and appear after the
active member. Closing the anchor promotes the next member. Groups are saved
with layouts and sessions. “Use Current Terminal Directory for Group” in the
palette fixes the starting directory for new members; “Reset Group Directory to
Follow Anchor” restores the default. Existing shells keep their directories.
Use the palette's “Move Workspace to Group” commands
to regroup existing workspaces. File-menu/palette actions can remove one member
or make it the anchor without restarting terminals. Multi-selection, group drag/drop, configurable
placement and group automation are still in progress. Group color and icon
editors are available in the File menu and palette; styling is saved independently
of the anchor workspace.
Group children support Move Workspace Up/Down and drag placement after another
group member. Dropping a child onto a standalone workspace removes it from its
group. Header drags and Move Workspace Up/Down reorder whole groups among
top-level rows in the same pinned/unpinned section. Native drag acceptance is
still pending.
Workspace number shortcuts and next/previous navigation follow group order.
Collapsed members keep their shortcut numbers; selecting one expands its group.
Group headers show workspace and unread-workspace counts, including hidden
members. Their notification preview shows the newest unread message across all
members, regardless of group order. Collapsing a group does not acknowledge its
notifications.
Close Workspace Group in the File menu/palette closes its terminals after one
confirmation for a multi-member group. Ungroup Workspaces keeps them running.
Cmd-click (macOS) or Ctrl-click toggles sidebar selections; Shift-click selects
a visible range. Group Selected Workspaces groups them in sidebar order with no
new shell. Its shortcut is Cmd-Shift-G on macOS, Ctrl-Alt-Shift-G elsewhere.
Settings → New group workspace chooses After current (default), First or Last
for where Cmd-N and the group header `+` insert new members. Save persists it;
changes apply immediately. The config equivalent is `group_new_workspace_placement`
with `after_current`, `top` or `end`.
The header button uses the anchor as its reference; `top` always inserts directly
below the anchor. Reload Settings applies changes to subsequent creations.
The File menu also includes the group directory capture/reset actions. Custom
shortcut action names include `new_group`, `group_selected`, `rename_group`,
`use_current_directory_for_group` and `reset_group_directory`.

Set `RMUX_SHELL` when launching rmux to choose the executable for new shell
terminals, including fresh shells restored from layouts. It takes precedence
over `SHELL` on Unix and `COMSPEC` on Windows. Supply one executable path/name,
not a command string; paths containing spaces remain literal. An empty override
is an error. Unix shells retain interactive login arguments (`-i -l`), and zsh
retains its existing integration. Use the startup `-- PROGRAM ARG…` form when
you need custom arguments. This override does not install or change your shell.

Optional startup configuration is read from `$XDG_CONFIG_HOME/rmux/config.json`
(or `~/.config/rmux/config.json`) on macOS/Linux, and
`%APPDATA%/rmux/config.json` on Windows. `RMUX_CONFIG` selects an explicit file.
No configuration file is created automatically. Example:

```json
{"font_size": 16, "confirm_close": "active"}
```

Open Settings File in the menu or palette opens that path with the system's
default application for JSON. If missing, this explicit action creates a default
file without overwriting an existing one. After editing, use Reload Settings.

`scrollback_mib` sets the retained-history budget per new terminal (0–256,
default 4). Zero disables scrollback. Reloading changes the default for new
terminals without trimming existing sessions. This is an engine retention budget,
not a cap on total process/GPU memory.
Settings exposes this as New terminal history, adjustable in 1 MiB steps.
Changes affect terminals started afterward; Save these settings persists the
budget for future launches.
In Settings, Page Up/Down moves through controls by the visible panel height;
Home/End jumps to the first control or editor link. The selected control scrolls
into view, including Save and Open all settings in shorter windows.

`terminal_padding` sets the inset around terminal content in logical pixels
(0–32, default 8). Reload Settings applies it to existing terminals and resizes
their grids. Find/status bars stay outside this inset. Set it to 0 for the prior
edge-to-edge layout; match padding when comparing benchmark window sizes.

`copy_on_select` (default `false`) copies a completed local mouse selection to
the system clipboard on release, including word/line and rectangular selections.
Reload Settings applies it to existing terminals. Application mouse reporting
keeps its normal behavior; hold Shift to select locally in those applications.

On macOS, Settings → **Option key behavior** switches between **Terminal Alt**
and **Native text** immediately; use Save these settings to keep your choice.
In JSON, `macos_option_as_alt` defaults to `true`. Set it to `false` to leave
Option-modified text and dead keys to native text composition, useful for
accented characters. Control+Option and non-text navigation keys retain terminal
encoding. Reload applies immediately. This setting has no effect on Linux or
Windows; native text commits follow the existing UTF-8/IME path.

`unfocused_cursor` is `"hollow"` by default, keeping the insertion position visible
in inactive panes and windows. Set it to `"hidden"` to suppress inactive cursors.
`focused_cursor` defaults to `"application"`, retaining application-selected
shapes. Choose `"block"`, `"bar"`, or `"underline"` to override the focused
cursor shape. Settings exposes both cursor controls, and Save these settings
persists them. Applications that hide their cursor remain hidden, and IME
composition uses a bar caret. Reload Settings applies these preferences to
existing windows; neither setting adds a blink timer.

`foreground_color`, `background_color` and `cursor_color` accept `"#RRGGBB"`
strings or `null` for the selected terminal theme's defaults. For example:

```json
{"foreground_color":"#dcdfe6","background_color":"#12141a","cursor_color":"#8cb4ff"}
```

Reload Settings updates existing terminals. Application OSC color overrides
take precedence until the application resets them. These settings affect the
terminal's default colors; sidebar styling and the ANSI palette are unchanged.

`ansi_palette` separately accepts an array of exactly 16 `"#RRGGBB"` strings:
black, red, green, yellow, blue, magenta, cyan, white, followed by their eight
bright variants. Reload Settings recolors existing ANSI-indexed output. Extended
indices 16–255 and true-color output retain their colors. Application OSC palette
overrides take precedence; setting `ansi_palette` to `null` restores the selected
terminal theme's default palette.

`terminal_theme` accepts `"dark"` (the existing default), `"light"` (white
background with a darker ANSI palette), `"solarized_dark"`, or `"solarized_light"`.
The Solarized presets use [Ethan Schoonover's published palette](https://ethanschoonover.com/solarized/).
Settings shows all four presets as direct choices, highlighting the selected one;
Left/Right still cycles when the theme control is selected. The palette offers
Terminal Theme commands for direct keyboard selection. Use Save these settings
to persist a session choice. It reloads live; explicit color and
palette values override the preset. This changes terminal content only, not the
sidebar or other application controls, and does not automatically follow OS
appearance changes.

Font sizes must be 8–40. Reset Font Size uses this configured default; new tabs
and splits inherit the source terminal's size and family, and saved layouts
retain their saved fonts.
Optional `font_family` selects an installed monospace font for terminals without
a source to inherit from; the default remains Menlo/Consolas/DejaVu Sans Mono by platform. Missing
fonts use GPUI's fallback stack. Existing terminals follow a changed family default only when their current family
matches the previous default; saved layouts preserve each terminal's family. Older layouts without a family
use the current configured default.
Use Reload Settings in the menu or command palette to apply file changes.
It keeps current settings if validation fails. Size and family values matching
the previous defaults update live; values differing from those defaults remain
unchanged. Matching is value-based, including fonts restored from layouts. `RMUX_CONFIRM_CLOSE` overrides the file's
`confirm_close` setting. Unknown keys and invalid values fail startup.
Use Configured Font in the View menu or palette applies both the configured
family and size to the focused terminal while preserving its running session.
Run `rmux --check-config` to validate settings without opening a window or
starting a process. It returns 0 on success or 2 with a diagnostic on failure.
`rmux --dump-config` prints the effective JSON preferences after defaults, the
configuration file and the close-policy environment override are applied, then
exits. Its output can be reused as a configuration file. It excludes internal
source paths and separate launch variables such as `RMUX_SHELL`.
`rmux --check-layout PATH` validates a saved layout's version, structure and
resource limits without loading preferences, opening a window or starting shells.
It returns 0 for a valid structure and 2 on error. Directory availability and
shell executability are checked when restoring, not by this structural check.

Close warnings use `RMUX_CONFIRM_CLOSE=active` by default (reported working/blocked
tasks, live directly launched commands, and Unix foreground jobs whose process
group differs from the shell). Set it to `always` to confirm closing any terminal-containing
tab/workspace/window, or `never` to disable these confirmations. The Quit rmux
action shares the policy. Set the variable before launching rmux; invalid values
fail startup. OS-originated quit routes, Windows foreground-job detection and
background jobs or commands that replace the shell remain pending.

A planned Rust terminal workspace application for macOS, Linux and Windows, using Ghostty terminal technology and a cmux-style workspace layout. The priorities are responsive input, low idle overhead, bounded memory and eventual feature parity.

This repository contains an early working macOS prototype plus the cross-platform roadmap. The prototype connects libghostty-vt 1.0.1, portable-pty and GPUI 0.2.2, renders terminal text directly through GPU elements, and supports workspaces, nested splits and per-pane tabs. Initial measurements and their limitations are recorded in `docs/benchmarks`; performance acceptance remains incomplete.

Move Workspace to Next Window in the menu or palette transfers the active
workspace into the next existing rmux window in the application's window list,
wrapping around. It retains live terminals, layout and workspace metadata;
pinned workspaces join the destination's pinned group. The destination becomes
active, and an emptied source window closes. If there is no other window, the
action shows a message; Move Workspace to New Window creates a destination.
Transfers into windows restoring a layout or waiting for window-close
confirmation are rejected before ownership changes.
The palette also lists `Move Workspace to Window N — workspace name` for each
other window, labelled with its active workspace. These entries retain the
source workspace identity and destination window handle; a closed destination
reports an error without removing the source workspace.

Development tooling is managed through mise. Never install tools with Homebrew. Installations outside mise require explicit approval; the Apple Metal Toolchain was approved and installed for this machine's GPUI build.

```sh
mise install
mise run bootstrap
mise run run
```

`mise run bundle` creates `target/debug/rmux.app` on macOS. `mise run test` runs workspace tests; `mise run check` checks the workspace. Cmd+D splits the focused pane right; Cmd+Shift+D splits below. Linux/Windows use Ctrl+Shift+D and Ctrl+Shift+E. Splits can nest in either direction. Cmd+V pastes and Cmd+Q quits.
Interactive split commands launch the shell and prepare its first frame in the background and show
“Starting split terminal…” while pending. Each workspace permits one pending
split. Completion splits the original pane; switching focus during launch keeps
your newer focus. Closing the original pane or transferring the workspace to
another window discards the pending result. Startup failure leaves existing
panes intact.
The split inherits the source tab's font at request time, even if you switch
tabs during startup. Inherited defaults follow settings changes made while it
starts; custom font choices are retained.
Use the pending indicator's cancel control, Cancel Split Terminal in the
File menu/palette, or a custom `cancel_split` binding to discard a pending split.
The indicator changes to Canceling until background work finishes; another split
can then start. Cancellation does not interrupt an OS spawn already in progress.

Clipboard text pastes accept up to 1 MiB of UTF-8 per operation and preserve
bracketed-paste boundaries. Each terminal accepts one pending paste at a time;
another attempt reports an error until delivery finishes. Ordinary input queued
after a paste follows its closing marker. A full or closed input queue reports
an error instead of accepting a partial paste.

Drag the sidebar's right edge to resize it; double-click the edge to restore its
200px default width. Save Layout includes the preferred sidebar width.
Double-click a workspace row to rename it. Hover over a truncated workspace name
to read it in full; the New Workspace button also shows its platform shortcut.
In the command palette, arrow keys select results and Page Up/Page Down jump eight
results at a time. Hover over a truncated command title to read it in full.

Launch with `rmux --working-directory PATH` (or `-d PATH`) to choose the default
directory for new shells. A focused terminal's reported directory takes precedence
for inherited tabs/splits. `--help` and `--version` exit without opening the UI;
invalid arguments or directories return exit code 2.
`rmux --rpc-stdio` provides standalone discovery without opening a window or
starting a shell. Send one JSON-RPC 2.0 object per line, for example
`{"jsonrpc":"2.0","id":1,"method":"system.capabilities"}`. Supported methods
are `system.ping` and `system.capabilities`; parameters must be absent or empty
(`{}` or `[]`). Responses use stdout, diagnostics stderr, and notifications
produce no response. Each request is limited to 64 KiB including its newline;
exceeding that limit closes the stream with exit code 2. Batch requests are not
supported. Capabilities explicitly report `live_workspace_access: false`: this
entry point does not connect to existing windows or claim cmux wire compatibility.

`rmux --rpc-stdio-live` launches the normal UI with automation on its
inherited stdin/stdout. It adds `window.list` and `workspace.list`, both with no
parameters, for all windows in that process. Window results contain IDs, workspace
counts and selected workspace IDs; workspace results include owning window ID,
zero-based sidebar index, metadata, selection, pin state, and pane/terminal counts.
IDs are opaque strings valid for that process lifetime, not saved-layout IDs.
Selected means the selected sidebar workspace, not OS keyboard focus. Queries
read cached UI state without terminal locks, scrollback extraction or focus changes.
Pipe IO runs on one worker; the UI request queue holds at most eight requests and
each reply waits at most five seconds. Error `-32000` indicates an unavailable/full
queue; `-32001` means the request expired or was canceled before execution.
If execution started but its reply is unavailable, `-32002` reports an unknown
outcome; inspect current state before retrying. Closing stdin
or breaking the stream detaches automation and leaves the windows running.
This launch mode can combine with `--layout PATH`, or with directory and startup
command options. Inspection flags remain incompatible. Stdio alone opens no socket
and does not attach to an existing process. Additional targeting, subscriptions
and cmux compatibility remain pending.
The native GUI-to-pipe path passed a macOS smoke check covering restored layouts,
zoom and target selection across two windows. Visual focus, OS activation and
other-platform acceptance remain pending. Reproduce with
`mise run smoke-live-rpc` on Unix with a usable GUI session.
`mise run smoke-live-sessions` additionally checks running shell state through tab,
workspace and workspace-merge transfers, including closure of emptied windows.

For opt-in Unix socket automation, launch with
`RMUX_SOCKET_DIR=/absolute/new-directory mise run run`. The parent must exist;
the endpoint directory must not. Connect to `rpc.sock` inside it using the same
newline-delimited JSON-RPC protocol and live methods. This also works alongside
`--rpc-stdio-live`; clients share the bounded UI dispatcher. The directory is
mode 0700 and socket 0600. Same-user clients can control the app. At most eight
connections are served, with ten-second read and five-second write timeouts;
idle clients should reconnect. No idle polling or network listener is added.
Normal quit removes the endpoint if its identity still matches. After a crash,
choose a new directory or manually inspect/remove the stale one. Startup failures
are logged and leave the GUI running without the socket. Windows named pipes and
caller-scoped permissions remain pending. `mise run smoke-socket` checks two live
clients, shared mutations, reconnection and normal-quit cleanup on macOS/Unix.

Build the lightweight external client with `mise exec -- cargo build -p rmux-cli`.
`rmuxctl help COMMAND` or `rmuxctl COMMAND --help` shows command-specific usage
without a socket or a running app. A leading `--socket PATH` is also accepted for
help without opening that path. `rmuxctl --version` prints the client build version.
Help flags are recognized in these dedicated forms; text after `--` remains literal
input for commands such as `send`.

`rmuxctl completions bash|zsh|fish` prints shell setup without requiring a running
app or `RMUX_SOCKET_DIR`. With `rmuxctl` on your PATH, enable it for the current shell:

```sh
# Bash
source <(rmuxctl completions bash)
# Zsh (after compinit in your shell setup)
source <(rmuxctl completions zsh)
# Fish
rmuxctl completions fish | source
```

The scripts complete supported friendly command names, command-specific flags,
and `--focus`/`--zoomed` boolean values. They do not contact rmux, enumerate live IDs, or
complete raw RPC JSON. Save the generated script in your shell's startup setup
if you want it enabled in later sessions. Bash and Zsh wrapper handling has been
checked locally; Fish runtime verification remains pending.

The macOS bundle also includes it at `Contents/MacOS/rmuxctl`, without a global
install or PATH change. It has no desktop renderer or terminal-engine dependency:

```sh
target/debug/rmuxctl --socket /absolute/new-directory/rpc.sock workspace.list
target/debug/rmuxctl --socket /absolute/new-directory/rpc.sock workspace.rename '{"workspace_id":"ID_FROM_LIST","name":"Build"}'
```

Alternatively, export `RMUX_SOCKET_DIR` and omit `--socket`. The client prints the
complete JSON-RPC reply; exit status is 0 for success, 1 for an RPC error and 2
for usage/transport failure. It never retries mutations automatically because a
lost reply can have an unknown outcome. Requests are capped at 64 KiB and replies
at 8 MiB. Alongside raw method calls, the client supports `list-windows`, `list-workspaces`,
`list-panes`, `list-panels`, `select-workspace --workspace ID`,
`rename-workspace --workspace ID NAME`, `focus-panel --panel ID`,
`send --surface ID [--paste] TEXT`, and `read-screen --surface ID` (optionally `--scrollback`
or `--selection`). Select/focus accept `--activate` to request OS activation;
otherwise they change the target selection without requesting OS activation.
Use `--` before positional text starting with a dash. All output remains JSON.
Lists cover the whole process by default. Narrow workspace/pane/panel lists with
`--window ID`; pane/panel lists also accept `--workspace ID`. Combined filters
intersect, and a well-formed missing or mismatched target produces an empty list.
Malformed IDs and unsupported filters are errors. Filtering does not focus or
create anything. Ref/index resolution, implicit caller targets and the rest of
the cmux CLI remain pending. Unsupported flags
are rejected rather than ignored.

Use `split-pane` to create a new terminal beside an explicit pane:

```sh
rmuxctl split-pane --pane PANE_ID --direction right --focus false
# Equivalent raw RPC:
rmuxctl pane.split '{"pane_id":"PANE_ID","direction":"right","focus":false}'
```

Use `left` or `right` for a side-by-side split, and `up` or `down` for a stacked
split. The new pane appears on the requested side of the target. Optional `--cwd` (`cwd` in RPC)
must be absolute; otherwise the new terminal inherits the target pane's cached
working directory. Its font settings follow that pane too. The reply contains
`pane_id`, `surface_id`, and `workspace_id` for subsequent layout operations.
Focus stays unchanged by default; `focus:true` selects the new terminal and
`activate:true` additionally activates its window. Successful splitting exits
pane zoom. A failed shell start preserves the existing split tree and zoom.
RPC creation is limited to 64 panes per workspace and respects busy-window guards.
Interactive and RPC split creation, including Break Tab, stop at 32 nested splits, matching saved-layout
validation; shallower sibling panes remain splittable.

Live layout commands use explicit IDs from the list commands:

```sh
rmuxctl move-panel --panel SURFACE_ID --workspace DESTINATION_WORKSPACE_ID
rmuxctl move-workspace --workspace WORKSPACE_ID --window DESTINATION_WINDOW_ID
rmuxctl set-pane-zoom --pane PANE_ID --zoomed true
```

Moves reuse the existing transfer operations and preserve running terminal
processes. They select the destination; pass `--activate` to additionally request
OS window activation. Zoom accepts an explicit `true` or `false`; enabling it
selects that pane's terminal within its window without requesting OS activation.
Busy, missing and invalid destinations retain the RPC error behavior. These are
rmux command forms, not a claim of complete cmux CLI compatibility.

`send-key --surface ID KEY` sends one symbolic press/release tap, for example
`Enter`, `Escape`, `Up`, `F5`, `Ctrl-c`, `Alt-Left` or `Shift-Tab`. The CLI also
accepts C-/M-/S- modifier prefixes, Return/Esc/BSpace aliases, and single Unicode
characters. It uses the terminal's negotiated keyboard encoding, not hard-coded
escape sequences, and preserves focus/viewport. Raw `surface.send_key` takes
`surface_id`, a lowercase named `key` (or a literal character), and optional
`control`, `alt`, `shift` booleans. A busy parser returns `-32007` before queueing;
other input failures return `-32006`. Multi-key sequences, held keys and --force
semantics remain pending.

`send --paste` sets `paste:true` on `surface.send_text`. It uses Ghostty's current
bracketed-paste mode, preserves viewport/focus, and queues the complete encoded
payload before later input. Both raw and paste automation text are capped at
16 KiB UTF-8. A busy parser returns `-32007` without queueing input; an exited
terminal, pending paste or unavailable input queue returns `-32006`. No implicit
newline is appended. Without `--paste`, input retains the existing raw behavior.

The notification panel lists messages newest-first across all workspaces in the
window, using arrival order rather than pane order. Keyboard selection remains
attached to the same message as newer messages arrive.

Use Mark read on a card, or R for the selected notification, to acknowledge only
that message without opening its terminal. It remains in All history. Other
messages and bell/task-status alerts are unaffected.

Notification previews show up to two title lines and four body lines. More/Less
expands one message at a time; press E to toggle the selected message. Expansion
does not mark it read.

Each notification card has a Copy button for its complete title and body. With the panel
focused, Command-C on macOS or Control-C on Linux/Windows copies the selected
message. Copying preserves unread state and does not open the source terminal.

The notification panel owns keyboard focus while open. Up/down, Page Up/Down,
and Tab/Shift-Tab navigate messages. Page keys use the visible panel height and
measured card sizes, advancing at least one message. Enter/Space closes the panel and opens the
selected terminal. Delete/Backspace dismisses the selected message, and Escape
closes the panel and restores its prior focus. Selection tracks terminal and message revision
as messages arrive; the panel and Settings are mutually exclusive.

`notify --surface ID --title TITLE --body BODY` posts into rmux's existing
notification panel and unread indicators without changing focus. Each surface
retains up to 32 notifications, newest first within each terminal.
`dismiss-notification --surface ID --revision NUMBER` removes one message using
the exact identifiers returned by `list-notifications`. It returns
`dismissed:false` for an unknown, evicted, or already dismissed revision; it does
not clear newer messages or switch focus. Revisions are unsigned 64-bit integers.

`list-notifications` lists non-dismissed messages
and accepts optional `--window`/`--workspace` filters. These are in-app messages;
native OS notification delivery is not implemented yet. The raw methods are
`notification.send`, `notification.list`, and `notification.dismiss`; dismissal
requires `surface_id` and the returned `revision`, and returns `dismissed:false`
if the message was dismissed, evicted by the history limit, or is unknown. Shell OSC notifications and RPC messages
share UI revisions while tracking shell updates independently. Titles/bodies are
bounded and control characters removed. The full cmux notify/reply/clear contract
is still pending. `surface.list` also includes its latest non-dismissed message
as `notification` (or null).

`close-workspace --workspace ID` calls `workspace.close`. It removes the targeted
workspace and shuts down its owned terminals through the usual lifecycle; closing
the last workspace also closes its window. Closing a background workspace preserves
the current selection/focus. If the configured close policy requires confirmation,
the API returns `-32003` and leaves the workspace untouched; complete the close in
the UI. No force override is exposed. The result reports `closed` and
`window_closed`; process reaping finishes asynchronously.

`new-window [--activate]` creates a window. `new-workspace --window ID` creates
one terminal workspace in an existing window, optionally with `--name NAME`,
`--description TEXT`, `--cwd ABSOLUTE_PATH`, `--focus true|false`, and `--activate`.
Workspace creation defaults to keeping the current workspace selected;
`--activate` implies selection and requests OS activation. New workspaces are
ungrouped; group creation has its own RPC methods. Command/env/layout overrides
for creation remain pending. Raw `workspace.create` accepts the same fields
using `window_id`, `name`, `description`, `cwd`, `focus`, and `activate`.
Invalid cwd/shell startup leaves no empty workspace behind. Windows are limited
to 64 workspaces through this API; busy windows reject creation for retry later.

`workspace.list` accepts an optional `window_id`. `pane.list` and `surface.list`
accept optional `window_id` and `workspace_id` filters; with no parameters they
list all panes and terminal tabs across that process. Both include parent window/workspace IDs;
surfaces also include pane ID, tab index, effective title, selected/attention flags
and a cached directory label (not a canonical path). Pane results include selected
surface ID, surface count and zoom state. Pane indices describe internal ordering,
not spatial position. `workspace.list` includes `layout`, a nested tree of
`pane` nodes with pane IDs and `split` nodes with `rows`/`columns`, child order and
relative weights, plus `zoomed_pane_id`. The tree retains hidden panes during zoom.
Use this tree for spatial layout and IDs for identity; tab order and indices may
change. Cached labels can be stale while terminal metadata updates are pending.
Surface results also contain `process_id` (the original PTY child PID, nullable)
and `process_exited` (whether reaping has completed). These avoid terminal locks;
a retained PID is not a liveness guarantee and may be reused after exit.

`window.list` includes `groups` with window-scoped handles such as
`workspace_group:17`, `window_id`, name, icon/color, pin/collapse flags,
`generated_anchor`, `anchor_workspace_id` and ordered `workspace_ids`.
Use the group handle together with its window ID; handles can repeat in another
window. `workspace.list` adds nullable `group_id` and boolean `group_anchor`.
Its `index` follows logical sidebar order, including collapsed members.
`workspace.group.update` requires `window_id`, `group_id` and at least one of
`name`, `collapsed`, `pinned`, absolute `cwd` or `clear_cwd: true`; it returns the
updated group record. `cwd` and `clear_cwd: true` cannot be combined. Clearing the
override restores inheritance from the anchor. Group records expose nullable `cwd`.
`workspace.group.ungroup` accepts only those two IDs and removes the container,
returning its `workspace_ids` without closing terminals. Both respect busy-state
guards and support notifications. `workspace.group.add`, `workspace.group.remove`
and `workspace.group.set_anchor` require `window_id`, `group_id`, `workspace_id`.
The workspace must belong to that window; remove/set-anchor also require current
membership. Add appends a member and is idempotent within the same group. The
response includes `group` (null if removing the final member dissolved it).
These methods preserve terminals and selection.
`workspace.group.create` requires `window_id` and `name`. Optional `workspace_ids`
must be a nonempty, unique list from that window; the first becomes the anchor.
Omitting it starts and selects a fresh anchor using the active workspace's cached
directory. Optional `external_id` is a persistent, window-scoped retry key:
repeating it returns the existing group without changing it or starting a shell.
Creating from existing workspaces preserves selection. Optional `cwd` supplies
an absolute directory for a fresh anchor and stores a group override for future
members. It can also be set when grouping existing workspaces; their shells are
unchanged. The override survives layout/session save and restore.
`workspace.group.ungroup` optionally accepts `remove_generated_anchor: true`.
This closes only a sole anchor explicitly created by rmux; it rejects groups with
other members and user-selected/promoted anchors. Close-policy confirmation
requirements return `-32003` so the user can close the workspace in the UI.
Successful cleanup returns `closed_workspace_id`; ordinary ungroup closes nothing.
`workspace.group.new_workspace` accepts `window_id`, `group_id` and optional
`placement` (`after_current`, `top`, `end`). It creates/selects a fresh workspace
using the anchor's cached directory and returns its `workspace_id`. The anchor
is the placement reference, so `after_current` inserts directly below it. Without
an override, the configured `group_new_workspace_placement` is used.

Live mode also supports `workspace.select` with an ID from `workspace.list`:
`{"jsonrpc":"2.0","id":2,"method":"workspace.select","params":{"workspace_id":"ID","activate":false}}`.
It uses sidebar selection behavior, including remembering the previous workspace
and restoring pane focus inside that window. `activate` defaults to false; true
also requests OS window activation. No index fallback is used for stale IDs.
Unknown workspaces return `-32004`; windows with active editors, palettes, layout
restoration or close prompts return `-32003` instead of interrupting that work.
Queued requests canceled at the deadline cannot execute later. Selection
notifications execute without a response; use a request ID to receive confirmation.

`surface.focus` accepts `{"surface_id":"ID","activate":false}` using an ID from
`surface.list`. It selects that terminal's workspace, pane and tab through the
existing terminal-navigation path. If the workspace is zoomed, zoom follows the
target pane. OS activation remains opt-in. The same deadline and busy-window
errors apply, including active tab renames and close prompts. A removed surface
returns `-32004` without selecting another tab. Success returns `surface_id`,
`workspace_id` and `selected: true`; visual focus/zoom acceptance is still pending.

`surface.send_text` takes `{"surface_id":"ID","text":"text"}` and queues the exact
UTF-8 bytes to that PTY. Text must be nonempty and at most 16 KiB after UTF-8
encoding. No newline or bracketed-paste envelope is added; include line endings
explicitly when wanted. Workspace/tab selection and the scrollback viewport are
preserved. `queued_bytes` confirms queue acceptance, not application execution or
completed delivery. An exited process or unavailable writer queue returns
`-32006`; invalid parameters return `-32602`. Existing target, busy and deadline
rules apply. The native live-shell test verifies Unicode delivery and retained
shell variable values after workspace transfer.

`surface.read_text` takes `{"surface_id":"ID","max_bytes":65536}`. It reads the
current viewport, including a scrolled-back viewport, from a fresh terminal
snapshot without changing selection or scroll position. Visual rows are separated
by newlines (including soft wraps), and row-end whitespace is trimmed. The optional
byte limit defaults to 64 KiB and ranges from 1 byte to 1 MiB; output ends at a valid
UTF-8 boundary with an explicit `truncated` flag. Responses include columns, rows,
scrollback offset and `scope: "viewport"` (the default).
If parsing owns the terminal lock, error `-32007` asks the caller to retry instead
of blocking the UI waiting for that lock. Snapshot extraction itself still has a
cost; sustained-read performance has not been benchmarked.

Use `scope: "selection"` with `surface.read_text` to read only selected text.
It uses the same bounded background export and UTF-8-safe truncation as history;
no selection returns empty text with `truncated: false`. Add `preserve_wraps: true`
to selection-scope requests, or use `read-screen --surface ID --selection
--preserve-wraps`, to retain terminal soft-wrap line breaks. The default joins
soft-wrapped lines. `preserve_wraps: true` is rejected for other scopes. It does
not change the clipboard, selection or viewport.

`surface.memory_usage` accepts `{"surface_id":"ID"}` and samples Ghostty screen
storage on demand in the background. It reports primary/alternate resident,
virtual, compressed and image bytes plus compression support. Its scope is
`ghostty_screen_storage`: these values exclude Rust snapshots, fonts, GPU assets
and child-process memory, and are not total application RSS. A busy parser returns
`-32007` for retry. No periodic sampler runs when the endpoint is unused.
The View menu and command palette also offer Show / Refresh Terminal Memory.
Opening or refreshing focuses the panel. Use ↑/↓, Home/End to select a row,
Enter to focus its terminal, R to refresh, S to cycle sorting, C to copy the JSON
report, and Esc to close. Selection follows the terminal through sorting and
refreshes while it remains in the sample; Esc restores prior focus when the panel
still owns focus. Rows are virtualized for larger terminal lists.
Its panel samples up to 256 terminals in the current window and provides manual
Refresh/Close controls, including explicit busy or unavailable rows.
Rows initially sort by resident storage; use the sort control or command palette's
Cycle Terminal Memory Sort to switch to shell or foreground RSS/CPU, descending with missing
samples last. Sorting reuses existing samples without another process scan.
Click a row to focus its terminal. Copy exports the last sample as JSON in the
displayed order, with raw byte counts and explicit unavailable values.
The panel also samples each terminal's root shell RSS and CPU on the background
worker. CPU requires two sufficiently spaced refreshes with the same PID set;
the first sample is unavailable rather than zero. Exited processes are omitted.
On Unix, a distinct foreground process-group leader is also sampled and included
under `foreground_group_leader` in copied JSON. Results are discarded if that
terminal's foreground group changes before sampling completes. Windows does not
yet expose this information. This measures the leader alone; a pipeline or compiler
can have additional workers whose memory is not included.
These process figures exclude descendants and helpers, and are separate from
Ghostty screen storage. Sampling is manual, with no idle polling timer.
The panel's rmux summary reports application-process RSS and CPU across all
windows, also included under `application` in copied JSON. RSS is the operating
system's resident process measure, not a GPU allocation total. Screen storage is
already part of the application's memory: do not add it to application RSS.

Use `{"surface_id":"ID","scope":"history","max_bytes":65536}` to export
the active screen's retained history oldest-first, unwrapping soft-wrapped lines.
The same byte limits apply; responses contain surface_id, scope, text and
truncated. Export runs on a background executor, without changing selection,
focus or scroll position. It returns `-32007` when the engine is busy or the
terminal closes before export. Formatting still holds the engine lock while
reading, so large exports can contend with rendering and parsing; sustained
history-read latency is not yet benchmarked. Alternate-screen exports contain
only that screen, not the main screen's history.

`surface.move_to_workspace` takes `{"surface_id":"ID","workspace_id":"ID","activate":false}`.
It moves the existing terminal into the destination workspace's remembered pane,
including across windows, and selects the transferred tab. Activation is optional
and defaults to false. Moving to the current workspace is a no-op. Stale IDs return
`-32004`; busy editing/restore/close states return `-32003`. Successful responses
contain the surface and destination workspace IDs. This shares the palette's
cross-window transfer path and preserves the PTY and custom tab label.

The rmux-specific `pane.zoom.set` method takes `{"pane_id":"ID","zoomed":true}`.
Setting true selects that pane's workspace and active terminal, then zooms it
without OS window activation. Setting false clears zoom only if that pane is
currently zoomed; it does not clear another pane's zoom or change selection.
Repeated requests are idempotent. Layout weights and terminal identities survive
zoom/unzoom. Missing IDs return `-32004`, malformed parameters `-32602`, and busy
windows `-32003`. Native state-transition checks pass; pixel-level zoom appearance
and latency remain unmeasured. This method is not a cmux wire-compatibility claim.

`window.create` accepts an optional `{"activate":false}` object and returns the
new window's ID, selected workspace ID and initial workspace metadata. It uses
the normal default-shell window creation path. Activation defaults to false;
true requests window focus and app activation. A close prompt or shutdown returns
`-32003`; native window creation failure returns `-32005`. Each successful call
creates a new window, so inspect `window.list` after an unknown-outcome error
before retrying. Creation does not guarantee that the configured shell started
successfully; the window retains the normal terminal error display if it fails.

`workspace.rename` takes `{"workspace_id":"ID","name":"New name"}` and returns
the ID and effective name. It shares sidebar editor normalization: trim whitespace,
remove control characters, keep at most 128 Unicode characters, then trim again.
An empty result returns `-32602`. Renaming does not select the workspace or request
OS activation; an active workspace also updates its window title. Existing
busy-window, stale-ID and deadline rules apply. The native smoke verifies Unicode
normalization and renaming an inactive workspace without changing selection.

`workspace.move_to_window` takes `{"workspace_id":"ID","window_id":"DESTINATION","activate":false}`.
It moves the existing workspace and terminals, selects it in the destination,
and removes an empty source window. Names, metadata, split layout and terminal
IDs are retained. The destination ID must come from `window.list`. Activation
defaults off; repeated requests targeting the current owner are no-ops. Missing
targets return `-32004`; busy source/destination interactions return `-32003`.
The native smoke verifies layout/ID retention and empty-source cleanup.
`mise run smoke-live-sessions` additionally verifies five live fixture-shell PIDs
survive both transfers and exit during harness cleanup. Arbitrary running-shell
state, OS focus and performance still need dedicated acceptance.

File → Save Session… (also in the command palette) captures all normal rmux
windows into one file. Capture refuses while a window is restoring, editing or
waiting for a close decision. The selected path is written atomically on a
background executor. This saves the state when the action was invoked.
File → Open Session in New Windows… imports that file into the running app,
leaving existing windows and sessions intact. It prepares fresh shells off the UI
thread, then opens the saved windows and activates the saved selection. No running
process or command history is resumed. All saved working directories must exist;
empty session files are rejected by this command. Closing the initiating window
cancels pending preparation. If native window creation fails partway through,
windows opened by this import are removed; existing windows remain. The command
is also available in the palette and as the `open_session` custom shortcut action.
Layout and session imports show the terminal preparation count. Cancel using the
status button, Cancel Layout / Session Restore in the palette, or a `cancel_restore`
custom shortcut. Cancellation stops at the next preparation boundary and cleans
up shells already prepared; it cannot interrupt a shell-spawn system call.


The repository includes [development and services layout examples](../examples/layouts/README.md)
with customization and preset setup instructions.

For repeatable workspace setups, save a layout or session once and add named
presets to your settings. Replace these example paths with your saved files:

```json
{
  "layout_presets": [
    {"name":"Development", "path":"/absolute/path/development.json", "cwd":"/absolute/project"},
    {"name":"All Projects", "path":"/absolute/path/projects.json", "session":true}
  ],
  "keybindings": [{"key":"ctrl-k d", "preset":"Development"}]
}
```

After Reload Settings, choose **Open Preset: Development** in the palette or use
its shortcut. You can also launch `rmux --preset "Development"` from a shell.
File → Open Layout Preset… opens a dedicated searchable picker. It shows the
layout/session type, optional project directory and source file, and includes
configured shortcut hints. Search matches those details as well as the name.
The `layout_preset_picker` shortcut action opens the same picker. If none are
configured, it explains how to add presets and reload settings.

The name must match configuration exactly. `--rpc-stdio-live` may be combined with
`--preset`; explicit files, `-d`, commands and config inspection cannot. Set the
preset's `cwd` in settings instead. Like other explicit restores, a preset launch
neither loads nor overwrites the normal automatic recovery file. Each invocation opens fresh windows/shells in the directories saved
in the file; it does not replay prior terminal commands or resume running jobs.
The file is read when invoked, so edits apply to the next use. Layout presets
(default `session:false`) contain one window; session presets contain multiple.
An optional absolute `cwd` supplies the directory for tabs whose saved `directory`
is null. Explicit saved tab directories take precedence. This lets several named
presets reuse one template for different projects without editing the template.
The resulting directories are checked before any shell starts; the source file
is never modified by applying a preset.
Up to 32 presets are allowed, with unique names and absolute paths. Invalid files
or missing working directories report an import error. Configuration loading does
not read preset files. A binding selects exactly one `action`, `command`, `text`,
or `preset`; preset shortcuts resolve the current configuration at invocation.

Use `rmux --session PATH` to restore a version-1 multi-window recovery file with
fresh shells. It preserves window order and the selected window; an empty session
opens one default window. `--check-session PATH` validates structure without
launching shells or checking directories. Session files allow at most 16 windows,
256 total tabs and 8 MiB. `--rpc-stdio-live` works with session restore.
Process/output history is not resumed.
Saved layouts and sessions also include window position, size and display mode.
Pane zoom is saved per workspace; restoring it retains the underlying split tree
and proportions, and focuses the visible pane. Older files default to unzoomed.
Older files use default placement. If displays change, restore clamps the window
to an available display. Moving/resizing a window schedules a checkpoint when
automatic recovery is enabled. Fullscreen/maximized and multi-monitor visual
acceptance remain pending.

To opt into recovery, set `"session_file": "/absolute/path/to/rmux-session.json"`
in settings. Its parent directory must already exist. Recovery requires an
exclusive OS lock on a sibling `.lock` file. Another
process using that recovery file opens a fresh window with recovery disabled,
so it cannot overwrite the owner's checkpoint. The lock file remains on disk;
ownership is released by the OS when the process exits, including forced stops.
Normal launches restore
that file if present; normal Quit captures the current windows and awaits an
atomic background save within the shared three-second cleanup deadline. Closing
all windows records an empty session. Corrupt/unrestorable files log an error and
open a fresh window with a recovery notice. Automatic saving is disabled for that
launch so the original file remains available for repair; Save Session can write
the new session separately. Lock failures also produce a recovery notice. Explicit
`--layout`, `--session`, directory/command launches and benchmarks neither restore
nor overwrite the configured recovery file. This setting is frozen at startup;
reload takes effect on the next launch. Saves can be skipped during an active
editing/restore/close interaction; failures are logged. OS-originated quit has
GPUI's shorter best-effort deadline. Window/workspace/pane changes also schedule
a checkpoint after 750 ms, coalescing notifications in a one-slot queue. There is
no idle polling or terminal-frame observer. Writes are serialized, unchanged
serialized state is skipped, and older jobs cannot replace newer checkpoints.
Deduplication retains one encoded session (at most 8 MiB) while enabled.
Directory snapshots use delivered shell metadata without waiting for the terminal
parser; an explicit startup directory is retained until a directory report arrives.
Recent changes may be lost before a checkpoint completes. The native smoke verifies
forced-stop/relaunch from a completed checkpoint, with fresh shells and preserved
layout/directory metadata. Power-loss durability, interruption during file
replacement and high-output capture performance remain unverified.

Live RPC `system.quit` requests the normal quit flow, including the configured
close confirmation. `{"requested":true}` confirms the request, not exit or save
completion; a notification can be used when no reply is needed.

Use `rmux --layout PATH` to start with a saved layout and fresh shells. Layout
validation and shell preparation finish before the first window opens; failures
return exit code 2. This cannot be combined with `--working-directory` or benchmark
mode. It restores layout metadata, not running processes or scrollback.

Drag a terminal tab onto another sidebar workspace to move that specific tab into
the destination's remembered pane. Valid rows highlight during the drag; the
destination becomes active after dropping. Cross-window tab drops are not supported.
New zsh sessions automatically load prompt and directory integration through a
private temporary startup wrapper. Existing user startup files run in order;
their `ZDOTDIR` is restored before `.zshrc`, and no shell configuration files are
modified. Set `RMUX_SHELL_INTEGRATION=0` before launching rmux to opt out.
The hooks emit OSC 133 boundaries and percent-encoded OSC 7 paths using built-ins.
For an already-running shell, source `shell-integration/zsh/rmux.zsh` manually;
the macOS bundle also carries it under `Contents/Resources/shell-integration/zsh/`.
Fish automatically loads its hooks after normal shell configuration for both
login and non-login sessions. PowerShell integration remains pending. Run
`mise run test-zsh-integration` for the focused hook check.

Bash reports completed-command status through OSC 133 D before the next prompt.
A leading hook captures the command's status and returns it unchanged for the
first user prompt hook; the trailing integration hook emits the captured value.
User hooks therefore cannot accidentally turn a failed command into a successful
status report. The first prompt does not emit a synthetic completion. This does
not add a command-start DEBUG trap or live running-command tracking.

Bash integration can currently be sourced manually from
`shell-integration/bash/rmux.bash` (also included in newly built macOS bundles).
It reports encoded working directories and prompt boundaries, preserves existing
`PROMPT_COMMAND` hooks, and supports `RMUX_SHELL_INTEGRATION=0`. Run
`mise run test-bash-integration` for its focused check. Automatic Bash login startup,
and command-start markers are still pending; the
current fixture covers macOS Bash 3.2, with modern Bash array-hook runtime
coverage still pending.

Unix shells launch as interactive login shells by default. Set `RMUX_SHELL_LOGIN=0`
before launching rmux to use interactive non-login shells instead. In this mode,
Bash automatically loads integration through a private `--rcfile` wrapper that
sources the user's `.bashrc` first. No user startup files are modified. Bash login
shells retain their normal startup path and require manual sourcing; use
`RMUX_SHELL_INTEGRATION=0` to disable automatic hooks in either supported shell.
The login-mode setting affects default-shell tabs, splits and restored layouts,
not explicit commands passed after `--`, and has no effect on Windows.
The macOS PTY fixture verifies both zsh and non-login Bash directory reporting,
Unicode path decoding, prompt navigation through scrollback and shell exit.

Fish hooks are available in `shell-integration/fish/rmux.fish`, also copied
into newly built macOS bundles. They report percent-encoded working directories,
prompt start, command start/end and command exit status through fish events,
without replacing prompt functions. Source only once per session; repeated
sourcing is guarded, and `RMUX_SHELL_INTEGRATION=0` disables installation.
These hooks do not emit a prompt-end/input-start marker or install automatically.
`mise run test-fish-integration` uses an already-installed fish; runtime verification
is pending because this machine has no fish executable. Event behavior follows
the [fish event documentation](https://fishshell.com/docs/current/language.html#event-handlers)
and [built-in URL escaping](https://fishshell.com/docs/current/cmds/string-escape.html).

View → Previous Prompt / Next Prompt navigate shell-reported prompt boundaries.
Use Cmd+Up/Down on macOS or Ctrl+Shift+Up/Down on Linux/Windows, or the command
palette. This requires semantic shell markers such as OSC 133; unmarked output
does not move the viewport. New zsh sessions supply these markers automatically.

Find Older/Newer Match reopen a closed search using its remembered query and
select a result on the first invocation. Explicit Find still prefers selected
text when prefilling a new search. Find Older/Newer Match are available in Edit
and the palette. Use Cmd+G /
Cmd+Shift+G on macOS, or F3 / Shift+F3 on Linux/Windows, with the terminal or find
field focused. If Find is closed, these commands open it. The find bar also has
28-pixel hover controls with labels for navigation and closing.

Unfocused terminals compress eligible scrollback after a quiet delay, using
bounded background steps. History restores transparently when accessed. Focusing
a terminal or opening Find cancels scheduling; completed passes stop until later
activity. Savings depend on content and platform support.

Edit → Clear Scrollback (also in the palette) removes retained history while
preserving the current screen and running shell. It clears selection and closes
Find; new output continues to accumulate within the normal history limit.

Edit → Select All selects the active terminal screen and its scrollback. Use
Cmd+A on macOS or Ctrl+Shift+A on Linux/Windows, then Copy. Alternate-screen
applications select only their own screen. In text fields, Select All selects
the field text instead.

View → Move Workspace to New Window (also in the command palette) moves the whole
workspace with its existing splits, tabs, live sessions, labels, color and pin
state. The source window closes if it has no remaining workspaces.

View → Equalize Pane Sizes (also in the palette) gives siblings equal space at
every split, exits pane zoom and preserves the split tree, tabs and running shells.

View → Detach Tab to New Window (also in the command palette) moves the active
tab into its own window, retaining its running shell, output and font size.
An emptied source pane/workspace closes.

The command palette also offers `Move Tab to Workspace …` for each other workspace
in this window. Search by its name to choose a destination directly; numbered
results distinguish duplicate names and resolve targets by stable workspace ID.
Multi-tab pane headers also have a `⌄` tab picker for crowded strips. Use its list
to see titles and unread markers; Up/Down navigates, Enter selects, and Escape
returns to the terminal without switching tabs. Page Up/Down advances by the
visible menu height using measured row sizes, including directory subtitles.
Open it directly with ⌘⇧T on macOS or Ctrl+Alt+Shift+T elsewhere, or choose
“Choose Terminal Tab” in the command palette. Type to filter titles and working
directories: all whitespace-separated query words must match either field,
ignoring case. Abbreviations work as in the command palette: `bld rmx` can match
“Build server” in `/work/rmux`. Contiguous matches rank ahead of subsequences;
equally strong title matches rank ahead of directory matches. With an empty query,
tabs retain their original order. Each row shows its cached working directory below the title when
available, with the full title and path in its tooltip. This uses the last
shell-reported directory or initial directory, without filesystem polling.
Keyboard selection follows the terminal identity when live title/directory updates
change result order. If that terminal no longer matches, the first result becomes
the stored fallback and is revealed, so an old match cannot silently reclaim
selection later; changing the query starts at the first result.
Moving focus elsewhere dismisses the picker.
The picker displays original tab numbers, a matching count and navigation hints.
A displayed tab number matches an exact query word: `3` prioritizes tab 3, and
`3 server` also requires a title or directory match for “server”. Numbered titles
and paths remain searchable normally. Home/End edit the search field. Page Up/Down moves
the highlighted choice by eight results without activating it; Enter opens the
choice and Escape cancels. Empty results cannot activate a terminal. The search
field uses the shared Unicode composition and editing component.
Command-palette terminal results also show and search cached working directories
across all open rmux windows. Other-window results include a window label;
selecting one focuses its existing terminal and activates that window. Closed,
moved or busy destinations show an error instead of switching to another tab.
These paths and targets are captured when the palette opens; reopen it to refresh
them after a shell changes directories or a terminal moves.
Other-window workspaces also appear as `Switch to Window … / Workspace: …`.
Selecting one restores that workspace's remembered pane focus and activates its
window. Switching does not transfer tabs or restart shells; closed, moved or busy
destinations produce a notice.
Direct terminal navigation reveals its workspace if hidden inside a collapsed
group and follows that terminal's pane when the workspace is zoomed.
Tab transfers also reveal a collapsed destination group. Move Tab to Next
Workspace follows sidebar order, including group members.
Next Unread Terminal and the notification list also follow sidebar workspace
order, including collapsed group members. Unread navigation checks the current
workspace first, wraps once, and focuses only the chosen unread terminal.
The notification panel shows a message count and a Clear all control. The command
palette also offers Dismiss All Notifications. Both dismiss current messages in
this window and clear their unread indicators without switching terminals;
subsequent notification revisions remain visible.
For another existing window, choose `Move Tab to Window … / Workspace: …`.
The destination's remembered pane receives the existing terminal and custom tab
label, becomes selected, and follows zoom if enabled. The destination window is
activated. Closed or busy destinations report an error before removing the source
tab. Native cross-window tab interaction acceptance is still pending.
View → Move Tab to Next Workspace (also in the command palette) appends the active
terminal to the next workspace's remembered pane, wrapping at the end. Its session
stays alive and the destination becomes active. Empty source panes/workspaces
collapse; with only one workspace, the action does nothing.
View → Join Pane into Next (also in the command palette) appends the focused
pane's tabs to the next pane in layout order, preserves their order and active
terminal, then collapses the empty split. The last pane wraps to the first;
a single pane is unchanged.
View → Move Tab to Split Left / Right / Above / Below (also in the command palette) moves the
active tab into a new adjacent pane without starting a new shell. It requires at
least two tabs in the source pane and focuses the moved tab. A single-tab pane
is unchanged.
View → Swap Pane with Next (also in the command palette) exchanges the focused
pane with the next pane in layout order, wrapping at the end. Tabs and sessions
stay attached to their pane, focus follows the moved pane, and split proportions
stay attached to their positions. Swapping exits zoom to reveal the new layout.
Cmd+Shift+Enter toggles pane zoom (Ctrl+Shift+Enter on Linux/Windows), also available
under View. Click the restore strip or repeat the shortcut to restore the splits.
Ctrl+Tab cycles panes while zoomed. Creating or closing a pane restores the layout;
directional focus shortcuts apply when the split layout is visible.

Drag split dividers to resize adjacent panes; double-click to give the adjacent
branches equal space. Resizing and closing panes preserve surviving sessions.

OSC 9, OSC 777 and OSC 99 title/body notification requests mark unfocused
terminals unread, like BEL. OSC 99 supports chunk assembly and Base64 text, with
up to 16 pending message IDs, 256 title bytes and 1024 body bytes. OSC 99
`o=always` (default), `o=unfocused` and `o=invisible` control whether a request
is accepted. Empty `p=?` queries return `p=title,body,?` and the supported `o`
values, preserving the query identifier and BEL/ST terminator. Queries do not
create notifications. Base64 text accepts padded or unpadded chunks, including encoding groups and
Unicode characters split across successive chunks.
Hidden tabs/workspaces, zoom-hidden panes and inactive OS windows
count as invisible. Suppressed requests are discarded, not replayed on blur.
OSC 9;4 progress reports show a percentage or Working/Paused/Error label in the
terminal tab. Remove reports and terminal reset clear it; no polling is used.
Process exit or terminal output closure clears running, indeterminate and paused
progress. Error progress remains visible until cleared or reset. Buffered activity
reports arriving after completion cannot restore a stale Working indicator.
OSC 7501 program status shows a tab label and hover details. Blocked work takes
priority, followed by errors, completed work, running work and idle state; `+N`
indicates additional records. Up to 64 records are kept per terminal. Shell prompt,
exit, reset and focus events clean up records according to their state.
View → Toggle Program Status (also in the palette) shows all records for the
focused terminal, including hierarchical IDs, inherited app names and messages.
If terminal output is busy, the panel opens with a Loading state and fills in
automatically; closing it cancels the pending load.
Hover a truncated task heading or app/title line for details. Each task's **Copy**
control copies its heading, app/title and complete message to the clipboard.
View → Toggle Notifications (also in the palette) opens a panel with up to 32 recent
messages per live terminal, with separate read state for each message. All and Unread
filters show live counts; press U while the panel is focused to switch filters.
Arrow keys or Tab navigate the visible messages, Enter opens the selected terminal,
and Delete dismisses the selected message. Switching filters preserves selection
when that message remains visible. Click a message to focus its terminal, or Dismiss
to acknowledge it without switching focus. Panes with unread tabs show an amber
border. A new notification appears again after dismissal. Focusing a terminal or Mark All
Read acknowledges retained messages without deleting them. Dismiss All clears the
window’s retained messages while preserving bell/status alerts. History lasts for
the terminal’s lifetime; native OS notifications are not yet implemented. OSC 99
action/close reports, replacement IDs after delivery,
expiry, icons and buttons remain unsupported.
Cmd+N creates a workspace; click sidebar rows or use Cmd+Alt+Up/Down to switch.
Right-click a workspace for Rename, Description, Color, Icon, Pin/Unpin and
Close actions. This selects that workspace and clears sidebar multi-selection;
Close uses the normal confirmation policy. Arrows/Home/End navigate the menu,
Enter/Space activates, and Escape restores terminal focus. Outside click or
focus loss dismisses it. Show Workspace Actions in the palette also opens this
menu; `workspace_menu` is available for custom shortcut bindings.
Right-clicking a group header instead offers Rename Group, group Color/Icon,
Pin/Unpin Group, Ungroup Workspaces and Close Group. Ungroup keeps the workspaces
and their running terminals; Close Group uses the existing group-close prompt.
Show Workspace Actions still targets the active workspace, including a group
anchor, so its individual metadata remains accessible.
View → Workspace Icon offers Terminal, Code, Build, Cloud, Notes and None.
The same choices are available in the palette. Icons persist in saved layouts
and follow a workspace when it moves into a new window.
File → Edit Workspace Icon accepts a single symbol or emoji. Edit Workspace Color
accepts an exact six-digit hex color, with or without `#`. Both are also in the
palette; an empty value clears the setting. Invalid input stays open for correction.
Both editors also show preset icon buttons or color swatches. Click a preset to
fill the draft, then press Enter to save or Escape to cancel. Preset edits support
the text field's undo/redo; choosing None stages an empty value.

View → Workspace Color offers six accent colors and None to clear the marker.
The same named choices appear in the command palette. Colors persist in saved
layouts and do not change the terminal's ANSI colors or unread indicator.

File → Edit Workspace Description (also in the palette) edits a secondary sidebar
label. Enter saves, Escape cancels, and an empty value clears it. Descriptions
are retained in saved layouts; older layouts default to no description.

View → Pin / Unpin Workspace (also in the palette) keeps pinned workspaces at the
top. Reordering stays within pinned/unpinned groups. Save/Open Layout preserves
pins; older layouts open unpinned.
View → Last Workspace (also in the palette) toggles between the current and most
recently visited workspace. Reordering preserves the target; closing it clears
the remembered destination. This history is per window and is not saved.
Cmd+= / Cmd+- change the focused terminal's font size; Cmd+0 resets it to the
configured font size (14 by default).
Linux/Windows use Ctrl+= / Ctrl+- / Ctrl+0. These controls are also in Edit and the
command palette. Zoom is per terminal, ranges from 8 to 40, and follows a moved
tab. New tabs and splits inherit the source terminal's size; a new workspace starts
at the configured size. Save/Open Layout preserves each terminal's size; older
layouts without a saved size default to 14. Reset Font Size preserves that
terminal's font family; Use Configured Font restores both the configured family
and size. Custom shortcuts can bind these separately as `reset_font_size` and
`use_configured_font`, or use `increase_font_size`/`decrease_font_size` for steps.
Cmd+Shift+P opens the command palette (Ctrl+Shift+P on Linux/Windows). Type words
to filter implemented actions and workspace names (abbreviations such as `splt blw`
also match, with contiguous matches ranked first), use Up/Down and Enter to run
an action or switch workspaces, or Escape to return to the previous focus.
Workspace results include their sidebar position to distinguish duplicate names.
Clicking outside dismisses it.
Cmd+1 through Cmd+8 select the first eight workspaces; Cmd+9 selects the last.
Linux/Windows use Ctrl+Alt+1 through Ctrl+Alt+9. Sidebar rows show shortcut hints,
and keyboard selection scrolls the selected workspace into view.
Cmd+B toggles the sidebar. Linux/Windows use Ctrl+Shift+N, Ctrl+Alt+Up/Down and
Ctrl+Shift+B. Switching preserves each workspace's sessions, split layout and
last focused pane. Cmd+Alt+Shift+Up/Down moves the active workspace in the sidebar
(Ctrl+Alt+Shift+Up/Down on Linux/Windows).

Cmd+Shift+R renames the active workspace (Ctrl+Shift+R on Linux/Windows), also
available in the File menu. Enter saves and Escape cancels. Names appear in the
sidebar and native window title; Save Layout preserves them for manual restoration.

Cmd+Shift+N opens a separate native window; Cmd+Shift+W closes the current window.
Linux/Windows use Ctrl+Alt+N and Ctrl+Alt+W. Each window owns its workspace list.
Closing a window drops its terminal sessions; quitting the app cleans up sessions
across all windows. Native menus expose the main window, workspace, tab, edit and
split commands where the platform provides an application menu.
System-originated quits that bypass rmux's Quit action request process cleanup
through GPUI's shutdown hook. That hook cannot show a cancellable confirmation
and allows only 100 ms to await completion; the normal Quit action retains its
confirmation policy and three-second cleanup deadline.

Cmd+T creates a tab in the focused pane; Cmd+Shift+[ / ] switches tabs.
Right-click a tab to select it and open a menu with Rename, Copy/Reveal Working
Directory, Move Tab Left/Right,
Close, Close Other Tabs and Close Tabs to the Right. Arrow keys navigate, Enter/Space activates,
and Escape returns to the terminal. Clicking outside or moving focus dismisses
the menu. Shift+F10 or Show Tab Actions in the palette opens the menu without
a mouse; the configurable action is `tab_menu`. Unavailable actions are
dimmed and skipped by keyboard navigation. Moving the pointer over an enabled
action selects it for keyboard activation too. Moving a tab preserves its running
terminal; left/right actions are unavailable at the respective ends of the strip.
Directory actions use the clicked terminal's asynchronous local-directory
lookup. Custom shortcuts can name `copy_working_directory` and
`reveal_working_directory`; unavailable directories report an error on that terminal.
If you repeat or switch these actions during lookup, the latest request replaces
the earlier one. Superseded copies do not overwrite newer clipboard content.
Close Other Tabs in the menu/palette keeps the chosen terminal and closes the
other tabs in that pane, using one confirmation when the close policy requires
it. The `close_other_tabs` shortcut action has no default binding. The prompt
captures its targets: tabs added afterward are spared, and the operation aborts
if the terminal being kept leaves the pane before confirmation.
Close Tabs to the Right uses the same policy and captured-target rules, keeping
the chosen tab and those to its left. It is available in the menu/palette and
as `close_tabs_to_right`, with no default shortcut.
New tabs start in the background. The Starting… indicator has a cancel button;
Cancel New Terminal Tab is also available in the menu/palette and as the
`cancel_new_tab` shortcut action. Cancellation preserves existing tabs and focus.
The pane accepts another launch once the pending job finishes cleanup; an OS
startup operation already in progress cannot be interrupted immediately.
Cmd+Shift+1–9 selects a tab directly within that pane (Ctrl+Shift+1–9 on
Linux/Windows). Missing tab numbers do nothing; 9 selects the ninth tab.
The palette also offers Select Tab 1–9, Next Tab and Previous Tab.
Double-click a tab to rename it, hover its title to read the full label, or
middle-click to close it using the configured close-confirmation policy.

Copy Working Directory in Edit or the palette copies the focused terminal's
reported local directory as plain text, without shell quoting. It requires shell
directory reporting; remote or unavailable directory metadata leaves the clipboard
unchanged and shows a message.
Reveal Working Directory uses the same local-only metadata to reveal the path
in the system file manager (Finder on macOS). Neither action runs a shell command.
Linux/Windows use Ctrl+Shift+T and Ctrl+PageUp/PageDown. Click tab labels to
select them, × to close, or + to create. Ctrl+Shift+PageUp/PageDown moves the
active tab left/right. Tabs retain their processes and scrollback while hidden.
Drag a tab onto another tab to move it to that position within the pane. Drag a
workspace row onto another row to reorder within its window. Drop a tab onto a
tab in another pane of the same workspace to move it there. The existing session
is retained and becomes active; an empty source pane collapses. Moving between
workspaces or native windows is not implemented yet.
Shell-provided OSC 0/2 titles replace the default tab labels when available.
New tabs and splits inherit the focused shell's directory when it reports a local
OSC 7 file URI. Without a usable report they use the normal shell startup directory.
Remote-host directory reports are not interpreted as local paths. No shell startup
files are modified; automatic reporting depends on your shell configuration.
Workspace rows show the focused pane's reported local directory beneath its name.

File → Save Layout… exports the current window's workspaces, splits, tabs, focus
and reported directories to a JSON file. File → Open Layout in New Window… reads
that file and starts fresh shells in the saved directories, restoring names,
tabs, split proportions, focus and sidebar width. Preparation happens in the
background with a Cancel control; errors leave existing windows intact. Closing
the originating window also cancels preparation. Automatic restoration is not
implemented. Layout files do not save running processes or terminal scrollback.

Cmd+W closes the active tab (Ctrl+Shift+W on Linux/Windows). Closing its last tab
removes the pane. Ctrl+Tab and Ctrl+Shift+Tab cycle pane focus. Closing the last
pane removes its workspace; closing the last workspace closes the window.
File → Close Workspace (also in the command palette) closes the active workspace
and its terminal sessions together, then selects a remaining workspace.

A bell from an unfocused terminal marks its tab and workspace with an amber dot. Focusing that
terminal in the active window clears the marker. System notifications and sounds
are not implemented yet.

Cmd+Shift+U jumps to an unread terminal (Ctrl+Alt+U on Linux/Windows), also under
View → Next Unread Terminal. It searches the current workspace first, then wraps
through this window's other workspaces in sidebar order. If a workspace is zoomed,
the target pane becomes the zoomed pane. With no unread terminals it does nothing.

Cmd+Ctrl+Arrow focuses a pane in that direction (Alt+Shift+Arrow on Linux/Windows).
Navigation uses the current pane geometry, prioritizing aligned neighbors, and
does not wrap at the outer edge. Ctrl+Tab still cycles panes in layout order.

Use the wheel/trackpad or Shift+PageUp/PageDown to browse scrollback; Shift+Home/End goes to the oldest/live output. Typing or pasting returns to the live viewport.

Last Prompt in the menu or command palette jumps to the newest retained shell
prompt, including one above a long command's output. It uses shell semantic
markers; without them it returns to live output.

Drag to select terminal text, double-click a word, or triple-click a line.
Alt-drag selects a rectangular block. Cmd+C copies the selection on macOS;
Ctrl+Shift+C copies on Linux/Windows. Hold Shift to select locally when a terminal
application has enabled mouse reporting. Copy uses Ghostty's text extraction,
including Unicode, joining soft-wrapped lines. Choose **Copy with Line Breaks**
from the Edit menu or command palette to retain those visual line boundaries.
It can be bound using `copy_with_line_breaks` in the `terminal` context; no default
shortcut is assigned. Both modes preserve explicit newlines and reject selections
above 16 MiB instead of copying a partial result. Drag above/below the terminal to extend
selection through scrollback; scrolling stops on release or at the history edge.

Cmd-click an OSC 8 hyperlink or plain-text HTTP(S) URL to open it in the default
browser (Ctrl-click on Linux/Windows). The lookup follows the displayed scrollback
viewport and runs only on modifier-click or modifier-hover; ordinary rendering does not copy link
URLs. Plain URLs can span soft-wrapped rows, but must be fully visible and fit
within an 8 KiB logical line. Surrounding quotes and trailing prose punctuation
are excluded. Explicit OSC 8 targets take precedence. Hold the link modifier while hovering to preview the target. Native hover
acceptance and local/remote file-path routing are still pending.

Cmd+F opens terminal search (Ctrl+Shift+F on Linux/Windows). A single-line selection
of up to 4 KiB prefills a newly opened search, selected for easy replacement;
otherwise Find restores that terminal's last query (empty for a new terminal).
The query survives closing Find, clearing scrollback and moving the terminal to
another window; it is not saved in layout files. Type a query, then
Enter to visit older matches or Shift+Enter for newer ones; the arrow buttons do
the same. Escape or × closes the find bar. Search includes scrollback, updates
with output, and runs in a background worker. Matching ignores ASCII letter case;
regex and case-sensitive modes are not yet available.
Invoke Find again while it is open to focus and select the query for replacement.
Active IME composition is preserved rather than selecting over marked text.

When a terminal application enables mouse reporting, clicks, movement and vertical wheel events are sent through Ghostty's mouse encoder. Hold Shift while scrolling to browse local history instead.

`mise run bundle-release` creates the optimized macOS bundle. For isolated idle measurements, run `mise exec -- sh scripts/prepare-macos-benchmark.sh 8`, open the resulting benchmark bundle, and sample its PID with `mise exec -- sh scripts/sample-macos-idle.sh PID`. The benchmark uses clean shells and disables the Metal HUD without changing normal launches or system settings.

The current UI is a P0 rendering prototype. Control keys use Ghostty's encoder and paste respects bracketed-paste mode. Full keyboard protocols, selection/search, accessibility, nested layouts and persistence remain unfinished. Linux/Windows dependency setup is provided, but their desktop builds and runtime behavior have not been validated. See [platform build instructions](building.md) and [implementation progress](progress.md). `mise run bench-engine` measures release engine throughput and snapshot costs for 1/4/16 panes.

`mise run bench-pty` measures input calls and response snapshots under output pressure from up to 15 other PTY sessions. It does not measure GUI or input-to-pixel latency.

Start with the [roadmap](roadmap.md), then the [feature inventory](feature-parity.md).

| Document | Purpose |
| --- | --- |
| [Roadmap](roadmap.md) | Delivery phases, dependencies and completion gates |
| [Feature parity](feature-parity.md) | 132 capability families with evidence and acceptance criteria |
| [Architecture](architecture.md) | Proposed cross-platform design and decisions requiring prototypes |
| [Performance](performance.md) | Workloads, measurement definitions and initial targets |
| [Research and sources](research.md) | Pinned baseline, evidence limits, discrepancies and refresh procedure |
| [Contract inventories](reference/README.md) | Commands, settings, shortcut IDs and static RPC capabilities |

The comparison baseline is cmux commit `a76816e4f0788003758b9434ab702068a640fb5d`, reviewed November 19, 2026. Main-branch capabilities are not all stable-release features. rmux's architecture and phase assignments are proposals; upstream evidence is identified separately.

Mark All Read in This Window is available in the View menu, command palette,
and notification panel. It acknowledges bell, notification and task-status
alerts across all workspaces and hidden tabs in the current window while keeping
messages visible. Other windows retain their unread state. A custom shortcut can
bind `mark_all_read`. Clear all dismisses messages instead.

Terminal, tab and split-operation errors appear in dismissible banners. Use the
copy icon to copy the full message or the close icon to dismiss it. Long messages
scroll inside the banner; dismissal does not retry the failed operation.

### Editing text fields

In search, rename and settings text fields on macOS, Command-Backspace deletes
to the start and Command-Forward Delete deletes to the end. With text selected,
these keys delete the selection. Command-Z undoes the edit. Option-Backspace
continues to delete by word; on Linux and Windows, use Control-Backspace for
word deletion.

### Marking a notification read from scripts

Use `rmuxctl read-notification --surface ID --revision NUMBER` with the identifiers
returned by `list-notifications`. It calls `notification.mark_read`, retains the
message in history and does not focus the source terminal. The response includes
`marked_read: true` when an unread message changed; missing or already-read
messages return `false`. An unknown surface remains an RPC error.

Group actions can be assigned custom shortcuts using `group_current_workspace`,
`edit_group_color`, `edit_group_icon`, `toggle_group_pin`, `ungroup_workspaces`,
`close_group`, `remove_workspace_from_group` and `set_group_anchor`.
`close_workspace` targets the current workspace and keeps its usual confirmation
policy. These action names do not add default key bindings.

### Resize panes from the keyboard

The menu and command palette expose **Resize Pane Left/Right/Up/Down**. Bind
`resize_pane_left`, `resize_pane_right`, `resize_pane_up`, or `resize_pane_down`
to your preferred keys in `keybindings`; no new default bindings are reserved.
Each action moves the nearest divider on that axis by 5% of its neighboring
pair's space. It prefers the requested side; at an outer edge it moves the
opposite shared divider in that direction, shrinking the focused pane. Nested
splits resize locally before considering outer splits. Each neighbor retains at
least 10% of the pair's space, or its existing share if it was already narrower
from a drag or restored layout. Such narrow panes never snap in the opposite
direction; growing them raises the minimum toward 10%. Existing pane identities, tabs and unrelated split
weights remain intact. Restore a zoomed layout before resizing; actions are also
ignored while a split shell is starting. **Equalize Pane Sizes** resets ratios.

Scripts can invoke the same directional resize operation with
`rmuxctl resize-pane --pane PANE_ID --direction left|right|up|down`, or JSON-RPC
`pane.resize` with `pane_id` and `direction`. The response contains `pane_id`,
`workspace_id` and `changed`; `changed: false` covers a single pane, missing axis
or a clamped divider. It does not change workspace selection or OS focus. Zoomed
layouts, active divider drags and pending split launches are rejected as busy.
Use `workspace.list` to read the updated split weights.

When a terminal process exits, its output stays available and a status strip shows
the exit message. **New shell** opens a fresh tab through the normal new-tab path,
using its directory/font inheritance; it does not replay the exited command.
The adjacent close button closes the finished terminal through the normal close
policy. Input, rendering and other operational errors retain their separate error
messages.

Finished terminals carry a **Finished** label in the tab strip. Hover the title
for the exit message, or search terminals in the palette to see and search their
finished status across windows. Custom tab names remain intact.

**Close Finished Tabs…** in the menu or command palette closes exited terminals
in the current pane while retaining the current tab and all running terminals.
It follows your close-confirmation policy. Tabs created or moved into the pane
while a confirmation is open are excluded. Bind `close_finished_tabs` to a custom
shortcut if desired; it has no default binding.

The pane's **Choose Terminal Tab** picker also searches finished state and exit
messages. Its scrollable results use fixed-height rows and render on demand as
you scroll, keeping large tab collections from building every result row at once.

**Split Left** and **Split Above** are also available in the menu and command
palette. Bind `split_left` or `split_up` for custom shortcuts; they have no default
bindings. All four split directions inherit the target terminal’s directory and
font settings through the same shell-launch path.

Palette search includes the shortcut shown beside each action. Try separate words
such as `cmd shift d` on macOS or `ctrl shift k` for a configured binding; macOS
symbols such as `⇧⌘D` also work. Combine key names with an action name to narrow
fuzzy matches. Hints reflect the originating terminal's bindings when you open
the picker, including custom overrides and disabled shortcuts.

To balance every split in a workspace from a script, run
`rmuxctl equalize-panes --workspace WORKSPACE_ID` or call JSON-RPC
`workspace.equalize` with `workspace_id`. The response includes `workspace_id`
and `changed`. It normalizes each split's weights to one without moving terminals
or changing focus/selection; repeated calls and single-pane layouts return
`changed: false`. Zoomed layouts, active divider drags and pending split launches
are rejected as busy. Unzoom first to equalize through automation.

For a focused developer check of these layout operations, build the desktop and
run `mise exec -- python scripts/smoke-workspace-placement.py --live-shell`.
It launches an isolated test app and fixture shells, checks retained process state
through resize/split/equalize, and observes cleanup without attaching to existing
rmux windows.
