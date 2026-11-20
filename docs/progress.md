# Implementation progress

## Current implementation focus

Completed streaming OSC 99 Base64 text assembly: encoded groups and multibyte
Unicode can span chunks, title/body maintain independent state, and independently
padded chunks still concatenate. A final unpadded group is flushed at message
completion; malformed or incomplete final text is discarded. Only up to three
encoded characters and three undecoded UTF-8 bytes persist per text field, besides
the bounded title/body. Plain text avoids a payload-sized scratch allocation.
Four focused tests passed, including every split of multilingual encoded text,
one-character chunks, interleaved title/body completion, padding, and malformed
endings. This finishes the previously documented encoding-group gap; notification
history, lifecycle actions and native UI acceptance remain outstanding.

OSC 99 capability discovery now replies to empty `p=?` queries with supported
text payloads and visibility policies, preserving safe identifiers and BEL/ST
termination. Unsupported action/close/sound/expiry features are not advertised.
Replies use the existing ordered PTY response path and are capped at 64 KiB per
drain; complete responses are omitted when capacity is exhausted. Response payloads
are never answered, preventing feedback loops. Identifier validation now follows
the protocol alphabet, and complete unpadded Base64 text chunks are accepted.
Focused query framing/order/bounds and text assembly tests passed. Cross-chunk
Base64 encoding groups and remaining notification lifecycle features are pending.

OSC 99 now retains `o=always|unfocused|invisible` across notification chunks and
applies the policy before storing messages or marking unread. Invisible uses the
active OS window, selected workspace/tab and pane zoom structure without acquiring
parser locks; unfocused uses the existing focus state. Suppression advances the
engine revision, preventing stale alerts on later focus changes. Legacy OSC/RPC
notifications retain their existing always-accept behavior. Three focused protocol
and truth-table tests passed and the desktop compiled. Native activation, hidden
pane/tab and multi-window interaction acceptance remain pending.

Added a conservative OSC 99 prefix fast path after the boundary benchmark exposed
OSC-heavy overhead. Chunks without `9`, without a potentially split introducer,
and without an active capture use Ghostty's original bulk writer. Differential
framing tests compare every split of mixed control streams with the boundary-only
path. In the same one/eight-pane benchmark, the no-digit-9 shell-metadata fixture
moved from 25–28% slower to within roughly 1% of the prior raw path; ANSI/Unicode
was within 1%, plain text 2–4% slower, and engine-resident bytes matched. Chunks
containing `9` conservatively retain the boundary path, so remaining overhead is
workload-dependent. See [measurements](benchmarks/notification-boundaries.md).
Validation: 77 terminal tests passed in the combined run; one existing shell
resize test hit a transient `openpty` allocation error and passed on its focused
retry. The manual throughput benchmark is ignored during ordinary test runs.

Measured OSC 99 stream-boundary overhead against the prior raw writer in one
release executable, alternating three samples at one/eight panes. ANSI/Unicode
throughput stayed within 1% in the original comparison; plain text varied by
1–5%, and synthetic two-OSC-per-line output was 25–28% slower. Engine-resident
memory matched each pair. Two speculative fast paths failed to improve results
consistently and were removed. The reproducible ignored benchmark and raw samples
are in [the measurement report](benchmarks/notification-boundaries.md). This leaves
OSC-heavy throughput as an explicit performance gap; GUI latency is not measured.

Added bounded OSC 99 text notifications (title/body, interleaved chunk IDs and
Base64) using Ghostty's stream-boundary API. Ghostty still owns UTF-8/control-string
framing and cancellation; non-99 OSC payloads are rejected before copying. Limits
are 8 KiB per captured sequence, 16 pending IDs, 256 title bytes and 1024 body bytes;
reset clears pending chunks. Completed messages use the existing notification
revision/unread path alongside OSC 9/777. All 77 terminal tests passed after stream
integration, including byte-split/cancelled strings and legacy coexistence. Desktop
compilation passed. Native UI and throughput acceptance remain pending. Protocol
queries, actions/close reports, post-delivery ID replacement, visibility filtering,
expiry and graphical payloads remain incomplete. Protocol reference:
https://sw.kovidgoyal.net/kitty/desktop-notifications/

Terminal selection copying now formats on the background executor instead of
blocking UI dispatch on the parser and formatter. The existing 16 MiB limit and
no-selection behavior are preserved. One pending copy per terminal coalesces
repeated requests; selection gestures, Select All and Clear Scrollback cancel
pending delivery. A shared generation guard orders results against newer terminal,
text-field, directory, memory-report and notice copies within rmux. Clipboard
export retains no engine until execution. Focused engine export and generation
tests passed, as did desktop compilation. Native clipboard interaction under
output pressure remains pending; other applications' clipboard writes are not
tracked by the internal generation guard.

Removed the blocking parser acquisition after queued text input. Follow-output
scrolling now tries once and otherwise coalesces into one atomic flag, consumed
under the next frame lock. Explicit viewport navigation cancels the deferred
follow; automation input continues preserving scrollback. No thread, timer or
unbounded queue was added. A real-shell regression deliberately holds the parser
lock and verifies input returns promptly, deferred follow reaches bottom, and
later explicit navigation wins. The automation bracketed-paste/viewport test also
passes. Key encoding, mouse handling, focus and other synchronous engine operations
remain performance work; this is not a claim of complete input-latency acceptance.

Settings now exposes new group workspace placement (After current / First / Last)
with keyboard cycling and saved preferences. Group directory capture/reset actions
also appear in the File menu; group creation, selection, rename and directory
actions can be bound to custom shortcuts. Five focused settings tests passed,
covering cycling, navigation and persistence. Native visual acceptance remains
pending: the desktop was checked again and is locked, preventing screenshots.

Workspace groups now retain an optional starting directory independently of anchor
shell navigation. Palette actions capture the focused terminal directory or reset
to following the anchor. Group create/update RPC accepts an absolute cwd, update
supports clear_cwd, and snapshots/layouts/sessions preserve the override. New group
members prefer the override while existing terminals are untouched. Fifteen focused
group tests passed, including mapping/restore and invalid/conflicting RPC input.
Native macOS spawned-shell acceptance now passes via `mise run smoke-group-directory`:
actual shell startup directories remain fixed after anchor navigation, follow an
updated override, then follow the anchor after reset. The anchor retains its PID
and directory throughout. The isolated harness owns its app, shells and temporary
files and checks startup paths independently of UI metadata. Native palette
interaction and other-platform acceptance remain pending.

Added close confirmation to Settings with Active tasks / Always / Never choices,
forward/backward keyboard cycling and immediate application across close scopes.
Saving persists the choice while preserving unrelated config. An environment
policy override stays authoritative and is not written into the saved preference.
Settings navigation now derives its control count from the settings list. Focused
settings tests pass; native panel interaction and appearance remain pending.

Git branch badges now subscribe to native HEAD changes, including atomic checkout
replacement. One watcher serves the application with shared repository watches,
capacity-one event channels and background discovery; idle terminals add no polling
or Git subprocesses. Dropping a terminal queues unsubscription without blocking the
UI. Shell reports and manual refresh remain available when native events are
unavailable. Three focused tests passed, including real native event delivery to
multiple subscribers and continued delivery after one unsubscribes. Native sidebar
visual acceptance remains pending; these changes are for the next release.

Added Mark All Read in This Window in the notification panel, View menu, palette,
and custom shortcut actions. It acknowledges all alert sources across hidden
and visible tabs in this window without focusing terminals, removing messages,
or touching parser locks. Focus acknowledgement shares the same helper. Other
windows remain untouched. The desktop compiled and the existing attention-source
transition test passed. Native interaction acceptance remains pending.

Follow-up after v0.1.0: moved Settings into the application menu and added native
macOS About, Hide, Hide Others and Show All commands. Cmd-H and Cmd-Option-H use
GPUI's platform operations; defaults are registered before custom shortcuts so
reload retains the usual override behavior. About uses AppKit's standard panel
and bundled version metadata. The focused desktop check passed; native panel/menu
interaction remains unverified. The published v0.1.0 archive remains unchanged.
Mouse-focus investigation found an existing focus-first handler; the earlier
capture alone does not establish a defect, so no speculative mouse change was made.

Prepared the first public macOS preview with a user-facing README and real app
screenshot. Added Sparkle's native updater/controller and Check for Updates menu,
public feed/key metadata, separate distribution bundle identity, Developer ID
signing, notarization/stapling and signed update archives. Local packaging passed
Apple notarization and Gatekeeper, and the native updater menu appeared. Published
binaries will be built on GitHub-hosted ARM64 runners through the new release
workflow; its signing credentials are encrypted Actions secrets and imported into
a temporary keychain for Apple signing and a temporary file for Sparkle signing. Release v0.1.0 is now published from GitHub Actions run 44755158627. Its public
ZIP passed checksum, pinned-public-key Sparkle signature, strict code signature,
stapled-ticket and Gatekeeper verification. The raw-main update feed serves that
exact archive. Final native update-dialog confirmation was blocked when the Mac
locked; no end-to-end version upgrade has yet been exercised.

Fixed invisible toolbar SVGs: GPUI requires explicit SVG text color instead of
inherited parent text color. Real-window capture now shows sidebar, notification,
add, close and split controls. Full visual/interaction parity remains incomplete.

Separated notification, bell and task-status unread sources. Dismissing the
current message clears only its notification alert; a bell received while a
message was already unread is retained. Focusing a terminal acknowledges all
sources. Existing tab/sidebar/RPC attention values remain booleans. A focused
state-transition test covers every bell/status combination, both arrival orders,
repeat dismissal and a new notification after focus acknowledgement. The focused
desktop test passed (1 passed); native notification-panel interaction remains
to be exercised with the updated running build.

Actual app screenshots are now available again: the earlier desktop-lock blocker
has cleared. Captures showed the older debug process and newer release process;
neither should be treated as proof that all latest source changes are running.

Tightened shortcut validation around GPUI's permissive key parser. Unknown named
keys and noncanonical function-key spellings now fail instead of creating inert
bindings. Canonical duplicates (including Ctrl-X versus Ctrl-Shift-x) and
single-stroke/chord prefix conflicts within the same custom scope report clear
errors; distinct two-stroke chords sharing a prefix remain valid. Focused tests
passed both insertion orders, aliases, valid shared prefixes and existing real
keymap precedence/disable behavior. Default-versus-custom conflict explanations
and graphical editing remain future work.


Added validated configuration keybindings for common actions and custom commands,
with terminal/default or global window scope and one/two-stroke sequences. Initial
built-in bindings are retained once; reload clears the keymap and reinstalls only
defaults plus current overrides, so removed bindings recover and reloads do not
accumulate handlers. Duplicate keys in a context, unknown targets and plain-text
strokes fail validation. GPUI keymap tests passed override precedence, editor
context isolation, disabled bindings, restoration and invalid/chord definitions.
Desktop build passed. Native key dispatch, full conflict UI and complete shortcut
contract coverage remain pending.


Added notification-panel focus ownership and keyboard navigation using stable
terminal IDs. Arrow/page/Tab movement scrolls the selected message into view;
Enter/Space opens its terminal and closes the panel, Delete/Backspace dismisses,
and Escape restores prior focus. Settings and notifications now share consistent
mutually exclusive open/close behavior. Cached message collection requires no
parser access; card styling uses neutral dark backgrounds with a selected-row
highlight. Desktop build passed. Native event routing, visual quality and
screen-reader acceptance remain pending; desktop capture was rechecked and is
still blocked by the locked Mac.


Refreshed ARM64 Linux checks after recent desktop/CLI/session changes. Desktop and
CLI cross-compilation passed. Sixteen PTY/session runtime tests passed in 1.16 s
inside the existing local Linux image with networking disabled, no pull/install,
read-only code/root and temporary writable /tmp. Coverage includes new nonblocking
metadata/resize paths, paste ordering/viewport preservation, direct-command env,
retention, process cleanup and interactive resize. Saved the exact binary hash
and output in linux-arm64-session-tests-2026-11-20.txt. Linux GUI/IME/presentation
and x86_64 execution remain pending; Windows validation remains deferred.


Extended custom commands with bounded per-child env overrides (64 portable names,
16 KiB total, no NULs; case-insensitive duplicate rejection on Windows). Existing
command entry points retain their default environment. A per-window pending flag
prevents concurrent custom launch bursts and clears on completion/failure. Desktop
build, configuration validation and actual PTY environment fixture passed; Unicode,
empty values and literal shell-looking content remain intact without mutating the
parent environment. Native repeat-activation interaction remains unverified.


Added bounded custom command definitions (name/argv/optional absolute cwd) to the
configuration model and searchable Run entries in the palette. Commands start a
new ungrouped workspace using a prepared PTY session; process startup is off the
UI thread and arguments are passed literally. Cached selected-terminal cwd is
used when no override is supplied. Busy/closing/limit state is rechecked before
inserting the prepared session; unavailable destinations drop the owned session.
Palette argument previews are sanitized and bounded. Desktop build passed;
configuration validation and direct-command PTY tests cover the shared launch
contract. Native palette activation and full upstream command/shortcut/environment
parity remain pending.


Made font defaults observable live: terminal preference observers track the prior
size/family defaults and apply changes independently only where current values
still match them. Custom zoom/family values remain; Reset Font Size and Use
Configured Font can rejoin defaults. The existing font invalidation path updates
cell measurement/cache behavior and persistent metadata without replacing shells.
Updated Settings hints, reload notice and documentation. Focused test passed all
four inherited/custom combinations. Matching is value-based, not an explicit
inheritance flag; native rendering and user interaction acceptance remain pending.


Removed blocking parser acquisition from output metadata handling. A single
try-lock reads title/directory/notification/progress/status summary, optional
status details and render-delay state. The existing per-terminal async update
task retains pending work and retries after 8 ms only on contention, preserving
revision cursors and stopping after success. Synchronized-output timer callbacks
also use nonblocking delay queries. Added metadata_busy diagnostics. Extended
contention regression passed immediate busy results and eventual title delivery;
native socket fixture passed OSC/RPC notification, input, creation and cleanup.
Metadata copies still execute on the UI thread after acquisition; this change
removes waiting, not all engine work or the need for presentation measurements.


Prepaint now uses try_resize before snapshot acquisition. A busy parser leaves
both PTY/engine dimensions untouched and schedules a display-frame retry using
the current layout rather than queueing obsolete drag sizes. Unchanged sizes
avoid engine locking, and Find refresh occurs only after an applied resize.
Added opt-in resize_busy counters. Focused Unix PTY test passed busy/no-op behavior
and latest-size consistency across PTY and engine; desktop build passed. Actual
reflow and PTY resize still run synchronously after access, so this does not prove
resize latency or complete removal of UI-thread engine work. Native drag/output
stress and presentation measurements remain pending.


Refreshed the release PTY pressure baseline: 768 requests across 1/4/8/16 panes
and two additional 16-pane runs completed. Eight-pane response-snapshot p99 was
1.331 ms; sixteen-pane p99 ranged 4.249–5.662 ms, closely matching snapshot-batch
tails. Raw results and limitations are recorded in pty-baseline.md. Inspection
found blocking prepaint snapshot acquisition; switched it to try_frame with cached
frame reuse and next-display-frame retry on contention, plus opt-in snapshot_busy
counter. Desktop build and existing parser-contention regression passed. This
change has not yet been measured in GUI presentation; resize/metadata/key locks
and end-to-end latency acceptance remain open.


Added workspace.close and close-workspace --workspace ID. The operation uses
existing busy/confirmation policy checks and the normal owned-terminal cleanup
path; confirmation-required calls return an error without prompting or mutating.
Fixed shared background-workspace close behavior to retain active focus instead
of reselecting/acknowledging its terminal. Native socket smoke passed background
selection preservation, active-workspace fallback, sole-workspace window removal
and owned fixture-shell reaping. RPC regression tests passed. Forced close and
full cmux close contract remain pending.


Added surface.send_key and send-key CLI with named keys, modifier prefixes,
common aliases and single Unicode characters. Encoding follows current Ghostty
modes and bundles press/release into one writer message, preserving viewport and
focus without waiting on parser access. Focused engine checks passed ASCII/Unicode,
Ctrl-C, application arrows and exact Kitty press/release bytes. Six CLI tests and
native Enter-to-fixture-shell smoke passed. Held keys, sequences and --force are
not yet supported; native Windows validation remains deferred.


Added send --paste and surface.send_text paste:true using a bounded nonblocking
session paste path. Ghostty encodes current bracketed-paste behavior; the writer
queues one complete payload with the existing single-paste permit. No viewport
scroll or focus change. Parser contention returns a retryable error before any
input is queued; automation remains capped at 16 KiB. Focused PTY test passed
exact Unicode/multiline framing, ordering, busy-permit release and viewport
preservation. Native CLI-to-shell smoke also passed paste delivery, and replaced
a timing-based notification assertion with waiting for actual shell output.


Refreshed the optimized macOS app bundle including rmuxctl, notification and
workspace automation, and the Settings panel. Focused Settings navigation/bounds
test passed after resolving a test-macro import collision. The socket integration
fixture passed against both actual release-bundle executables, covering clients,
filters, workspace creation/failure rollback, RPC/OSC notification coexistence and
normal quit cleanup. Existing user app processes were not terminated; an already
running old instance does not update until relaunched. Pixel/interaction review
remains pending and is not implied by this release smoke.


Added Settings keyboard focus ownership, remembered return focus, seven-control
Tab/Shift-Tab/up/down navigation, left/right adjustment, Enter/Space activation
and Escape close. Selected rows/buttons show a highlight and navigation scrolls
them into view. Plain typing is contained in the panel; switching to notifications
closes Settings and restores focus. Removed duplicate open-state storage in favor
of the optional focus/navigation state. Desktop build passed; focused navigation
and value-bound checks cover the pure behavior. Native event routing, focus
restoration and screen-reader acceptance still require an unlocked desktop.


Added an in-app Settings side panel through Cmd+, (Ctrl+, elsewhere), app menu
and palette. Controls cover default text size, padding, terminal theme, unfocused
cursor and copy-on-select, with session changes and explicit Save. Saving merges
only these fields into the latest validated config, preserving other options and
following existing symlinks; invalid edited config is left untouched. Writes use
an atomic temporary-file replacement on a background worker. Font default
semantics are stated in the UI; custom color overrides remain effective. Desktop
build passed; native appearance, keyboard navigation and full settings parity
remain pending on visual/interaction review.


Added notification.send/list/dismiss plus notify/list-notifications CLI commands.
Messages share existing in-app panel/unread state and never focus a terminal.
Latest-per-surface storage stays bounded; dismissal is revision-checked. Separated
shell notification cursor from UI message revision so RPC messages cannot cause
old OSC metadata to replay over new content. Native fixture-shell socket smoke
passed send/replace/list/stale-dismiss, OSC after RPC, and RPC remaining current
after ordinary shell output. Native OS delivery, reply actions and full cmux
clear/history behavior remain pending; screenshot acceptance is still blocked by
the locked desktop.


Added workspace.create and CLI new-workspace/new-window commands. Workspace
creation reuses the normal shell/workspace path with a new no-focus mode, accepts
name/description/absolute cwd, and defaults to preserving the selected workspace.
Explicit focus/activate selects the new workspace; failed shell startup discards
only the empty entry and restores numbering. Busy-state and 64-workspace guards
apply. Native socket smoke passed background/focused creation and failed-cwd
rollback with unchanged discovery state. OS activation still needs manual
acceptance. Command/env/layout overrides and full cmux creation semantics remain.


Added window filtering to workspace discovery and combined window/workspace
filtering to pane/surface discovery, exposed through CLI --window/--workspace.
Filters reject malformed IDs/unknown fields and intersect without changing focus;
valid missing/mismatched targets return empty lists. Unrelated windows and
workspace children are skipped before snapshot construction. CLI tests and native
two-window socket smoke passed, including combined filters, mismatched ownership
and malformed-ID errors. Caller permissions and ref/index resolution remain open.


Added nine common CLI commands based on the checked-in cmux command inventory:
window/workspace/pane/panel lists, workspace select/rename, panel focus, text send,
and screen/history/selection reads. Explicit opaque targets, literal text after
--, strict duplicate/conflicting/unsupported option rejection and optional OS
activation preserve predictable behavior. Raw RPC remains available. Four CLI
tests passed; native socket smoke now renames and lists via command syntax.
Full cmux targeting/output compatibility and remaining options are still pending.


The macOS bundle now builds and carries rmuxctl alongside rmux, with an explicit
ad-hoc signature for the helper before sealing the app. No global install or
PATH change. The socket smoke accepts explicit app/client paths; the new
`mise run smoke-bundle-socket` exercised both copied debug-bundle executables
successfully, including discovery/error exit codes, shared mutations, reconnect
and endpoint cleanup. Trying-rmux now documents the packaged client and reflects
implemented socket/CLI support rather than listing them as entirely missing.


Added the standalone Rust `rmuxctl` client without GPUI/Ghostty dependencies.
It connects to explicit Unix socket paths or RMUX_SOCKET_DIR, validates JSON
object parameters, bounds request/reply allocation, preserves RPC replies and
returns distinct success/RPC/transport exit codes. No mutation retries. Two
focused tests passed, plus native socket smoke using the actual client for
capability discovery and unknown-method error handling. cmux command aliases,
ref/index targeting, caller scope and Windows transport remain pending.


Added opt-in Unix socket RPC using the existing bounded live dispatcher. Private
endpoint creation refuses existing paths; identity-checked cleanup preserves
replacements. Eight-client admission and read/write timeouts bound transport
resources. A focused endpoint unit test and native two-client smoke passed,
including shared Unicode rename, reconnect and normal-quit cleanup. Windows
transport, caller scope, subscriptions and cmux compatibility remain pending.


User requested progress screenshots from the main app rather than a preview
workspace. Switched the visual review workflow accordingly. Found an older debug
bundle still running, so refreshing the release bundle alone could not update
that visible process. Preserve existing terminal processes and open the current
release separately for review. Native screenshot capture still depends on the
Mac being unlocked; synthetic fixtures are not visual acceptance evidence.

Integrated the macOS native traffic-light controls with the 36px app header,
removing the separate stacked titlebar across new, detached and restored windows.
Reserved control space in the sidebar header or workspace header when the sidebar
is hidden. Title labels support double-click zoom and native AppKit drag; GPUI's
generic drag method is a no-op on macOS, so a scoped borrowed-window bridge uses
the current mouse event. Existing pinned objc/raw-window-handle dependencies are
reused; no tools were installed. Native session-restore smoke passed after the
titlebar change and compilation passed for the AppKit bridge. Actual traffic-light
placement, fullscreen behavior and mouse dragging still require visual/native
interaction acceptance. Linux/Windows retain their normal titlebar configuration.

Lightened the workspace frame: split separators now draw a centered 1px line
inside the existing 5px drag target, preserving resize geometry and keeping the
hover affordance. Removed the duplicate bottom New Workspace control; the header
add button retains its platform shortcut tooltip, menu and keyboard routes.
Terminal padding already defaults to 8px and was left configurable. Desktop
compilation passed via mise. Rendered divider alignment, hover and drag acceptance
remain pending.

Added an isolated native visual-preview harness and `mise run preview-ui` task.
The generated scene combines grouped workspaces, nested splits, long tab names,
synthetic branch metadata and unread notification content; overflow/compact
variants exercise crowded tabs and smaller windows. It requests fixed logical
geometry and uses temporary configuration rather than user recovery/session
files. Headless layout validation passed; native visual execution and screenshot
comparison remain pending. This supplies a repeatable acceptance scene, not
evidence that the current layout is polished.

Refined sidebar context layout: branch and directory now truncate independently,
with branch width capped at 45% of the row and the full values in a tooltip.
Added consistent branch/pin SVGs and moved pin status from an extra text line
into the workspace title row, reducing vertical noise. Context remains cached
and non-interactive so it does not steal workspace clicks. Desktop compilation
passed via mise; the layout still needs rendered narrow-sidebar acceptance.

Completed the prompt-driven Git refresh path with an explicit palette refresh
action and cached `git_branch` in surface discovery. The focused engine test
proves repeated identical OSC 7 reports remain observable. Extended native session
smoke changes a fixture HEAD to a Unicode branch without changing directory,
sends another prompt directory report and waits for the background cache to
update. Both checks passed through mise, including the existing recovery and
cleanup scenarios. Native visual placement and manual action invocation remain
pending; no file watcher or idle timer was added.

Started T09 sidebar Git context: branch and detached-HEAD labels appear beside
the active/remembered terminal's directory. Discovery runs off the UI thread on
directory reports, reads at most 4096 bytes per metadata file and traverses at
most 64 ancestor directories. `.git` indirection supports linked worktrees.
One request per terminal runs at a time with one coalesced follow-up; stale-path
results are discarded. No Git subprocess or polling timer is added. Focused
tests passed for nested repositories, Unicode branch names, worktree indirection,
detached HEAD and oversized metadata. PR/port information, refresh outside shell
directory reports and native visual acceptance remain pending.

Added the reference sidebar's notification context: clear blue/white unread
badges and two-line message previews, including hidden group members. Previews
use cached unread, non-dismissed messages in pane/member order and are gathered
only while the sidebar is visible. They neither query terminal engines nor
acknowledge notifications. Adjusted selected group-summary contrast. Desktop
compilation passed via mise; native screenshot access was rechecked and remains
locked. Rendered clipping/spacing and visual acceptance are still pending.

Continued the screenshot-led visual pass with a coherent embedded SVG icon set
for add, close, disclosure, copy, refresh, split, sidebar and notification controls.
Replaced inconsistent font glyphs and CSS-drawn split icons, keeping established
hit areas and tooltips. The workspace header now has direct sidebar/notification
controls and a compact semibold title. Icons are built into the executable via
GPUI's asset source; no runtime disk reads or icon fonts are needed. Native
screenshot comparison remains blocked by the locked Mac; no visual parity is
claimed from compilation alone.

Prioritized the user's explicit cmux UI/UX parity request over socket work.
Inspected the official landing screenshot and recorded the visual hierarchy in
`docs/reference/visual-parity.md`. Applied a coordinated first pass to neutral
chrome, selected workspace contrast, typography, tab height, sidebar separator
and header add control. Native rendered comparison is still required; a locked
Mac prevents claiming visual acceptance. The intended follow-up is screenshot-led
layout/interaction refinement, not another unrelated diagnostics feature.

Produced a refreshed optimized macOS app at `target/release/rmux.app` with all
current workspace/UI work. The local bundle's strict signature verification and
live-shell RPC smoke against its actual executable passed, covering identity,
Unicode I/O, cross-window transfer/state retention, zoom, bounded reads and owned
process cleanup. Executable size is 9.1 MiB (not runtime RSS); SHA-256:
`e235bc9339f5406cd820248830802e5ab60cbb72671626a06437b0ec69dccdcb`.
Added `mise run open-app`, `mise run smoke-release` and a concise trying-rmux guide
separating implemented capabilities from acceptance gaps and missing P2 work.
No visual, OS activation, latency or full phase acceptance is claimed. Next work
should deliver coherent daily-workspace/automation batches rather than additional
diagnostic polish.

Added notification count and Clear all to the notification-panel header, plus a
Dismiss All Notifications palette action. The operation snapshots each current
message revision and uses existing per-terminal guarded dismissal to clear its
unread marker without focusing terminals or suppressing future revisions. It
walks terminals once and retains entity handles for updates, avoiding repeated
workspace searches. Desktop compilation passed through mise; native dismissal,
new-message and layout acceptance remain pending.

Aligned Next Unread Terminal and notification-list ordering with logical sidebar
order, including pinned groups and collapsed members. Unread navigation checks
the current workspace first and wraps once, then uses the shared direct terminal
focus path for zoom, remembered pane and group visibility without acknowledging
an unrelated previously active tab. Removed the duplicate focus implementation.
Desktop compilation passed through mise; native unread ordering/acknowledgement
acceptance remains pending.

Fixed tab transfer selection to reveal collapsed destination groups for both
same-window and cross-window moves. Move Tab to Next Workspace now follows the
logical sidebar ordering (including group members) instead of backing-vector
order. Extended the native session fixture with a transfer into a collapsed
child workspace and back, checking expansion and retained process identity.
`mise run smoke-session-restore` passed. The new collapsed cross-window transfer
case and keyboard invocation of next-workspace movement still need native
acceptance; rendered focus remains unverified.

Fixed direct terminal navigation leaving its workspace hidden in a collapsed
group. Terminal focus now shares workspace selection's group-reveal behavior,
before scrolling the sidebar, while preserving an anchor's intentional collapsed
state. The extended native session fixture focuses a child workspace's terminal
and verifies group expansion, selected workspace and zoom following the target
pane. `mise run smoke-session-restore` passed, including existing restore,
recovery and owned-process cleanup scenarios. OS activation and rendered focus
remain unverified; the fixture proves application state through live RPC.

Added direct cross-window workspace switching to the command palette alongside
terminal switching. Window-labelled results resolve stable workspace IDs,
validate the destination's availability, restore its normal remembered-pane
focus and activate its window. Closed/moved/busy targets produce a notice before
changing destination selection. Desktop compilation passed through mise. Native
focus restoration and OS activation acceptance remain pending.

Added cross-window terminal switching to the command palette. Results include
other windows' workspace/pane/tab labels and cached directories; choosing one
uses stable window/workspace/terminal identities to focus the existing process
and activate its window. Destination availability and membership are checked
before focus changes, with a visible notice for closed/moved/busy targets.
Desktop compilation passed via mise. Native multi-window focus, OS activation
and stale-target interaction acceptance remain pending.

Extended directory-based terminal switching to the command palette across all
workspaces in the current window. Terminal results now have a muted directory
subtitle, full title/path tooltip and directory-inclusive fuzzy search. Other
commands retain single-line rows and shortcut badges. Paths are copied from the
existing cache when opening the palette; switching still uses stable workspace
and terminal identities. Desktop compilation passed through mise. Native layout
and interaction acceptance remain pending.

Added cached working-directory subtitles to the tab picker so identical shell
titles can be distinguished. Queries now match words across title and directory;
full paths remain available in tooltips when the subtitle is truncated. Rendering
and filtering use cached shell-reported/initial paths, with no parser lock,
filesystem access or polling. Desktop compilation passed via mise. The command
palette already had an explicit empty state and guarded no-result activation;
no changes were needed there. Native visual acceptance remains pending.

Added title search to the pane tab picker using the existing Unicode composition
text field. Case-insensitive query words filter titles while original tab numbers
and unread markers remain visible. Matching counts and an empty-result state are
shown; Enter on no results is a no-op. Arrow/page navigation shares the palette's
bounded selection helper and leaves active composition alone. Numbers now enter
the query, and Home/End edit it; direct pane-level tab shortcuts are unchanged.
Desktop compilation passed via mise. Native UI access was rechecked and the Mac
remains locked, so visual/IME/keyboard acceptance is still pending.

Improved the crowded-pane tab picker with visible tab numbers, a tab count and
keyboard hints. Plain 1–9 opens the corresponding tab through the existing
selection path; missing numbers are no-ops. Page Up/Down moves the highlight by
six rows with bounds clamping and scroll-into-view, preserving Enter-to-open and
Escape-to-cancel. Desktop compilation passed via mise. Native picker layout,
keyboard routing and visual comparison remain pending acceptance.

Optimized process sampling to refresh the requested PID set in one batch instead
of enumerating the host process table for each PID. Incomplete batches fall back
to individual memory probes and reset CPU baselines, preserving exited-process
handling without computing CPU twice over an invalid interval. The focused
sampler checks and benchmark passed; observed debug warm medians fell to
0.783/0.963/2.722 ms at 1/8/32 shells plus the app. The benchmark document records
both runs and limits; this is not an input-latency or release acceptance claim.

Measured process sampling with an opt-in Unix fixture and mise task at 1/8/32
blocked shells plus the application PID. Debug-build warm medians were
1.298/4.837/12.052 ms; five spaced samples per case returned finite nonnegative
CPU after unavailable first samples. Fixture processes are reaped even on panic.
See `docs/benchmarks/process-sampling.md` for scope and limits. This establishes
sampler cost, not release or input-latency acceptance; full-cap and descendant
sampling still need work.

Added descending screen-storage, shell-RSS and shell-CPU sorting to the memory
panel, available through an inline control and command palette. Missing samples
sort last, and copied JSON follows the displayed order. Sorting uses references
to at most 256 existing rows, without process resampling or copying terminal
data. The selected ordering survives refreshes, including an in-flight refresh.
Desktop compilation passed through mise; native interaction/visual acceptance
remains pending.

Added application-process RSS/CPU to the memory panel summary and copied reports.
The app PID is sampled alongside up to 256 shell PIDs on the existing manual
background refresh; the summary explicitly covers all windows. Closing clears
the displayed sample, and existing generation checks prevent late refreshes
from restoring it. Documentation distinguishes OS process RSS from GPU allocation
totals and warns against adding screen storage to RSS. Desktop compilation passed
via mise. Native visual acceptance and sampling performance remain unverified.

Added root-shell RSS and CPU to the manual terminal-memory panel and copied JSON
reports using the already pinned sysinfo dependency. Sampling stays on the
background executor and is bounded to 256 requested PIDs, with no polling timer.
PID-set changes reset the cache and CPU baseline; first/too-close refreshes leave
CPU unavailable. Each PID refresh must succeed before cached data is reported,
preventing exited shells from appearing live. Two focused desktop tests passed
through mise, covering positive RSS, unavailable first CPU, cache reset and a
sampled child that exits. Descendant/helper attribution, CPU accuracy/performance
measurements and native panel visual acceptance remain pending.

Improved the memory panel with descending resident-storage sorting (unavailable
samples last), click-to-focus rows using stable workspace/terminal IDs, and a
Copy JSON report control. Reports contain raw byte counts, scope, sample mode,
identity and availability rather than turning failed samples into zeros. Clicking
a stale row cannot target a replacement terminal. Sorting runs with sampling on
the background executor; report serialization occurs only on explicit Copy.
Desktop compilation passed via mise. Native row navigation, clipboard and visual
acceptance remain pending.

Added a terminal-memory panel through View and the command palette, with manual
refresh and close controls. It samples up to 256 terminals in the current window
on the background executor, showing workspace/tab names, PIDs, resident,
compressed, image and virtual storage plus explicit busy/closed states. The
summary counts successful samples rather than presenting busy entries as zero.
No polling timer was added, and generation checks discard results after close or
reopen. Desktop compilation passed via mise; native panel layout/interaction is
unverified while the Mac is locked. CPU/process RSS and richer task-manager
controls remain pending.

Started task-manager memory diagnostics with on-demand `surface.memory_usage`
RPC. UI lookup creates a weakly owned one-shot sample; Ghostty memory inspection
runs on the background executor and returns busy rather than waiting on parsing.
The response explicitly scopes primary/alternate resident, virtual, compressed
and image bytes to Ghostty screen storage, not application RSS or child memory.
Focused session contention test and extended native live-session smoke passed
through mise. No idle sampler or timer was added. UI, process CPU/RSS attribution
and refresh controls remain pending.

Verified selection export under parser contention and across the session/thread
boundary. The focused session test returns busy without waiting on a held parser
lock, confirms creating the queued exporter adds no strong engine owner, and
reads selected text on another thread without changing the selection. It passed
in 0.02s. Extended native live-session smoke also passed through mise: no-selection
RPC returns empty text with correct scope/truncation, alongside existing history,
viewport, identity, shell-state and cleanup checks. Native nonempty selections,
terminal-close timing and graphical screenshots remain separate acceptance work.

Started terminal selection-read parity: `surface.read_text` now accepts
`scope: "selection"`. Selection formatting uses the same bounded streaming writer
as history export, preserves UTF-8, caps output at the requested limit and runs
off the UI thread with a nonblocking engine-lock attempt. No selection returns
empty text with `truncated: false`; busy terminals retain the retry error. Four
focused export tests passed via mise, including selection/viewport preservation
and truncation in a multibyte character; desktop compilation passed. Native
selected-text RPC interaction and screenshot capture remain pending.

Added Close Workspace Group to the File menu and command palette. Multi-member
groups use one explicit confirmation naming the group and workspace count;
single-member groups use the existing workspace close policy. Confirmation
captures member IDs and closes only those still in the same group, with the
anchor last, through normal workspace/process cleanup. Cancel preserves the
group, and the dialog points to Ungroup for keeping terminals. Desktop
compilation passed via mise; native dialog/confirmation acceptance is pending
while the Mac remains locked.

Retried native UI access: the Mac is still locked and automatic unlock failed,
so visual review remains unavailable. Continued with group-create failure
rollback: when no shell/pane starts, discard the empty entry directly and restore
selection, previous-workspace history, title and numbering instead of running
normal close (which could focus a neighbor or expand its group). Extended native
smoke passed via mise: an invalid cwd with two existing workspaces returned an
error and left complete window/group/workspace discovery unchanged. Existing
creation, placement, directory, recovery and child cleanup checks still pass.

Added opt-in generated-anchor cleanup to ungroup RPC. Eligibility requires
explicit generated provenance and sole membership; user-selected/promoted
anchors and groups with children are rejected before mutation. Existing close
policy is honored, returning a confirmation-required error instead of bypassing
it. Native smoke passed for ordinary user-anchor rejection, memberful-group
rejection and cleanup of a disposable generated anchor while all eight other
terminals remained present; tracked children were reaped. A focused provenance
test passed via mise, including promotion losing cleanup eligibility. Native
confirmation behavior remains separate acceptance work.

Group-create RPC now accepts an absolute `cwd` for fresh anchors and rejects
combining it with existing workspace IDs. The explicit path flows directly to
shell construction without changing global startup directory state. Native smoke
passed via mise: a generated anchor and three members actually started in a
separate Unicode project directory, while original terminals remained in their
own directory. External-ID retry still starts only one anchor. Focused parser
checks reject relative cwd and empty/duplicate workspace lists before UI work.

Ungroup now materializes logical sidebar/member order before removing the group
container, preserving active workspace identity and individual workspace pins.
Both palette/File actions and RPC use the same path. Extended native smoke
passed via mise: a group created in reverse storage order was flattened in its
displayed order, selection stayed on the same workspace, and terminal IDs/PIDs
remained unchanged. Existing placement, cwd inheritance, restore and cleanup
checks also passed. Native visual appearance remains unverified.

Group headers now aggregate unread state across their workspaces, including
collapsed members, and show a compact workspace/unread count. The existing
attention dot lights for any unread member. Rendering reads each workspace's
cached attention state once and uses a set for group membership checks; it does
not acquire terminal parser locks or acknowledge notifications. Existing metadata
subscriptions drive redraws. Desktop compilation passed via mise; native visual
notification/collapse acceptance remains unverified.

Added sidebar multi-selection using Cmd-click on macOS / Ctrl-click elsewhere,
plus Shift-click ranges over visible rows. The header shows the selection count
and a shortcut hint. Group Selected Workspaces is available through File/menu
palette and Cmd-Shift-G (Ctrl-Alt-Shift-G elsewhere); it uses logical sidebar
order, creates no terminal, and promotes source-group anchors as needed. The
shortcut propagates when fewer than two workspaces are explicitly selected.
Closed workspaces are pruned from selection; plain clicks reset it. Desktop
compilation passed via mise; native modifier-click and shortcut interaction
remain unverified.

Added `workspace.group.new_workspace` RPC with explicit placement overrides and
configured fallback. It uses the group's anchor as the placement/cwd reference,
selects the new workspace, enforces the 64-workspace window limit and reports
shell-start failure after removing an empty failed workspace. Extended native
smoke passed for end/top/after-current order, fresh shell counts and actual
working-directory inheritance recorded by every fixture shell. The first cwd
assertion compared lexical paths and failed on macOS path aliases; resolving
both paths fixed the assertion, and rerunning the smoke passed including child
cleanup and recovery. Native visual interaction remains unverified.

Added the reloadable `group_new_workspace_placement` preference with
`after_current` (default), `top` and `end`. Cmd-N and header `+` now route through
one placement function; absent/stale after-current references fall back below
the anchor. Focused placement and configuration tests passed via mise, covering
all three modes, anchor preservation, backward defaults and unknown-value
rejection. Per-directory/per-group overrides and native placement UI acceptance
remain pending.

Added group-create RPC for explicit existing workspace lists and fresh generated
anchors. All supplied members are resolved before regrouping; duplicate/empty
lists are rejected. Optional external identity is persisted and window-scoped;
retries resolve it before spawning or modifying groups. Native smoke passed for
existing-member creation/retry, fresh-anchor creation/retry (exactly one new
terminal), and persisted external identity through automatic recovery. Existing
terminal IDs/PIDs survived regrouping and cleanup passed. Nine focused group
tests passed, including external identity mapping, uniqueness and validation.
Explicit cwd override, cleanup of generated anchors and native visual acceptance
remain pending.

Added live group add/remove/set-anchor RPC with same-window checks, membership
validation, busy guards and stable selection/anchor-slot handling. Removing the
last member returns a null group; adding an existing member is idempotent.
Extended native smoke passed: moved the second fixture workspace into the first
window, added it to the group, made it anchor, removed it and verified promotion
of the original anchor, then ungrouped. All four terminal IDs/PIDs remained
unchanged. Automatic restore/capture/forced-stop checks also pass. Focused
parameter rejection tests passed via mise. Native visual focus and group-create
RPC remain pending.

Added `workspace.group.update` (name/collapsed/pinned) and
`workspace.group.ungroup` to live JSON-RPC with strict parameter validation,
window-scoped target lookup, busy-state guards and notification routing. Ungroup
returns member IDs and keeps terminals alive. Native session smoke passed via
mise: updates reflected in discovery, ungroup removed membership, and terminal
IDs/PIDs were unchanged. Automatic group recovery checks still pass. A focused
parser test passed for malformed IDs, empty updates/names, invalid booleans and
unexpected ungroup options without queueing UI work. Creation and membership
RPC remain pending.

Exposed groups through live RPC discovery. `window.list` includes ordered group
records with window-scoped handles, names, style, pin/collapse/provenance,
anchor IDs and ordered member IDs. `workspace.list` reports nullable group IDs,
anchor flags and indices in logical sidebar order (including collapsed members).
Extended native session smoke passed through mise for explicit restore and
forced-stop relaunch: discovery matched live workspace IDs, Unicode group style
and persisted state. Group mutation RPC methods remain pending; visual UI
acceptance is still separate.

Unified sidebar ordering and keyboard navigation through a tested group-order
projection. Next/previous workspace, numbered workspace actions, sidebar shortcut
labels and palette workspace numbering now follow pinned/group/member order.
Collapsed children retain their numbers and remain reachable; selecting one
expands its group. This avoids shortcut renumbering when a header is collapsed.
Eight focused group tests passed via mise, covering mixed pins, member pins,
collapsed versus navigation order, and reordered top-level groups. Native
keyboard/visual interaction remains pending.

Preserved the sidebar slot of a group when its anchor closes, detaches, is
removed/regrouped, or is explicitly replaced. A live previous anchor trades
entry positions with its replacement; closing an anchor moves its replacement
into the vacated slot while retaining surrounding standalone row order. Active
workspace identity is restored after those changes, and closing the active
anchor selects its promoted member. Seven focused group tests passed through
mise, including replacement and closure with member storage on either side of
the header. Desktop compilation passed; native close/focus interaction remains
unverified.

Top-level workspace/group keyboard ordering now follows visible header rows and
uses each group's independent pin state. Header drag moves the group at its
anchor position, including relative to other group headers, without merging
members. Pinning projects rows into pinned/unpinned tiers without rewriting
underlying order; restore retains saved entry order instead of sorting by
individual workspace pins. Cross-tier drops return without changing membership.
Desktop compilation passed via mise. Native mixed-row ordering, anchor-promotion
position stability and shortcut-number order still need acceptance/refinement.

Workspace Up/Down now reorders group children within their member list, clamping
at the first child so ordinary movement cannot replace the anchor. Sidebar
workspace drops onto grouped rows place a child after the target, including
cross-group and standalone-to-group moves; dropping a child onto a standalone
row removes its membership before normal ordering. Anchor drops into groups
produce a notice instead of implicitly merging groups. Six focused model tests
passed through mise, including forward/backward placement, cross-group transfer
and anchor preservation. Native drag hit targets and full top-level group/pin
ordering still need acceptance and implementation work.

Added independent group colors and icons to saved state and sidebar headers.
File-menu/palette editors reuse workspace color swatches, icon presets, custom
entry and validation, with Enter/escape behavior retained. Group styling belongs
to the container and survives anchor changes; headers no longer borrow the
anchor workspace's style. Older saved groups default to no custom style. Desktop
compilation and six focused group/model-format tests passed via mise, including
custom-style round-trip, member-index mapping, backward defaults and invalid
color/icon rejection. Native visual/editor interaction remains unverified.

Added palette destinations for moving the active workspace into an existing
group, plus File-menu/palette actions to remove only the active member or make
it the anchor. Destinations include stable IDs to distinguish duplicate names;
the current group is omitted. Membership changes use existing entities and do
not spawn, move or terminate PTYs. Moving an anchor promotes a source member;
moving the last member removes its empty source group. A new focused test covers
destination identity after source removal, invalid destinations, same-group
idempotency and explicit anchor provenance. Four group-model tests and desktop
compilation passed through mise. Native palette interaction remains unverified.

New group members now inherit the anchor workspace's cached working directory;
new empty groups inherit the active workspace's directory without temporarily
joining or expanding its group. Ordinary new workspaces also inherit the active
workspace's cached directory. Cached OSC/launch metadata avoids acquiring the
terminal parser lock. Explicit-directory spawning shares the existing workspace
pane constructor. Restoring now reapplies saved collapse flags after focus setup,
and scrolling a hidden member targets its anchor header. Desktop check and the
extended native session smoke passed: a collapsed, pinned anchor-only group with
a Unicode name and stable identity survives automatic checkpoint, forced-stop
relaunch and normal Quit. Native cwd inheritance and multi-member collapsed
restore interaction still need direct acceptance coverage.

Added the first workspace-group sidebar UI: anchor headers replace the anchor's
ordinary row, members are indented, chevrons collapse/expand without changing
terminal focus, and header `+` creates a member after the anchor. Cmd-N inside a
group inserts after the active member. New Empty Workspace Group is available
with Ctrl-Cmd-G on macOS / Ctrl-Alt-G elsewhere; palette and File-menu actions
also create a group from the current workspace, rename, pin and ungroup. Header
double-click edits the group name. Group pins determine the top-level tier and
visible-row mapping drives workspace scrolling. Desktop cargo check passed;
native appearance/interaction remains unverified. Anchor cwd inheritance,
multi-selection, membership drag/drop, configurable placement and automation
still need implementation; current group creation uses normal new-shell cwd.

Connected group ownership to window state and layout/session persistence. Saved
groups carry stable group IDs, ordered workspace indices (anchor first), names,
collapse/pin state and generated-anchor provenance. Restore resolves indices
before workspace pin sorting; capture resolves current live identities back to
indices. Older files default to no groups. Validation rejects missing, duplicate
or multiply owned members, empty groups and invalid identities before shell
preparation. Closing/detaching a workspace now updates its source group and
promotes an anchor as needed. Existing 16 snapshot tests and four focused group
tests passed through mise. Sidebar rendering/actions and native group recovery
acceptance remain pending; groups are not yet exposed in the UI.

Started W08 workspace groups with a window-local ownership model in
`workspace_groups.rs`, based on the pinned cmux group contract. Ordered members
define the anchor; removing it promotes the next member and clears generated
provenance, while preserving group identity and collapse/pin state. Regrouping
keeps ownership exclusive, validates before mutation, and removes empty source
groups. Ungrouping returns members without touching terminals. Two focused tests
passed through mise, covering promotion, invalid operations, regrouping and
same-name groups with distinct identities. This is model groundwork only:
sidebar integration, saved sessions and automation remain next steps; no group
UI or full W08 completion is claimed.

Added optional zoomed-pane persistence using depth-first layout indices. Older
files default to unzoomed; invalid zoom indices are rejected. Restore focuses
the visible zoomed pane while retaining the full split tree and weights. The
focused format test and native session smoke passed: explicit and automatic
restore retained zoom, forced-stop/relaunch retained it again, and unzooming
returned the unchanged 1:3 split tree. Actual visual focus remains unverified.

Verified partial restore rollback with real PTYs. Preparation now accepts an
internal shell factory, allowing deterministic later-spawn failures without
changing environment variables. A focused test fails in the first and second
windows and cancels after the first spawn; each case confirms every previously
prepared shell is reaped before the error returns. The production path still
uses the existing shell constructor. The test passed in 0.24s. OS/window-opening
failures and native cancellation dialogs remain separate acceptance work.

Constrained the tab picker to the pane's actual available height using an inset
flex container, retaining its 240px cap and scrolling for overflow. This avoids
relying on stale terminal bounds during split resizing. Added full-title tooltips
for truncated picker rows. Desktop cargo check passed for the containment change;
native preview was retried but the Mac remains locked, so visual acceptance is
still pending.

Improved window notices with a bounded scrolling text area, consistent spacing,
background/border contrast, Copy and Dismiss controls. Active restore notices
retain Cancel instead of Dismiss, and action controls do not shrink behind long
messages. Desktop cargo check passed; native narrow-window/clipboard/scroll
appearance and interaction acceptance remain pending.

Made recovery failures visible in the fresh window and disabled automatic writes
after failed restoration, preserving the original file for repair instead of
overwriting it with fallback state. Lock failures also produce a notice. Native
smoke passed for malformed JSON and unsupported session versions: both launched
one fresh window, exited normally, and retained the original file bytes. Existing
restore/checkpoint/lease/forced-stop checks passed. Notice appearance remains
visually unverified while native UI access is unavailable.

Added optional window geometry to saved layouts/sessions, including windowed,
maximized and fullscreen modes. Older files retain default placement. Restore
prefers the display with greatest overlap and clamps bounds to an available
display, keeping windows reachable after monitor removal. Window bounds changes
now trigger coalesced recovery checkpoints. Focused fitting and format-validation
tests passed; native restore/crash-relaunch smoke passed and confirmed valid
windowed geometry in captured files. Exact visual placement, display changes,
maximized/fullscreen runtime behavior and platform acceptance remain pending.

Added an exclusive OS-backed lease for configured recovery files. A sibling
.lock file survives atomic checkpoint replacement; startup holds the lease
before restore, and background writers retain it through writes. A competing
launch falls back to a fresh window with recovery disabled. Focused tests passed
for exclusion and last-owner release. Native smoke confirmed a second process
does not restore/overwrite the owner's two-window checkpoint, then SIGKILL/relaunch
reacquires the lease and restores successfully. All fixture shells exited.
Cross-platform lock/runtime acceptance remains pending; Windows validation stays
deferred per user direction.

Fixed a checkpoint notification gap: font size/family changes now emit terminal
metadata events, reaching the observed pane/workspace path. Previously they only
notified TerminalView, which recovery deliberately does not observe per frame.
Desktop cargo check passed; native font-change/relaunch acceptance remains pending.

Verified recovery without normal Quit. The automatic native smoke waits for a
completed rename/directory checkpoint, SIGKILLs only its isolated test rmux,
confirms checkpoint bytes remain unchanged, and launches a new process from the
same configured file. Two windows, workspace names, split weights and tab labels
return with four distinct fresh shell PIDs; the Unicode reported directory
survives the subsequent normal-quit checkpoint. Cleanup verifies both generations
of fixture shells exit. The extended smoke passed against the existing binary.
Power-loss and interrupted-replacement behavior, uncheckpointed recent changes,
visual/OS focus and high-output capture latency remain unverified.

Removed terminal parser locking from layout/checkpoint directory capture. Each
TerminalView now retains the last delivered directory metadata as a PathBuf;
snapshots clone that cache instead of calling Session::current_directory. An
explicit spawn directory seeds the cache until the first OSC directory report,
so shells without integration no longer lose their requested directory on save.
Subsequent remote/cleared reports clear the fallback. The native session smoke
passed with assertions for all four initial directories and a later OSC 7 report
containing Unicode/spaces reaching the automatic checkpoint before Quit. Capture
uses delivered metadata rather than forcing a synchronous engine read. Large
workspace capture timings and remote-directory clearing acceptance remain pending.

Added opt-in event-driven checkpoints observing window/workspace/pane changes,
coalesced through a one-slot channel and a 750 ms timer that runs only after a
notification. No terminal-frame observer or idle polling is installed. A shared
background writer serializes automatic and quit saves, skips identical encoded
state, and uses generations to reject stale jobs; deduplication retains at most
one 8 MiB encoded session. Busy capture waits for another notification rather
than polling. Native restore smoke now renames a workspace and verifies the file
changes before Quit, then verifies the quit checkpoint/cleanup; both explicit and
automatic cases passed. A focused stale-writer regression covers generation
ordering. Forced-crash/relaunch, busy-interaction retries, output-pressure capture
cost and complete change-source coverage remain pending.

Connected opt-in automatic startup recovery and quit checkpoints through an
absolute session_file setting, frozen at startup. Normal launches recover it;
explicit layout/session/directory/command and benchmark launches do not touch it.
Unrestorable files fall back to a fresh window. Normal Quit captures before
shutdown and awaits background atomic save inside the shared three-second
cleanup deadline; closing all windows records an empty session. OS quit remains
best effort under GPUI's shorter deadline. Added system.quit RPC using the normal
confirmation flow. Native explicit and automatic two-window restore smoke passed;
automatic Quit wrote a newly captured checkpoint and all shells exited. Parent
directory creation, change-triggered/crash checkpoints, corrupt-file fallback
runtime coverage and native confirmation/OS-quit acceptance remain pending.

Added File/command-palette Save Session to produce the multi-window recovery
format from live window state. Capture preserves window enumeration order and
the active-window index, uses shared layout capture, excludes benchmark windows,
and rejects busy editing/restore/close interactions before showing the save
picker. Serialization and atomic replacement run on the background executor;
the saved state is the action-time snapshot. Desktop cargo check passed with no
remaining unused session-writer warning. Native save-dialog and captured-session
round-trip acceptance remain pending, as does automatic checkpoint scheduling.

Verified multi-window session startup through a new isolated native smoke and
mise smoke-session-restore task. Headless inspection starts no shells; an invalid
directory in the second window returns exit 2 before any fixture shell starts.
Valid startup restores two windows, Unicode workspace names, metadata/pins,
1:3 vertical split weights, tab labels and selected tab. Four distinct live shell
PIDs are observed and all exit after cleanup. Both the initial smoke and extended
preflight rerun passed; executable evidence is in
docs/benchmarks/session-restore-smoke.md. Visual/OS activation, active-pane focus,
partial-spawn rollback and automatic checkpoint scheduling remain pending.

Connected multi-window recovery files to startup with --session PATH and the
headless --check-session PATH. Preparation validates all windows/directories
before spawning shells and shares the existing cancellable preparation path;
failure in a later window drops and waits for previously prepared shells.
Startup opens saved windows in order and activates the recorded window (first
if none); an empty session opens a fresh default window. Live stdio RPC can be
combined with session startup. Existing CLI tests and a focused session-mode
conflict test passed. Native multi-window restoration and rollback fixtures are
still pending. Automatic checkpoint capture/scheduling is not yet connected.

Added a versioned multi-window SessionSnapshot format for recovery, preserving
window order and optional active-window index alongside existing layouts. Loading
is capped at 8 MiB, sessions at 16 windows, and the shell budget is shared across
all windows (256 total tabs). Empty sessions are valid. Window and session saves
share an atomic same-directory temporary-file replacement with file sync; invalid
state is rejected before replacing an existing checkpoint. Fourteen focused
snapshot tests passed, including multi-window round trip, prior-checkpoint
preservation, aggregate shell-budget rejection and oversized/version validation.
This is storage infrastructure only: automatic capture/write scheduling and
startup recovery are not wired yet. It restores fresh shells, not live processes,
and does not claim directory-fsync/power-loss durability.

Prepared layout capture for reuse outside the manual Save dialog. Window capture
is now a separate layout_snapshot method, and saved active-pane indices are
derived directly from depth-first layout order. Existing split/swap operations
already maintain matching pane-vector order, so this removes an implicit
dependency rather than fixing a demonstrated current restore failure. A focused
nested-tree swap/removal index test and desktop cargo check passed. Automatic
multi-window persistence, startup recovery and crash-safe scheduling remain
unimplemented; manual save behavior is retained.

Made the tab picker reachable through the command palette and a scoped terminal
shortcut (Cmd+Shift+T on macOS, Ctrl+Alt+Shift+T elsewhere), with matching control
tooltips. Added Home/End navigation and a picker-owned blur subscription so
switching focus/workspaces cannot leave a hidden picker blocking automation.
Explicit selection/dismissal releases that subscription; picker opening is
idempotent and refuses to interrupt tab rename/close prompts. Desktop cargo check
passed. Native shortcut routing and focus/overflow visual acceptance remain pending.

Added a compact tab picker to multi-tab pane headers. It lists tab titles with
active/unread indicators in a bounded scrolling overlay, selects by stable
terminal ID on click, and supports arrow navigation, Enter to choose and Escape
to restore terminal focus without changing tabs. Selecting a tab reveals it in
the existing horizontal strip. Outside clicks dismiss the picker, and automation
mutations treat an open picker as an active interaction. Desktop cargo check
passed. Native overflow, focus, narrow-pane placement and keyboard acceptance
remain pending while native UI access is unavailable.

Verified the last-tab cross-window transfer lifecycle through the native RPC
harness. A sixth fixture shell sets a Unicode variable, moves its sole tab into
an existing workspace, retains its PID/state, and loses only its empty source
workspace/window. The removed destination ID subsequently returns not-found.
All six fixture children exited after app termination. The harness now prints
its passed report only after cleanup assertions succeed. The focused live run
passed in under a second without rebuilding the unchanged application binary.
Busy/restore destinations and actual visual/OS-focus acceptance remain pending.

Added live surface.move_to_workspace with explicit surface/workspace IDs and
optional window activation, sharing the palette's transfer implementation.
Both same-window and cross-window destinations are supported; same-workspace
requests are no-ops. The native live-shell smoke initially caught an incorrect
terminal-ID-as-pane-ID lookup that crashed the new path; replaced it with direct
owner lookup and reran successfully. A multi-tab terminal crossed windows and
returned with the same identity/PID, destination selection and stale-ID errors
verified. All five fixture shell variables survived the complete sequence, and
cleanup reaped them. Desktop build passed. Last-tab source-window closure,
destination busy/restore cases and actual visual/OS-focus acceptance remain pending.

Added command-palette destinations for moving the active terminal tab into a
named workspace in another existing window. Live destination validation rejects
stale windows/workspaces and editing/restore/close states before source removal.
The transfer retains the terminal entity/PTY and label, rebinds window focus and
input subscriptions, targets the destination's remembered pane, follows zoom,
selects only the transferred tab, and removes an empty source pane through normal
workspace cleanup. Desktop cargo check passed. Native UI inspection was attempted
but the Mac is locked and automatic unlock failed; cross-window tab PID/state,
last-tab cleanup, stale-target and visual acceptance remain pending.

Added visual preset choosers inside workspace color/icon editing. Color swatches
and icon buttons reuse the existing menu presets, wrap to the sidebar width,
show the current draft selection and named tooltips, and include a clear option.
Choosing a preset updates the text draft through its undo path and restores text
focus; Enter commits and Escape cancels through the existing metadata editor.
Custom hex colors and glyph entry remain available. IME composition is not
overwritten by a preset click. Desktop cargo check passed; native visual,
pointer/keyboard and undo acceptance remains pending.

Added a reproducible release history-export benchmark and mise bench-history
task. A 100×32 mixed-Unicode fixture retained 4,519/77,194 history rows under
4/64 MiB budgets. With 64 MiB retention, the 1 MiB RPC cap took 2.324 ms warm p50
and 3.191 ms first access after compression; exporting the entire 5.48 MB text
took 12.550 ms warm p50 and 15.906 ms first access. Default 64 KiB reads stayed
below 0.32 ms in this run. Results and limitations are recorded in
docs/benchmarks/history-export.md. Retain the RPC cap; whole-app concurrent
render/input latency and allocation peaks remain unproven.

Connected full-history export to live surface.read_text with optional
scope:"history"; viewport remains the default. UI lookup creates a weak,
one-shot session job without locking the engine. A background executor performs
the bounded export, returns retryable busy errors on parser contention, and does
not retain a PTY/session while queued. The extended contention regression passed;
native live-shell smoke passed with Unicode soft-wrap reconstruction, history
byte truncation and invalid-scope rejection, alongside existing transfer/PID/state
checks. Desktop build passed. History output is capped at 1 MiB through RPC.
The formatter still holds the engine mutex during traversal; background execution
does not prove large-export rendering latency. Incremental contention mitigation
and quantitative large-history measurements remain pending.

Added bounded full-history plain-text export to the terminal core using Ghostty's
streaming formatter. It reads the active screen's retained history oldest-first,
unwraps soft wraps, preserves selection and viewport, and stops formatter output
at a caller-specified limit capped at 16 MiB. Truncated output ends on a UTF-8
boundary. Buffer capacity requests grow geometrically within the export limit,
avoiding a reallocation for every character callback. Three focused tests passed:
history/selection/viewport/alternate-screen behavior, every byte boundary of a
multibyte wrapped fixture, and 10,000 single-byte callbacks with at most three
capacity growths. This is a synchronous core API only; background session/RPC
integration and large-history responsiveness remain pending. Existing RPC text
reads still export the viewport only.

Added a focused PTY contention regression for snapshot reads and targeted input.
A separate thread holds the terminal parser mutex with pending output; try_frame
returns busy and input_without_scroll still accepts input before lock release.
Afterward, the shell receives the Unicode input, pending output remains visible,
the earlier snapshot stays immutable and the shell exits. The test passed in
0.02s. Its bounded hold/500ms guard detects accidental blocking; it is not an
interactive latency benchmark or proof of flood-output performance budgets.

Added surface.read_text for fresh bounded viewport snapshots, with dimensions,
scroll offset, explicit truncation and retryable busy errors. Session::try_frame
does not wait for the parser lock. Frame text extraction now streams directly
into one output string, preserving UTF-8 boundaries and trimming visual row ends
without intermediate per-row strings/vectors. Focused multibyte/spacing/limit
tests passed. The native live-session read check initially exposed a fixture
assumption that soft-wrapped output is contiguous; the fixture now accounts for
documented visual row breaks and passed, including Unicode output, byte limits,
invalid limits and all transfer/state checks. Full-scrollback export and sustained
snapshot-read latency remain pending.

Added surface.send_text with bounded nonempty UTF-8 payloads (16 KiB), identity
targeting and explicit queue-acceptance/error responses. It uses the existing
bounded writer without scroll-follow or terminal-engine locking, and does not
change focus/selection. The native live-session smoke delivered distinct Unicode
state values to all five fixture shells, verified selection was unchanged, moved
both workspaces, then queried each retained variable through its original
surface ID. All original PIDs stayed live; stored values survived; cleanup reaped
the fixtures. Compilation and the full smoke passed, including oversized/stale
input errors. Output-pressure throughput, queue saturation through RPC and visual
viewport acceptance remain pending; queued success is not execution confirmation.

Added cached PTY process_id and reaper-completion process_exited to surface
discovery without acquiring terminal/exit locks. Extended the native harness with
opt-in live fixture shells and a mise smoke-live-sessions task. Five independently
started shell PIDs remained live and unchanged through populated-workspace and
last-workspace moves; all five disappeared after harness cleanup. The complete
native run passed, including layout/identity, selection, zoom and notification
checks. This verifies process continuity, not arbitrary shell variable/history
state, visual focus or latency. PID reuse after exit is documented.

Added workspace.move_to_window over live RPC with explicit source/destination
IDs and optional activation. UI and RPC now share a result-returning transfer
path; both windows reject transfer during editing, palette, restore or close
interactions. Same-owner requests are no-ops. The native smoke moved a populated
workspace, repeated the request without duplication, moved the last source
workspace, verified empty-source window removal, retained every terminal ID and
layout tree, and focused a moved terminal. Compilation and the full smoke passed;
build hash is recorded in docs/benchmarks/live-rpc-smoke.md. The fixture uses
exited shells, so running-shell PID/state and visual activation remain unproven.

Verified automatic non-login Bash through a real macOS PTY and Ghostty terminal.
The fixture loads the private startup wrapper, observes its initial OSC 7 path,
changes to a Unicode/space/percent-containing directory, checks the decoded path
and command output, fills scrollback, navigates between shell-marked prompts and
waits for clean shell exit. It shares the existing zsh PTY fixture; both tests
passed in 0.07s. This closes the basic Bash wrapper-to-terminal integration gap;
user themes, full command-status markers, login-mode Bash and other platforms
remain pending.

Added explicit Unix non-login startup through RMUX_SHELL_LOGIN=0, retaining login
shells as the default. Non-login Bash now receives a private --rcfile wrapper
that sources user .bashrc before embedded rmux hooks; the wrapper lives with the
session and never changes user files. Integration opt-out remains honored.
Explicit command launches and Windows are unaffected. The focused macOS Bash
wrapper test passed for user rc loading, hook chaining, integration idempotence
and temporary-file cleanup; existing Bash integration tests also passed.
Automatic Bash login startup remains pending because login shells bypass rcfile;
native non-login PTY/theme acceptance and other-platform startup remain pending.

Added opt-in manual fish integration using named prompt/preexec/postexec/posterror
events and built-in URL escaping for OSC 7. The hooks preserve user prompt
functions, guard re-sourcing, support the existing opt-out, report command status
and close unfinished command regions at the next prompt. macOS bundle packaging
now includes the script. Added a no-install fish fixture and mise task. Python
fixture syntax and bundle shell syntax passed; fish syntax/runtime acceptance is
pending because no fish executable is installed and mise has no short-name fish
registry entry. No tool was installed. Automatic loading, input-start markers,
theme/event ordering and interactive syntax-error acceptance remain pending.

Made the command palette height-aware. Its maximum height follows the window
viewport, the overlay uses a smaller top inset below 480px, and the results list
can shrink while preserving its scroll handle. Search input does not shrink;
below 300px the descriptive heading/footer are omitted to leave more room for
input and results. Normal-size windows retain the existing 48px inset and 320px
result-list cap. Desktop compilation passed. Native compact-window clipping,
resize and keyboard/IME acceptance remains pending while visual access is locked.

Added command-palette Page Up/Page Down navigation in eight-result jumps, clamped
to list bounds while arrow keys retain wraparound and scroll-to-selection.
Existing IME composition protection remains. Long command titles have hover
hints, selected-result hover remains distinct, and clicks resolve the displayed
command's identity against current search results instead of trusting a captured
row position. Desktop compilation and focused empty/single/long-list navigation
boundary tests passed. Native keyboard, hover and visual acceptance remain pending.

Aligned sidebar workspace interactions with terminal tabs: standard double-click
opens the existing name editor; click callbacks resolve current workspace IDs
rather than captured list indices. Truncated names have full-name hover hints,
selected rows retain a distinct hover color, row spacing is explicit, and the
36px sidebar header keeps its height and divider aligned with the workspace
header. New Workspace exposes its platform shortcut in a tooltip and, where
space permits, inline. Desktop compilation passed for the primary change; native
visual/interaction acceptance remains pending because CUA reports the Mac locked.

Added workspace.rename with identity targeting, shared sidebar-editor name
normalization and existing busy/deadline guards. It changes metadata without
selecting or activating the workspace; active names also update the window title.
Sharing normalization fixes blank-looking control/whitespace-only names in the
editor, keeping them from later failing saved-layout validation. Compilation and
the native two-window smoke passed, including Unicode normalization, blank-name
and stale-ID errors, and renaming an inactive workspace without selection changes.
The smoke also covers the preceding JSON move/ID lookup changes. Native pixel
appearance and performance measurements remain pending.

Reduced automation allocation overhead. Target IDs are converted once per UI
request and compared as generation-bearing EntityIds, avoiding per-candidate
string allocation across windows/panes/tabs. Noncanonical spellings and overflow
remain not-found errors rather than aliases. Discovery now moves JSON arrays,
workspace layouts and response payloads instead of deep-cloning them during
flattening/wrapping; recursive split serialization also moves child trees.
All ten automation tests passed on the final change, including ID generation and
spelling checks plus nested layout serialization. The two-window native smoke
passed after the ID conversion and initial array-move change; later JSON wrapper
cleanup received focused test coverage. Allocation/RSS/latency benefits have not
been measured; no numeric performance claim is made.

Added window.create with optional activation (default false), bounded/deadlined
dispatch, close-prompt/shutdown guards and explicit creation errors. It uses the
existing new-window path, now parameterized for focus/app activation; ordinary
UI launches retain their previous behavior. Responses identify the new window
and its initial workspace. Expanded the native smoke to create a second window,
verify discovery ownership, focus terminals in both windows and retain all
original IDs. Compilation and the native smoke passed; recorded the build hash.
OS focus behavior, creation latency, shell-start failure behavior and visual
acceptance were not measured by this fixture.

Added explicit pane.zoom.set with strict pane_id/zoomed parameters, identity
resolution, existing busy/deadline guards and live capability advertisement.
Zoom-in selects the pane's current terminal without OS activation; zoom-out only
clears that pane's zoom and repeated requests do not toggle. Expanded the native
macOS smoke to check malformed/stale targets, repeated zoom-in, unrelated-pane
zoom-out, surface focus transferring zoom, and unzoom restoring the same layout
weights and terminal IDs. Compilation and the expanded smoke passed. Recorded
the executable hash in docs/benchmarks/live-rpc-smoke.md. Visual appearance,
cross-window behavior and latency remain outside this check.

Live stdio mode now combines with saved-layout restoration or directory/startup
command options while rejecting headless inspection combinations. Extended the
native harness to restore two workspaces, three panes and four tabs, then select
an inactive tab, sibling pane and different workspace. All target selection and
parent-link assertions passed without changing terminal IDs; notification,
stale-ID and EOF checks still passed. All six focused startup tests passed.
Evidence and executable hash are updated in docs/benchmarks/live-rpc-smoke.md.
Cross-window behavior, zoom, visual focus and latency remain unverified by this
check; no additional tools were installed.

Ran the live automation path against a freshly built native macOS debug app.
The isolated single-window/single-pane fixture passed discovery identity links,
workspace selection, surface focus, stale-ID errors, notification response
suppression and stdin-EOF survival. The fixture uses /usr/bin/true as its shell,
temporary settings and only terminates its own process. Added the reusable
scripts/smoke-live-rpc.py harness and mise smoke-live-rpc task. Evidence and exact
executable hash are in docs/benchmarks/live-rpc-smoke.md. This closes the basic
GUI/pipe integration gap; visual focus, cross-window changes, zoom, concurrency,
latency and other-platform acceptance remain pending.

Added surface.focus with strict surface_id/activate parameters and live-only
capability advertisement. Resolution searches current window/workspace/pane/tab
ownership by identity; stale IDs return a structured not-found error. The existing
terminal navigation path selects workspace and tab, remembers focus and moves
zoom to the target pane; OS activation remains opt-in. Shared automation busy
checks now also protect tab rename/close interactions for both surface focus and
workspace selection. Desktop compilation and all nine automation tests passed,
including malformed parameters, standalone rejection, routing, response IDs and
notification execution. Native focus, zoom, moved-tab and live-pipe acceptance
remain pending.

Extended live discovery with pane.list and surface.list, linking terminal tabs
to stable process-local pane/workspace/window IDs. Results expose tab titles,
selection/attention, cached directory labels, counts and zoom without accessing
PTY or Ghostty state. workspace.list now includes the nested split tree, axes,
relative weights and zoomed pane ID; clients need not infer geometry from internal
pane order. Desktop compilation and all eight focused automation tests passed,
including nested split serialization. Native live discovery and mutation/focus
acceptance remain pending.

Added live workspace.select with strict workspace_id/activate parameters and
process-local identity targeting across windows. Selection uses the sidebar path
and preserves its workspace/pane focus history; OS activation is explicit and
defaults off. Busy editing, palette, restoration, close-dialog and shutdown states
reject the operation. Unknown targets return a structured error without index
fallback. An atomic queued/started/canceled handshake prevents expired queued
mutations from executing later; lost replies after execution starts explicitly
report unknown outcomes. Selection notifications execute without replying.
Desktop compilation and all seven focused automation tests passed, covering
parameter validation, notification execution, typed errors, queue bounds and
deadline/cancellation state transitions. Native selection/focus and live-pipe
acceptance remain pending; no compatibility or native performance claim is made.

Added --rpc-stdio-live to launch the GUI with read-only discovery through inherited
pipes. window.list and workspace.list return process-local entity IDs, selection,
metadata and counts from UI state without terminal locks, snapshot extraction or
focus changes. Blocking IO runs on one named worker; a bounded eight-request
channel wakes the UI on demand, with a five-second reply deadline and nonblocking
reply delivery. EOF detaches automation without closing windows. Standalone mode
still advertises only its own methods. Compilation and all 54 desktop tests passed,
including live bridge routing, disconnection, saturation, capability separation,
notification suppression and CLI conflict coverage. Native GUI-to-pipe acceptance,
socket transport, targeting/mutations and events remain pending.

Added --rpc-stdio as a headless automation discovery foundation. It serves
system.ping and system.capabilities with JSON-RPC response IDs, structured
errors, notification suppression and a 64 KiB per-request limit; oversized
requests terminate the stream. It bypasses settings loading and UI/PTY creation,
and rejects launch/inspection combinations. Capabilities explicitly identify
standalone mode without live workspace access. All 52 desktop tests passed,
including protocol and CLI conflict coverage; an actual cargo-run stream returned
both discovery responses and exited successfully. Live-window transport, caller
scope, targeting and events remain pending; this is not cmux wire compatibility.

Stopped unpressed mouse-motion reports outside the inset text area, while
retaining local selection drags and releases for presses started in the grid.
Cell-based application mouse reports now use the actual terminal rows/columns
instead of ceiling the pixel area, and clamp trailing partial-cell coordinates
to the last valid cell. SGR pixel reports retain pixel coordinates. Focused tests
passed for fractional cell widths, bottom/right partial cells, resize, negotiated
motion and pixel reporting; desktop compilation passed. Native padded-pane and
drag interactions remain unverified.

Added terminal_padding (0–32 logical pixels, default 8). The terminal element
derives one inset content rectangle for PTY sizing, text/cursor placement,
selection coordinates and IME input bounds, while painting its background across
the outer rectangle. Content clipping uses the inset; find/status bars retain
their own layout. Padding clicks focus without generating an application mouse
press. Reload applies through existing observers; tiny panes clamp the content
size to zero instead of producing negative geometry. The focused geometry test
and desktop compilation passed. Native DPI, drag-selection, IME and visual
acceptance remain pending; earlier app benchmarks used zero inset.

F12 renderer diagnostics now include snapshot_storage: cell-vector capacity,
logical text bytes, inline/heap cell counts and heap text payload bytes. Frame
accounting walks cached data only when explicitly exported, with no engine lock
or fresh snapshot extraction. Added focused accounting assertions to the short/
long-grapheme fixture; that test and desktop compilation passed. Documentation
distinguishes cached/possibly stale hidden-tab data from Ghostty storage, allocator
overhead, shared allocations and process RSS. Native collection remains pending.

Added cursor_glyphs_reused/cursor_glyphs_rebuilt to opt-in F12 renderer diagnostics.
The counters record actual block-cursor glyph cache hits and rebuilds, without
timers or automatic exports. Existing row counters remain separate. Documented
their scope so native runs can measure reuse during repaints and invalidation
after text/font/color/DPI changes. Native collection remains pending.

Added a one-entry block-cursor glyph cache per terminal. Repainting the same
covered cell now reuses its shaped line instead of rebuilding text/font runs.
The key includes the complete cell, cell metrics, font family and display scale;
style, color, text and font/DPI changes rebuild it. Hidden-terminal cache release
and window rebinding clear the entry alongside shaped rows. The cache owns at
most one cell and shaped line and adds no timers. Desktop compilation passed;
native font/color transitions and cursor-cache latency benefits remain unmeasured.

Added macos_option_as_alt, default true, to let users opt into native Option text
composition. When false, Option-modified character/space presses without Control
bypass terminal key encoding and remain on AppKit's text/dead-key path. They do
not enter the terminal pressed-key map, avoiding orphan release reports. Control+
Option and navigation keys retain encoding; Linux/Windows behavior is unchanged.
Configuration default/type coverage and desktop test compilation passed. Native
international-layout, dead-key and TUI/IME acceptance remain pending.

Added terminal_theme presets dark and light on top of the existing color/palette
configuration. Dark retains current defaults; light supplies a white background,
dark foreground, blue cursor and a darker ANSI palette. Explicit colors and
palette override the preset, while null inherits it. Existing resolved-value
comparison and preference observers apply changes live without new timers.
Focused preset/override/validation coverage and desktop test compilation passed.
The preset affects terminal content only; app chrome, OS appearance following
and native visual acceptance remain unfinished.

Changed renderer text-run construction to determine each compatible style run
and its byte length before allocating its String. Long ASCII runs now reserve
their required capacity once instead of repeatedly growing the buffer; short
style runs do not reserve the remainder of the row. This adds a cell traversal
and is an allocation reduction, not a measured native speedup. Updated the empty-
cell test fixture for immutable small-string storage. Focused row-cache tests
cover concealment, decorated blanks, style boundaries, wide cells and trimming;
native shaping/latency measurements remain pending.

Changed snapshot Cell.text from String to the existing smol_str dependency:
short graphemes remain inline, eliminating individual cell-string heap allocation
on the common path, while long graphemes remain supported. Renderer shaping
still builds owned text runs. All 61 terminal tests passed, including new
ASCII/CJK/combining and long-grapheme storage coverage; desktop compilation
passed. An eight-engine release run recorded 251.6 MiB/s and 0.148 ms snapshot
p99 (earlier separate run: 227.2 MiB/s and 0.186 ms). Raw evidence and limitations
are recorded in docs/benchmarks; no native latency or RSS improvement is claimed.

The current ARM64 Linux desktop cross-check passed via mise run
check-linux-desktop after the accumulated dirty-state, large-paste, cursor,
window-transfer, lifecycle and color/palette changes. This checks Rust/FFI
compilation against the Linux dependency build; it does not link or run the
desktop or establish native appearance, input or lifecycle acceptance. Windows
validation remains deferred as requested. Updated T03's feature ledger to include
the implemented color and ANSI palette settings while keeping native gates open.

Added ansi_palette as an optional, exactly 16-entry #RRGGBB configuration array.
New terminals apply it at creation; Reload Settings updates existing terminals
only when the palette changes. The engine updates default ANSI entries while
preserving default indices 16–255, leaving application OSC overrides separate.
Null resets the built-in defaults. Focused tests passed for normal/bright ANSI
cells, unchanged extended/true-color values, application override/reset behavior,
built-in reset and array-length validation. Desktop compilation passed. Native
theme appearance and full T03 acceptance remain pending.

Preference reload now compares each terminal's last successfully applied color
defaults before acquiring its engine lock. Unchanged colors avoid setter calls,
revision increments and full color-driven snapshot invalidation when changing
unrelated settings. Failed applications remain retryable on the next reload;
window transfers retain the applied state. Native access was checked again and
the Mac is still locked, so visual acceptance remains pending.

Added foreground_color/background_color/cursor_color configuration as validated
#RRGGBB strings or null defaults. TerminalView applies colors at creation and
through the existing event-only preference observer on reload. Ghostty default-
color setters preserve application OSC overrides; clearing configuration restores
rmux defaults. Snapshot revisions and Ghostty dirty state invalidate affected
render rows through the existing cache path. Focused tests passed for color
updates, old-snapshot isolation, OSC override/reset precedence and default reset;
configuration validation/serialization coverage and desktop compilation passed.
Native appearance/reload acceptance and full palette/theme controls remain open.

Centralized shutdown registration in TerminalView::new so normal tabs, splits
and restored terminals all use one path. Each registered handle now waits on
the existing reaper completion channel and removes completed registry handles
as soon as that channel closes, releasing empty vector capacity too. Previously,
normal creation pruned old handles but restore only appended them, and a closed
tab's bookkeeping could survive until another creation. The waiter retains only
the shutdown handle, not the session, PTY or terminal, and uses no polling timer.
Desktop compilation passed; native repeated restore/close memory and descriptor
measurements remain pending, so no RSS improvement is claimed.

Added a system-quit cleanup fallback through GPUI's on_app_quit hook. Shutdown
handle collection and process-stop requests now share begin_shutdown with the
normal Quit action; all requests are issued before awaiting any completion, and
the quitting flag prevents duplicate work. The normal action retains confirmation
and its three-second wait. GPUI's system hook cannot veto quitting and gives
returned futures only 100 ms, so this remains best-effort cleanup, not system-
quit confirmation or guaranteed teardown. Desktop compilation and the focused
stubborn-shell shutdown test passed; native system-quit acceptance remains open.

The command palette now offers named destination windows for workspace moves,
labelled by window-list number and the destination's active workspace. Both
next-window and explicit-target actions use the same transfer function. Palette
entries capture the original workspace entity ID and window handle rather than
re-resolving by mutable list index or label; a closed target fails before source
removal, and a missing source cannot move another workspace accidentally.
Desktop compilation passed. Native multi-window transfer, stale-target and focus
acceptance remain pending; full W06 parity is not claimed.

Added Move Workspace to Next Window to the menu and palette. It chooses the next
existing normal rmux window, transfers the same workspace/layout/terminal
entities with rebound window subscriptions, preserves metadata/pin state and
last-pane focus, and inserts pinned workspaces at the pinned-group boundary.
Destination ownership is established before source removal; an emptied source
window closes and the destination activates. Missing destinations and pending
restore/window-close operations report errors before mutation. Desktop
compilation passed. Native process-identity, focus, confirmation, pinned-order
and source-window teardown acceptance remain pending; W06 stays in progress.

Find Older/Newer now queues the requested navigation when reopening a closed
find bar, so one shortcut both starts the search and selects a result after its
scan completes. Repeat-find prefers the remembered query over unrelated current
selection; explicit Find retains selection-prefill behavior. Empty queries still
open the input without navigating. The existing worker retains movement during
incremental scans, avoiding UI-thread search work. Desktop compilation passed;
native shortcut/reopen acceptance remains pending.

Fixed tab-close focus handling: removing an inactive tab no longer unconditionally
focuses that pane's selected terminal. A replacement receives focus only when
the removed terminal contained it. Removing the final tab in a split now leaves
another focused pane alone rather than selecting a layout neighbor. Desktop
compilation passed; native pointer/prompt focus transitions remain unverified.
Rebuilt target/debug/rmux.app through mise, including the recent dirty-state,
large-paste, inactive-cursor and tab-interaction changes. Bundle creation and its
headless --version check passed; this is a development build, not a release.

Added the roadmap's eight-pane case to engine/PTY benchmark arguments and default
matrices. Corrected engine idle sampling to consume the last dirty frame before
timing unchanged reads. Sequential release runs recorded 227.2 MiB/s engine
throughput, 0.186 ms flood-snapshot p99, 63 ns unchanged reads, and 31.25/3.59 MiB
Ghostty resident page storage before/after compression. The real eight-session
PTY fixture completed 128 responses under seven flooding peers at 1.366 ms
response-snapshot p99; no benchmark children remained afterward. Raw JSONL and
scope are in docs/benchmarks. This is engine/PTY evidence, not native GUI latency
or proof of full P0 performance acceptance.

Tab-strip pointer polish: double-click selects and opens the existing rename
editor; middle-click uses the existing close-confirmation flow; title tooltips
expose labels truncated at 220 px. Close controls now have a centered 22×22 px
target and tooltip, and labels have a 48 px minimum for stable short-title tabs.
Pointer callbacks resolve terminal identity rather than relying on an old tab
index. Existing horizontal scrolling and active-tab reveal remain in place.
Desktop compilation passed; native click/drag/overflow visual acceptance remains
pending while the host UI is unavailable.

Inactive terminals now show a hollow cursor by default instead of losing their
insertion-position indicator. The new unfocused_cursor setting accepts hollow
or hidden and live-reloads across existing terminal views through an event-only
global observer. Cursor focus now includes window activation, so inactive
windows do not retain a focused cursor. Focused application cursor shapes,
application-requested cursor hiding and the IME bar caret remain intact. The
focused preference/shape test passed with desktop test compilation; native
visual focus-transition acceptance remains pending. No blink timer was added.

Large clipboard paste now accepts up to 1 MiB of UTF-8 text (previously just
under 64 KiB). Encoded data moves into one writer-queue message, retaining the
bracketed-paste envelope without interleaved keys or terminal replies. A per-
session permit admits only one queued/in-flight paste and is released on all
encoding, enqueue and write failure paths; the existing 32-message queue and
64 KiB ordinary-input cap remain. Focused tests passed for the exact size limit,
permit lifetime/queue rejection, existing sanitization and a real macOS PTY
receiving 140 KB of Unicode/multiline text followed by ordinary input in order.
Cross-platform pressure/cancellation acceptance remains pending for T07.
Native visual inspection was attempted again but the Mac remains locked.

Fixed stale per-row Ghostty dirty flags after snapshot extraction. The complete
owned snapshot now consumes render state with ghostty_render_state_clean rather
than clearing only the global flag. A regression test verifies that a subsequent
single-row edit exposes only that row, while the old snapshot stays immutable.
All 55 terminal library tests passed. Re-running the same 160×48 release fixture
across 1/4/16 terminals measured 2.11–3.85 µs extraction means versus the prior
122.6–130.3 µs, retaining identical row reuse. Raw results are in
benchmarks/cleaned-dirty-snapshot-reuse.jsonl. This removes unnecessary full-screen
decoding; whole-app latency and native UI parity remain unverified.

Added and ran the release dirty_snapshot_bench fixture across 1/4/16 terminals
at 160×48. Identical rewrites preserved 48/48 row identities; changing one row
preserved 47/48. Engine extraction means were 122.6–130.3 µs, showing that cell
decoding/comparison remains substantial even while renderer reshaping can be
avoided. Raw results and scope are recorded in benchmarks/dirty-snapshot-reuse.jsonl
and the benchmark README. No native UI speedup or before/after claim is made.

Dirty-row extraction now compares complete cell content/style against the prior
snapshot before replacing its row handle. Identical rewrites and selection-only
changes retain row identity, allowing the renderer to reuse shaped glyphs even
when Ghostty conservatively marks those rows dirty. Actual text/style changes
still replace rows through copy-on-write. Focused tests passed for identical
rewrites, selection isolation, style invalidation and unrelated-row reuse;
desktop compilation passed. The comparison adds CPU work to dirty extraction;
net native shaping/latency benefits remain to be measured.

Added Open Settings File to the menu and palette. Its explicit invocation
prepares the configured path on a background worker, atomically creates defaults
only if missing, and opens it through the system association. Existing content,
including temporarily invalid edits, is preserved. It shares the settings
in-flight gate to avoid overlapping opens/reloads. Focused create/preserve
coverage and desktop compilation passed; native editor handoff remains pending.

Saved-layout validation now rejects control characters in workspace names and
fallback tab labels, matching existing custom-title/description restrictions.
The shared decoder applies this before restore preflight or shell creation and
through --check-layout; save validation also rejects such state. Focused coverage
passed for newline, carriage return, escape, NUL and DEL while retaining the
valid Unicode fixture. Split geometry guards were already present and unchanged.

Added --check-layout PATH for headless structural validation using the same
bounded 2 MiB loader as restore. It validates schema/version, geometry and tab/
workspace limits without loading user preferences or spawning sessions. Launch
options and conflicting inspection modes are rejected. Focused parser coverage
and desktop test compilation passed. This command intentionally does not claim
directory availability or shell executability; restore retains its preflight.

Added --dump-config to print effective, reusable JSON preferences and exit before
GUI/PTY startup. Serialization excludes internal source/override bookkeeping;
the values include defaults and the applied close-policy environment override.
It rejects launch options, duplicates and combining with --check-config. A real
headless invocation printed the expected defaults and exited zero; focused parser
coverage passed. Separate launch environment variables are not part of this dump.

Pressed-key tracking now pairs recognized shifted/unshifted aliases through a
symbolic key identity instead of the raw UI string. A press reported as ? can
therefore pair with a release reported as / after Shift is lifted; the original
sent key is retained for release encoding and focus-loss cleanup. Unicode
identities remain distinct, repeats keep one entry, and unsent shortcut/text
events still cannot emit orphan releases. Five focused keyboard tests and desktop
compilation passed. This does not replace physical scan-code/layout support;
native modifier-order acceptance remains pending.

Single-character non-ASCII key identities now use Ghostty's unidentified-key
path with a Unicode codepoint, allowing report-all mode to encode them rather
than silently falling through. Focused coverage confirms é produces CSI 233 u
only under negotiated report-all mode. Desktop key handling also leaves ordinary
character identities without native text on the composition path, avoiding a
premature key report for those dead-key events. Desktop compilation passed;
native dead-key, international layout and IME acceptance are still pending.

Kitty report-all-keys mode now routes mapped ordinary character presses through
Ghostty instead of bypassing the encoder. The desktop supplies native key text
for unmodified/Shift characters, preserving associated-text reports separately
from the unshifted key identifier. Existing pressed-key tracking pairs releases;
normal text and active composition retain their native input paths outside this
mode. Four keyboard tests passed, plus focused shifted associated-text coverage
and desktop compilation. Non-ASCII key identities, physical layout mapping and
native application/IME acceptance remain incomplete.

Shifted ASCII punctuation now maps to Ghostty key identifiers instead of falling
through to native text handling when combined with Control/Alt. Focused encoder
coverage confirms Ctrl+Shift+underscore/caret/question-mark produce US/RS/DEL,
and Ctrl+Shift+at-sign follows Ghostty's CSI-u disambiguation. Unmodified symbol
text still uses native input/IME. Native keyboard-layout acceptance remains open;
these symbolic mappings do not provide physical-key identity for arbitrary layouts.

Added Reveal Working Directory to Edit and the palette through GPUI's platform
reveal API. It shares local-directory resolution and unavailable-path feedback
with Copy Working Directory, preserving OS paths without constructing a shell
command or file URL. Remote directory reports are not mapped to local paths.
Desktop compilation passed; native file-manager handoff and platform acceptance
remain pending.

Added Copy Working Directory to Edit and the command palette. It uses the
existing local-only OSC directory resolver on demand, with no filesystem polling
or process inspection. Unavailable/remote metadata leaves the clipboard untouched
and reports why; valid paths copy as literal text rather than shell commands.
Host workspace all-target compilation passed, including examples/tests after the
shared Frame API changes. Native clipboard acceptance remains pending.

Added copy_on_select (default false) to bounded JSON preferences. When enabled,
releasing a local mouse selection copies through the existing Unicode-aware,
size-limited clipboard path; application mouse reports do not trigger copying.
The preference is read at release, so Reload Settings applies to existing
terminals. Configuration validation and desktop test compilation passed.
Native clipboard, Shift-override and rectangular-selection acceptance remain open.

The optimized macOS bundle has been rebuilt with the recent snapshot, search,
navigation and cache-lifecycle changes. Benchmark packaging now ad-hoc signs
after changing the bundle identifier and launch environment, then verifies the
signature. Generated the eight-pane benchmark bundle successfully and its
--version command exits zero. These are packaging/headless checks, not a native
eight-pane performance result. Developer ID signing/notarization remain separate.

Rebuilt the macOS bundle and found its copied executable was killed at startup
(exit 137), while target/debug/rmux ran normally. Code-signature verification
failed for the bundle. The packaging script now applies a local ad-hoc signature
after copying the executable, plist and resources. The rebuilt bundle verifies
successfully, and its --version and --check-config commands both exit zero.
This is local development signing, not Developer ID signing or notarization.
The fresh CUA check still reported a locked Mac; native UI acceptance is pending.

Frame extraction now allocates its UTF-8 scratch buffer only after finding a
dirty row. Unchanged frame reads no longer allocate that 64-byte temporary.
Focused styled/Unicode snapshot coverage passed. The release snapshot benchmark
follow-up measured 64–83 ns unchanged-frame means (four-terminal fixtures:
67/66/66 ns for 24/48/120 rows); raw results and single-run caveats are recorded
in benchmarks/lazy-scratch-snapshot-costs.jsonl and the benchmark README.
Native latency and allocation instrumentation remain pending.

Added and ran a release snapshot microbenchmark at 1/4/16 sequential terminals
and 24/48/120 rows. Unchanged engine frame means were 72–99 ns; Frame clones were
3–5 ns. A synthetic copy of the old row/selection vectors measured 49–250 ns and
grew with row count. Raw results are in benchmarks/shared-snapshot-costs.jsonl;
the benchmark README documents the fixed-order, one-run scope. This does not
measure native UI latency, dirty frames or whole-app memory, and is not a
before/after comparison of full builds.

Selection spans now share an Arc-backed vector from engine Frame through the
paint snapshot. Dirty text rows only copy that vector when their selection span
actually changes; unchanged snapshots clone handles without copying either row
or selection vectors. Copy-on-write preserves existing highlight snapshots
through selection changes and clears. Focused selection lifetime and unchanged
snapshot reuse tests passed, along with desktop compilation. Native performance
measurements remain pending.

Engine Frame snapshots now share their outer row list as well as individual
cell rows. Cloning an unchanged frame avoids allocating a row vector and
incrementing each row's reference count. Dirty-row replacement and resize use
copy-on-write, preserving older snapshots and sharing rows Ghostty leaves clean. Focused
coverage passed for outer-list reuse, changed-row isolation and resize lifetime;
desktop compilation passed. Selection vectors still clone, and native allocation
and latency measurements remain pending.

The shaped-row cache now shares its complete row vector with the paint snapshot
through an Arc. Unchanged repaints check source-row identities and clone only
that outer handle, avoiding a vector allocation and one reference-count update
per row. Changed content uses copy-on-write so any outstanding paint snapshot
remains coherent; font/scale/background changes still invalidate the cache.
Four focused row-cache tests passed with desktop test compilation. Native paint
latency and allocation measurements remain pending; no speedup figure is claimed.

Added RMUX_SHELL as an explicit executable override for normal and restored
shell terminals, before inherited SHELL/COMSPEC and platform fallback. Shell
paths now remain OS strings rather than requiring UTF-8; spaces are literal,
an empty override errors, and empty inherited values use the platform default.
Existing Unix login arguments and zsh integration remain in the shared launch
path. Direct startup commands and benchmark fixtures are unchanged. Focused
resolution coverage and desktop compilation passed; native custom-shell launch
acceptance remains pending.

Metrics export now attempts the engine lock without waiting when collecting
memory statistics. A busy parser produces a null memory sample while the render
counters are still exported; null is documented as unavailable, never zero.
The normal blocking memory API remains available for engine measurements.
Desktop compilation passed. Snapshot formatting still runs on demand on the UI
thread; file writes remain asynchronous and single-flight.

Render-metrics JSONL file opening/appending now runs on the background executor.
An application-wide in-flight flag ignores repeated export requests until the
write completes, avoiding concurrent appends and an unbounded export queue.
Completion clears the flag even if the originating workspace has closed; write
errors appear on that workspace when it still exists. Snapshot collection still
runs on demand on the UI thread and covers all tabs in the active workspace.
Desktop compilation passed; native export responsiveness remains unverified.

Opt-in render diagnostics now report cache_releases and rows_released alongside
rows_rebuilt/rows_reused and paint timings. Inactive-tab, hidden-workspace and
zoom release paths share one counter update; empty releases do not count. These
are cumulative dropped cache handles, not bytes freed or GPU atlas eviction.
No timers or automatic writes were added. Desktop compilation passed; native
before/after switching measurements remain pending.

Shaped-row cache release now covers workspace visibility changes and panes hidden
by zoom. Each window keeps a weak reference to its last rendered workspace and
releases that workspace's caches when the rendered target changes, covering
navigation paths without scanning every hidden workspace each frame. Zoomed
workspace rendering clears caches of the other panes. These operations neither
notify nor stop terminal processes; visible rows reshape on return. Desktop
compilation passed. Native switch/zoom latency and RSS measurements remain open.

Pane rendering now releases shaped-row caches belonging to its inactive tabs,
without notifying or adding timers. Engines, PTYs and scrollback remain owned;
returning to a tab rebuilds its visible glyph rows. Empty caches skip mutation.
This drops cached row handles and vector capacity, though GPUI may retain shared
font/atlas resources and the current paint frame can temporarily own rows. Four
focused cache tests passed, including release ownership/capacity. Whole-app RSS
and tab-switch latency are not yet measured; hidden-workspace caches remain a
separate lifecycle gap.

Concealed terminal cells now skip glyph shaping and decoration painting entirely,
while their backgrounds and column positions remain intact. Previously they
were painted in the cell background color, which could make their outlines
visible over a selection highlight. Visible text runs now stop at concealed
cells, and overlines are suppressed there too. Three focused row-cache tests
passed, covering concealed run boundaries alongside existing Unicode/style and
decorated-blank behavior. Native selection-highlight appearance remains pending.

Added direct tab selection within the focused pane: Cmd+Shift+1–9 on macOS,
Ctrl+Shift+1–9 elsewhere, plus Select Tab 1–9 palette actions. Next/Previous Tab
are now exposed in the palette too. Selection uses the existing pane path to
reveal the tab, focus its terminal and preserve its process; absent numbers do
nothing. Existing workspace/pane shortcuts remain distinct. Desktop compilation
passed; native shortcut routing and platform acceptance remain pending.

Added Last Prompt to the menu and command palette. It navigates from the live
viewport backward to a nearby prompt and then forward to the newest retained
marker, so a prompt inside the active screen is not accidentally skipped.
Unmarked output falls back to the live viewport. Focused engine coverage passed
for long trailing output, an arbitrary older viewport, a latest visible prompt
and no markers; desktop compilation passed. Native acceptance remains pending.

Invoking Find while its bar is already open now focuses and selects the query
for replacement, with explicit FindInput shortcut bindings on macOS and other
platforms. The shared text-field Select All path preserves active IME marked
text and resets pointer selection state otherwise. This does not recreate the
worker or resubmit an unchanged query. Desktop compilation passed; native
keyboard and IME acceptance remain pending.

Find now remembers the last query per terminal when closed, including closes
caused by Clear Scrollback or transfer to another window. A valid current
selection takes precedence when reopening; otherwise the saved query returns
selected for replacement and the on-demand search worker restarts. Retention is
one bounded query string, not a running worker or search index, and is not added
to layout snapshots. Desktop compilation passed; native acceptance remains open.

New Find bars now prefill from a single-line terminal selection up to 4 KiB and
start the existing asynchronous search worker immediately. The text starts
selected for replacement; reopening an existing bar preserves its query. A
bounded selection extractor checks formatted byte size before allocating its
output, so a large selection is not copied just to seed Find. Multiline/control
text and oversized selections leave the field empty. Focused Unicode extraction
limit coverage and desktop compilation passed; native interaction remains open.

Refreshed target/debug/rmux.app through `mise run bundle`, including the current
preferences, font handling and hyperlink implementation. The build and bundle
copy completed successfully. A fresh native-UI availability check reported the
Mac locked and automatic unlock unavailable; no launch or click acceptance is
claimed. The bundle is ready for the next unlocked native verification pass.

Link opening now checks the displayed frame revision against the engine under the
session lock. Output, viewport movement, resize, retention changes and selection
navigation invalidate older snapshots; synchronized-output holds retain the old
frame revision. This prevents mixing cached text with newer wrap metadata or
opening a newly substituted OSC target before its frame is displayed. The check
adds scalar bookkeeping without per-frame URI scanning. Four focused hyperlink
tests and desktop compilation passed, including output holds, scrolling and
resize invalidation. Native click acceptance remains pending.

Plain-text HTTP(S) URLs now use the same modifier-click path as OSC 8 links.
Detection runs only on click, follows soft-wrap flags, maps wide Unicode cells to
text offsets, excludes prose punctuation and preserves balanced URL parentheses.
Explicit OSC 8 targets win. Logical lines are bounded at 8 KiB; lines clipped at
either viewport edge are rejected to avoid launching truncated addresses. Three
focused hyperlink tests and desktop compilation passed, including wrapping,
wide-cell prefixes, explicit-target precedence and clipping. Native interaction,
hover feedback and file-path routing remain open.

OSC 8 HTTP(S) hyperlinks now open in the default browser with Cmd-click on macOS
or Ctrl-click elsewhere. Lookup reads the clicked viewport cell under the engine
lock and allocates URI text only on demand, capped at 8 KiB. Ordinary clicks keep
their selection/application-mouse behavior; link presses do not send a matching
release to the terminal application. Focused engine tests passed for scrollback,
alternate screens, wide Unicode link text, out-of-grid coordinates and oversized
URI rejection (the current Ghostty parser drops the oversized OSC itself).
Desktop compilation passed. T10 remains in progress: native click acceptance,
hover feedback, plain-text URLs and local/remote path routing are still pending.

The engine benchmark now accepts a scrollback budget and reports retained history
lines. A four-engine release comparison at 0/1/4/16 MiB per terminal is recorded in
benchmarks/engine-scrollback-budgets.jsonl. At 1 versus 4 MiB, total engine resident
storage was 3.13 versus 15.63 MiB with similar throughput (~268/~267 MiB/s). Zero
retention used 1.56 MiB but ran slower (~174 MiB/s); 16 MiB used 62.50 MiB before
compression. These are one-run engine-only measurements, not whole-app memory or
native interaction acceptance. The benchmark README records scope and caveats.

Configured scrollback retention is now set before PTY child creation, covering
normal shells, direct startup commands and manual/startup layout restoration.
This prevents early output being retained or evicted under the old default while
waiting for a view. The benchmark constructor retains its fixed 4 MiB budget.
A focused real-PTY test passed for immediate startup output with zero and 1 MiB
limits; desktop compilation passed. Native UI and broader memory measurements
remain pending.

Added scrollback_mib preferences (0–256, default 4) applied when a terminal view
is created. Zero disables retained history; reload affects newly created views
without trimming existing sessions. The shared engine setter uses Ghostty's
retention API and clears stale selection state. Focused engine coverage passed for
existing-history removal, active-screen preservation, disabled future history and
resumed retention; preference validation and desktop test compilation passed.
The limit is an engine history budget rather than a total RSS/GPU cap. Native
configuration acceptance and memory measurements at alternate limits remain open.

Use Configured Font now applies the configured family and size to the focused
terminal through View/palette actions, preserving its session and history. Font
changes invalidate measurement and cancel selection-drag scrolling; row-cache
font keys trigger reshaping on the next paint. Reset Font Size still changes only
size. Requests for the already-selected family/size now avoid measurement reset
and redraw. Desktop compilation passed; native live-font switching, reflow and
IME acceptance remain pending.

New tabs and splits now inherit the source terminal's font family as well as its
size. New workspaces without a source terminal use the current configured default;
the benchmark fixture explicitly retains its platform font. Each new terminal
measures its inherited font independently on first paint. Desktop compilation
passed. Native inheritance after reload, mixed-font layouts and resize acceptance
remain pending.

Manual layouts now preserve each terminal's font family as well as its size.
The optional field keeps older layouts compatible: absent families use the
current configured default. Restored families are validated before shells launch
and measured when their views render. A focused snapshot test passed for Unicode
names, round trips, omitted fields and invalid/control/oversized names; desktop
test compilation passed. Native mixed-font restore and fallback acceptance remain
pending.

Added validated font_family preferences for newly created terminals. Row shaping,
cursor text and IME preedit/hit testing use the same per-terminal font; row-cache
keys include the family. Grid width is measured from the resolved font's zero
advance, with line height at least its ascent/descent extent. Measurement occurs
once per font size rather than per frame, and zoom invalidates it. Existing
terminals retain their family; restored layouts use the current configured one.
The focused preference validation test and desktop test compilation passed.
Native glyph alignment, IME/selection geometry, fallback fonts and rendering
performance remain unverified while desktop access is locked.

Reload Settings is now available through the menu and palette. File reading and
validation run on the background executor; successful results replace global
defaults/policy together, and failures retain current settings with a visible
diagnostic. Requests are coalesced across windows. The source path is made
absolute before startup directory changes, and the startup environment override
is retained. No filesystem watcher is added. A focused test passed for changed
font defaults, preserved override, invalid reload rollback and missing required
files; desktop test compilation passed. Native reload feedback and font reset
acceptance remain pending. Existing font sizes are preserved until explicit reset.

Added `rmux --check-config` for headless validation of the same file/environment
settings used at startup. It returns 0 with a success message or 2 with the
configuration diagnostic, and rejects combinations with layout, directory or
command launch options. Real binary invocations passed for valid, invalid and
missing explicit files, help despite invalid settings, and command conflicts.
No GUI or terminal process is started by this mode.

Added startup JSON preferences for font_size and confirm_close. Unix defaults to
XDG_CONFIG_HOME/rmux/config.json or ~/.config/rmux/config.json; Windows uses
APPDATA/rmux/config.json. RMUX_CONFIG selects an explicit required file, and
RMUX_CONFIRM_CLOSE overrides file policy. Missing default files use defaults;
invalid values, unknown keys and files over 64 KiB fail before shell preparation.
No files are automatically created and there is no watcher. New terminals and
Reset Font Size use the configured default, while splits inherit their source
size and restored layouts retain saved sizes. The focused defaults/validation/
missing-file/size-limit test and desktop test compilation passed. Native font
acceptance, live reload and settings UI remain pending.

Close policy now evaluates activity lazily. Always/never settings and empty scopes
skip terminal-engine locks and foreground-process-group queries entirely. The
focused policy test passed with assertions that those branches never invoke the
activity query. The macOS bundle was refreshed before this policy optimization;
a fresh native-access check again reported the Mac locked, leaving GUI acceptance
pending.

Termination prompts now share one application-wide permit, preventing tab,
workspace, window and Quit requests from stacking confirmation dialogs. The
permit releases on answer or task drop, independently of whether its original
window/entity can still be updated. A focused test passed for overlap rejection
and release/reacquisition after cancellation; the desktop test build passed.
Native modal-dialog behavior remains pending.

Active close warnings now include live direct commands and Unix foreground jobs
whose PTY process group differs from the owning shell PID, in addition to program
status records. This uses portable-pty's on-demand foreground-group query, without
process-table scans or timers. A real macOS PTY test passed for idle shell → sleep
job → Ctrl+C → idle and direct-command detection/cleanup. Desktop compilation
passed. Linux runtime and native dialogs remain pending; Windows job detection,
background jobs and shell-replacing exec commands are not covered by this signal.

Close-warning checks now use a boolean session/status query without cloning,
sorting or resolving inherited app strings for every record. The bounded status
scan returns as soon as a working/blocked record is found, and exited sessions
return false. Existing state-transition coverage now checks the predicate before
and after task completion; that focused test and desktop compilation passed.
This removes avoidable allocation from tab/workspace/window/quit checks without
adding polling or background work. Native close latency remains unmeasured.

Added a shared startup close-warning policy through RMUX_CONFIRM_CLOSE:
active (default), always or never. Tab/workspace/window/native-window-close and
Quit action paths use it consistently; empty scopes do not prompt. Invalid values
fail before layout/command shell preparation, while help/version remain available.
Prompt wording now describes termination/output loss without assuming every
prompt was triggered by reported work. The focused policy test passed for empty,
idle, active and invalid settings. Native dialog acceptance, OS-originated quit
routes, persistent settings UI and unreported foreground-task detection remain
pending.
The Quit rmux action now scans all rmux windows for reported active work and
shows one application-wide confirmation, defaulting to Cancel. Repeated Quit
actions are suppressed while it is pending; confirmation enters the existing
asynchronous process-cleanup path. No-active-work and last-window cleanup retain
their direct shutdown behavior. Desktop compilation passed. Native multi-window
quit/cancellation acceptance, OS-originated quit routing outside the rmux action,
warning configuration and non-reporting process detection remain pending.

Close Window now checks all workspaces for reported active work. The menu/shortcut
action and GPUI native-window close callback share a confirmation path, defaulting
to Cancel. A pending window prompt suppresses duplicate requests; a pending
workspace prompt must be resolved first. Empty/idle windows retain direct close.
The native callback is installed once per window and holds a weak root reference.
Desktop compilation passed. Native close-button, cancellation and multi-window
dialog acceptance remain pending, as do application-quit warnings and policy
configuration. This does not detect work that omits program-status reporting.

Close Workspace now checks all terminals for reported working/blocked tasks and
asks for confirmation before terminating the workspace. Confirmation resolves the
workspace entity rather than its list index, so reordering cannot redirect it;
removed or transferred targets are ignored. Empty-workspace cleanup and workspace
detachment retain their non-prompting ownership-transfer paths. Tab/workspace
checks share the same active-work predicate. Desktop compilation passed; native
dialog acceptance, window/app close routing and configuration remain pending.

Tab close requests now warn for live sessions with any working/blocked program
status record. The prompt defaults to Cancel; confirmation resolves the original
terminal identity and focus handle before removal, avoiding index drift or
cross-window rebind races. Repeated requests are suppressed while pending and
workspace rebind clears pending editor/prompt state. Transfer operations do not
go through termination confirmation. Desktop compilation passed; native prompt
acceptance remains pending. Workspace/window/app closing, configurable policy and
detection of foreground work that does not report status are still unfinished.

Added Focus Pane 1–9 palette actions and terminal-scoped shortcuts: Cmd+Option+
number on macOS, Ctrl+Alt+Shift+number elsewhere. They select the pane in traversal
order, retain its active tab/session and move zoom when zoomed. Missing pane
numbers are ignored. Desktop compilation passed; native shortcut dispatch,
keyboard-layout compatibility and zoom interaction remain pending. W04 and W10
implementation records now include numeric pane and named terminal selection.

Direct startup commands are now prepared before GUI initialization. An executable
spawn failure prints its cause to stderr and exits with code 2, rather than
opening an empty workspace. The first pane consumes the prepared session once;
subsequent panes still create normal shells. A real headless invocation with a
missing absolute executable built successfully and exited with code 2 and the
expected diagnostic. Successful command-window rendering remains unverified;
rmux still does not propagate a completed child's exit code to its launcher.

Direct-command tabs now use the program's basename as their fallback label,
bounded to 128 control-free characters. Command arguments are not displayed in
tab labels. Automatic OSC titles and explicit renamed tabs still take precedence.
Exit and post-exit input messages now say “process” so they describe both shells
and directly launched programs accurately. The focused real-process exit test
passed (status 7 and rejected subsequent input), and desktop compilation passed.
Native command-tab rendering remains pending; layouts still restore fresh shells,
not previously launched commands.

Startup accepts `rmux [-d PATH] -- PROGRAM ARG...` and runs the argv directly in
the first terminal, without shell interpolation. Arguments retain OS-string
boundaries; later tabs/panes use the normal shell. Commands inherit the launching
directory unless overridden, receive the terminal environment, and use existing
PTY output/exit handling. Layout and benchmark combinations are rejected. Focused
parser and real Unix PTY tests passed for literal metacharacters, Unicode/space
arguments, directory/environment and invalid empty commands; desktop compilation
passed. Native first-window rendering and Windows execution remain pending.
Spawn failures now report before GUI startup (see the newer startup entry above).
Completed child exit status is not propagated to the rmux launcher.

The command palette now lists every terminal in its window with workspace, pane,
tab number and display title, using existing fuzzy matching. Selection resolves
live IDs and reveals the destination pane through the shared terminal-focus path;
closed targets are ignored. Custom tab names now also appear in notification
cards, and tab/drag/palette/notification labels share one title resolver. Entries
are built when opening the palette rather than maintained by a polling task.
Desktop compilation passed; native switching, stale-target and zoom acceptance
remain pending.

Rename Tab is available through the menu and command palette, using the existing
text editor with Enter/Escape handling. Custom names override automatic OSC titles;
an empty submission restores automatic naming. Overrides live on terminal entities
so pane/workspace moves retain them, and optional snapshot fields preserve them
through manual save/restore. Input is bounded to 128 characters and control-free.
Desktop compilation and the focused snapshot test passed for older layouts,
Unicode round trips and invalid/oversized names. Native editing, transfer and
restore acceptance remain pending.

Automatic Bash startup experiment: the installed macOS Bash 3.2 did not execute
an ENV injection file under interactive --posix, including login/non-login and
--noprofile variants. A proposed private wrapper therefore failed its startup
test and was removed before enabling it in Session::shell_in. Existing login
startup and manual integration remain unchanged; terminal-core compilation
passed after removal. Do not assume Ghostty's POSIX/ENV injection works on this
Bash version. Automatic loading needs a version-compatible approach and isolated
startup-order coverage before being enabled.

Added manually sourced Bash integration for OSC 7 directory tracking and OSC 133
prompt/input boundaries. It preserves scalar PROMPT_COMMAND hooks (including
trailing comments), supports array hooks, avoids duplicate sourcing and replaces
its own prompt suffix after theme changes. A focused macOS Bash 3.2 fixture passed
for hooks, idempotence, prompt replacement, opt-out and Unicode/control-character
paths; it caught and fixed Bash 3.2's signed-byte printf behavior. The bundle recipe
now includes the script. Automatic Bash startup, command lifecycle markers,
modern Bash array-hook runtime coverage and native UI acceptance remain pending.

Last Terminal now selects the destination tab before the workspace selection
focuses its pane, avoiding a transient focus on that pane's previously active tab
and unintended unread acknowledgement. The source pane is remembered first.
Desktop compilation passed. The macOS bundle was refreshed; a new native-access
attempt confirmed the Mac remains locked, so interactive acceptance is still
pending.

Last Terminal is now available in the menu and command palette. Each window keeps
up to 32 recently focused terminal IDs, fed by existing focus/metadata events.
The command resolves the prior terminal against live workspaces/panes/tabs,
prunes stale IDs, selects its workspace and tab, and exits pane zoom to reveal it.
History owns no terminal/session references and follows tab moves within a window.
Desktop compilation passed. Native mouse/keyboard focus history, repeated toggles,
closed-tab fallback and cross-window detach acceptance remain pending; no default
shortcut has been assigned yet.

Scroll to Top and Scroll to Bottom are now explicit View menu and command palette
actions, using the existing terminal viewport path. This makes history boundaries
discoverable alongside Previous/Next Prompt and the existing keyboard navigation.
Desktop compilation passed; native menu/palette dispatch acceptance is pending.
Dedicated last-prompt behavior remains separate and unfinished.

Layout restore now checks every saved working directory before launching any
shells, including tabs in nested splits and other workspaces. Missing paths and
regular files fail preflight; cancellation is checked during traversal. The
focused test passed for nested paths, missing/file/directory transitions,
unspecified directories and cancellation. Spawn-time checks and rollback remain
in place for filesystem races or later process errors. Native restore acceptance
remains pending.

Startup now accepts --layout PATH and uses the same bounded snapshot validation
and fresh-shell preparation as manual restore. The prepared window is published
without creating a throwaway default shell. Preparation occurs before GUI startup;
errors report to stderr with exit code 2. Working-directory override and benchmark
mode combinations are rejected. Restore failure/cancellation now waits for owned
session rollback before returning, avoiding immediate CLI exit racing asynchronous
child reapers. Parser coverage passed for layout paths and conflicting/missing
arguments. Native startup rendering and partial-restore cleanup acceptance remain
pending; this does not resume processes or scrollback.

Session startup now validates every explicit working directory before allocating
the terminal or PTY. Missing paths and regular files produce a directory-specific
error instead of portable-pty's Unix fallback to HOME. This covers restored tabs,
inherited directories and startup overrides through the shared spawn path. A
focused test passed for missing and non-directory targets, verifying rejection
before executable lookup. Filesystem changes racing validation and spawn remain
subject to the underlying PTY adapter; this is not an atomic directory handle.

Added startup CLI handling for --help/-h, --version/-V and --working-directory/-d.
Help/version return before GUI initialization; unknown/missing/duplicate arguments
and inaccessible directories report an error with exit code 2. Paths remain OS
strings. The selected directory is passed explicitly as the fallback for new
shells; a focused terminal's reported directory takes precedence. This explicit
fallback is required because portable-pty otherwise starts Unix shells at HOME,
regardless of the app process cwd. Parser tests and a real headless --version run
passed. Native launch/directory inheritance and live-window CLI/RPC remain pending.

Idle compression cancellation now propagates to queued background clones through
a shared cancellation guard owned by the UI task. Workers check it before trying
the engine lock and again after obtaining it, so focusing a terminal, opening
Find or dropping the view skips queued work. A step already past the final check
can still complete its bounded call. Two focused tests passed for cancellation,
nonblocking busy-lock handling, changed history activity and weak ownership;
workspace compilation passed. Native scheduling and latency acceptance remain
pending.

Equalize Pane Sizes is now available through View and the command palette. It
resets sibling weights at every split to equal shares, cancels divider dragging
and exits pane zoom. Tree topology, leaf order, focused terminal and sessions
are retained; saved layouts naturally record the updated weights. A focused
nested-layout test covers preserved axes/identities and equalized weights.
Native command routing, resizing and visual acceptance remain pending.

Compression latency instrumentation now measures each bounded call and the first
post-compression scroll-to-top plus frame extraction. The focused 1/4/16 release
run needed 11 calls per terminal; maximum calls were 0.050/0.062/0.063 ms and
maximum history revisits 0.143/0.151/0.149 ms. Revisited frame contents were checked.
Raw timings and scope limits are recorded in benchmark docs. These observed
engine-only maxima do not cover contention, active PTY pressure, shaping/GPU
presentation or UI scheduling, and are not worst-case responsiveness guarantees.

Automatic idle compression is now wired for unfocused terminals with no Find
worker open. Output updates a timestamp and shares one pending task rather than
creating a timer per chunk. After an initial two-second delay, a background probe
captures Ghostty's activity token and waits another two quiet seconds before the
first step. Changed history activity restarts that delay; pending bounded steps
yield 16 ms, and a busy engine lock retries after 50 ms without blocking on it.
Focus or opening Find cancels scheduling. Completed/unsupported passes stop, with
a final UI check retaining output wakeups that raced completion. Compression work
uses a weak terminal handle, so it cannot retain a closed session; an already
dispatched bounded step may finish during cancellation. No compression redraws
are requested. Compilation and the focused activity/busy-lock/teardown test
passed. That test showed Ghostty's token does not change for every active-screen
write, hence the separate UI output-idle timestamp. Native scheduling, cancellation,
decompression latency and whole-app memory/idle-CPU measurements remain pending.

Added the bounded incremental Ghostty compression API and a focused history
preservation test. The test passes for copied Unicode scrollback before/after
compression and later output. The engine benchmark now measures post-compression
storage outside parser timing. A local 1/4/16 run reduced reported resident page
bytes from 4,096,000 to 470,684 per terminal (88.5%) for repetitive output; total
sequential compression passes took 0.284/1.115/4.838 ms. Raw data and limits are
documented in benchmarks. Automatic UI idle scheduling, per-step latency and
decompression interaction measurements remain pending; no whole-app memory or
responsiveness claim follows from this engine-only result.

Custom workspace icon and hex-color editors now use the existing inline metadata
field through File and the palette. Icons accept one bounded grapheme; colors
accept exactly six ASCII hex digits with an optional #. Empty input clears the
setting. Invalid input preserves the previous value and leaves the editor open
with an explanation. Undo/Redo and IME support come from the shared TextField;
saved-layout validation and workspace movement retain these values as before.
Workspace compilation passed. Native editor/visual acceptance, graphical pickers
and automatic persistence remain pending.

Workspace glyph icons now have View-menu and palette choices (Terminal, Code,
Build, Cloud, Notes, None), a fixed-width sidebar slot, and manual layout
persistence. Whole-workspace detachment retains the icon. Old files default to
None; imported values must be one nonblank grapheme, at most 64 bytes, without
control characters. Rendering uses text glyphs with no image asset cache.
Workspace compilation passed. Focused persistence coverage checks old files,
an emoji sequence round-trip and invalid values; native glyph appearance/menu
acceptance and a custom icon picker remain pending.

The release engine benchmark now records Ghostty resident/virtual/image bytes
after timing. A focused 1/4/16-terminal run completed: filled page storage was
4,096,000 bytes per terminal (3.91/15.63/62.50 MiB totals), with no image storage.
Throughput was 258.3/262.9/260.2 MiB/s and snapshot p99 0.160/0.141/0.141 ms in this
single sequential fixture. Raw JSONL and interpretation are recorded in the
benchmark docs. This establishes engine-storage scaling for this workload, not
whole-app memory, concurrent PTY scaling, input-to-pixel latency or a release gate.

Opt-in render-metrics dumps now include Ghostty memory accounting per terminal:
primary/alternate resident page bytes, reserved virtual bytes, compressed bytes,
image bytes and compression support. Values are queried only when a metrics dump
is requested, under the existing engine lock; no idle sampler or retained history
is added. Missing query results serialize as null. These figures exclude Rust
snapshots, fonts, GPU resources and other process allocations; compressed bytes
are already included in resident bytes, while image bytes are separate. A focused
engine test passed for primary allocation, lazy alternate-screen allocation and
screen preservation, and workspace compilation passed. This adds measurement
capability; it does not establish a new whole-app memory or latency result.

Text fields now support Option+Left/Right and Option+Backspace/Delete on macOS,
with Ctrl equivalents elsewhere, for Unicode word-segment navigation/deletion.
Shift extends from the existing anchor; whitespace is skipped between segments
and punctuation/emoji remain selectable segments. Word deletion participates in
the bounded undo history and expands endpoints to whole graphemes. The focused
test passed for combining accents, emoji, whitespace deletion and selection
anchors. Native platform shortcut acceptance remains pending.

UI text fields now have Undo/Redo in Edit with platform text-editing shortcuts.
History retains at most 32 prior states across undo/redo, including UTF-16
selection and direction; new edits discard the redo branch. With the existing
4 KiB input cap, retained text is bounded near 128 KiB per open field and is
released with that field. Composition records its initial state once and blocks
Undo/Redo while marked text is active. A focused test passed for Unicode text
and selection restoration, redo, branch invalidation and the 32-state bound.
Native IME/clipboard/shortcut acceptance remains pending. A fresh native UI
attempt still found the Mac locked; code work continues independently.

UI text fields now support double-click Unicode word-segment selection and
triple-click Select All. Cut is available in Edit and through Cmd+X on macOS or
Ctrl+X elsewhere, scoped to text fields; empty selections leave the clipboard
untouched and active composition is not cut. Multi-click gestures cancel the
single-character drag anchor. A focused word-selection test covers combining
accents, emoji sequences, whitespace, UTF-16 positions and end-of-document clicks.
Native multi-click, clipboard and IME acceptance remain pending.

Persistent text fields now support mouse drag selection and Shift-click extension
with a retained anchor and reversed-selection tracking. Click/drag hit testing
snaps to the nearest shaped grapheme boundary, matching keyboard behavior for
combining text and emoji. Release inside or outside the field ends the gesture;
keyboard edits and composition cancel pointer state. Active composition is left
to the platform input method. This uses pointer events and the existing caret
scroll positioning, adding no timer. Native drag/outside-release, horizontal
scrolling and IME interaction acceptance remain pending.

Find, palette and workspace text fields now move the caret and delete by Unicode
grapheme clusters rather than scalar values. Combining accents, emoji modifiers,
ZWJ sequences and flags stay intact during arrow movement, Shift selection,
Backspace and Delete. Keyboard deletion expands partial-cluster selections;
platform IME replacement ranges retain their existing UTF-16 semantics. The
already-resolved unicode-segmentation crate is now a direct desktop dependency.
All seven focused composition tests passed, including the new cluster cases and
existing surrogate/IME replacement tests. Native IME, pointer selection and
accessibility acceptance remain pending; terminal grid rendering is unchanged.

Find match navigation now has Edit-menu and palette commands plus Cmd+G /
Cmd+Shift+G on macOS and F3 / Shift+F3 elsewhere. Bindings target terminal/find
contexts, and invoking navigation with Find closed opens its input. Existing
worker navigation coalescing handles commands received while searching. Find-bar
arrow/close targets now reuse the 28-pixel pane-control styling with hover labels.
Code inspection confirmed refreshes remain event-driven on output/resize, with
no idle polling added. Workspace compilation passed with the scoped bindings;
native shortcut routing and visual acceptance remain pending.

Clear Scrollback is implemented in Edit and the command palette. It temporarily
sets Ghostty's history retention to zero, then restores the queried byte limit;
no bytes are injected into the parser or PTY. The UI cancels selection/autoscroll
and closes Find before history removal. A focused engine test passed for removing
history while preserving visible text, clearing selection, restoring future
history retention and continuing an escape sequence split across the clear.
Native command routing, alternate-screen behavior and visual acceptance remain
pending; this command does not clear the current screen or restart the shell.

Automatic zsh integration now has an end-to-end macOS PTY check using the real
Session and Ghostty engine. It verifies initial directory reporting, changing
into a path containing spaces/Unicode/percent, output arriving through the PTY,
semantic prompt navigation through generated scrollback and normal shell exit.
The fixture uses isolated startup files and the same wrapper preparation as
production. Its first run exposed an expected-path mismatch between macOS /var
and /private/var; canonicalizing the fixture path resolved it, and the check
passed. This proves the local shell-to-engine path, not native UI rendering or
other-shell/platform support. No broad regression suite was repeated.

New Unix zsh sessions now load the integration automatically. Embedded scripts
are written to a private temporary directory retained by the session, and startup
wrappers source existing user files in order, carry forward ZDOTDIR changes, then
restore the user's directory before .zshrc and normal .zlogin processing. RCS
disablement skips integration, and RMUX_SHELL_INTEGRATION=0 bypasses the wrapper.
No user startup files are modified. Workspace compilation and a focused real-zsh
startup test passed for unset/custom ZDOTDIR, .zshenv redirection, login file order,
RCS opt-out, loaded hooks and temporary-directory cleanup. The prior hook test
covers emitted markers, encoded paths and command status. Global startup files
that override ZDOTDIR before the wrapper, unusual prompt frameworks, Linux runtime,
native UI behavior and other shells remain unverified or unimplemented.

Added sourceable zsh integration with OSC 133 prompt/input/output/completion
boundaries and OSC 7 local directory reports. It uses built-ins only, appends
existing hook arrays, guards duplicate sourcing and wraps the prompt with one
zero-width input marker. Directory bytes are percent-encoded, including UTF-8
and control characters. The script activates only for interactive rmux zsh and
supports opt-out before sourcing. No user startup files are edited; automatic
loading remains pending. macOS packaging now copies the script into Resources.
Syntax checking and a focused real-zsh run passed for hook preservation,
idempotence, prompt/output markers, exit status, encoded paths and opt-out.
The test uses no user rc files and is available as `mise run test-zsh-integration`.
Native prompt rendering, prompt-theme compatibility, automatic loading and other
shell integrations remain open. This does not establish full shell integration.

Previous/Next Prompt now route View-menu, palette and terminal shortcuts through
Ghostty's semantic viewport navigation. macOS uses Cmd+Up/Down; other platforms
use Ctrl+Shift+Up/Down. Navigation is relative to the viewport top and requires
shell markers; no matching prompt is a no-op. The engine owns prompt tracking,
with no separate history index or periodic work in rmux. A focused OSC 133 test
passed for forward/backward navigation, clamping to the earliest marker and
unmarked-output no-op; workspace compilation passed. Native shortcuts, shell
integration and dedicated last-prompt behavior remain pending.

Select All now uses Ghostty's selection derivation and installs the result under
the same engine lock, without copying scrollback until Copy is requested. The
operation resets any ongoing selection gesture and cancels UI autoscroll. Edit,
the palette, Cmd+A on macOS and Ctrl+Shift+A elsewhere route to the terminal;
text fields retain their existing select-all behavior. Copy retains its existing
16 MiB bound. A focused engine test passed for history plus Unicode, alternate
screen isolation, returning to the primary screen and clearing selection. Native
shortcut, highlighting and clipboard acceptance remain pending.

Move Workspace to New Window is implemented in View and the command palette.
The destination retains the workspace, pane and terminal entities, split weights,
tab order/selection, zoom state, last focused pane, name, description, color and
pin flag. Terminal focus subscriptions and workspace pane-event subscriptions
are rebound to the destination window. Workspace subscriptions now have explicit
ownership per pane and are removed on pane removal, replacing detached observers.
The source entry is removed only after destination creation succeeds; failure
rebinds input to the original window. No replacement sessions are spawned.
Workspace compilation and the macOS app bundle build passed. Native repeated
movement, focus/IME, unread behavior, source-window closure and multi-pane lifecycle
acceptance remain pending. Movement into an existing window remains absent.

Workspace colors are selectable by name through the View → Workspace Color
submenu and command palette: blue, green, amber, red, purple, cyan, or None.
Both routes dispatch the same parameterized action. A small square alongside
the sidebar name carries the accent, separate from the unread dot. Color changes
notify the window without touching terminal state or adding periodic work.
Manual layouts retain optional 24-bit RGB values, defaulting older files to None.
Workspace compilation and a focused snapshot test for missing color, valid RGB
extremes/round-trip and out-of-range rejection passed. Native visual/menu acceptance,
an arbitrary color picker, workspace icons and automatic persistence remain open.

Workspace descriptions now have a File-menu/palette editor, a secondary sidebar
line, and manual layout persistence. The editor shares the existing focus/IME
path used for workspace names, accepts empty text to clear the description, and
caps input at 256 Unicode scalar values after trimming/filtering controls.
Snapshots default missing descriptions to empty and reject control characters or
values exceeding 1024 bytes. Descriptions remain independent of terminal metadata.
Workspace compilation and a focused snapshot test for older files, Unicode
round-trip, length limits and control rejection passed. Native editing/layout
acceptance remains pending; icons, colors and automatic persistence remain open.

Tab detachment is implemented through View and the command palette. It creates a
new window around the existing terminal entity and PTY, then removes the source
tab and any emptied pane/workspace. Window focus subscriptions are replaced;
transient find, composition, pointer state and rendering cache are reset while
terminal output, session, font size and metadata remain attached to the entity.
No replacement shell is spawned. Failed window creation retains the source tab
and restores its focus wiring. Workspace compilation and the macOS app bundle
build passed. Native testing was attempted but the Mac remained locked; detachment,
last-window lifecycle and input/IME acceptance remain pending. Moving tabs into
an existing different window remains absent.

Workspace pinning is available through View and the palette. Pin moves the active
entry after existing pinned entries; unpin moves it to the start of unpinned
entries, retaining workspace/session identity and focus history. Keyboard reorder
stops at the group boundary, and drag reorder clamps to that boundary. Sidebar
rows label pinned entries. Snapshot pin flags default false for older files;
restore stably groups pinned entries first and resolves the saved active workspace
by identity after ordering. Workspace compilation and a focused pin round-trip/
backward-compatibility test passed. Native ordering, restored focus and visual
acceptance remain pending; cross-window workspace movement is still absent.

Toggle Program Status now opens a per-terminal detail region through View or the
palette. It displays all bounded records sorted by ID, child-depth indentation,
inherited app names, state/blocked kind, progress, title and message. The region
has its own scroll area and close control, capped at 220 logical pixels/45% of
terminal height. Mouse events are stopped before reaching the underlying terminal
handlers. Record snapshots are copied on opening and status revision changes only
while open, then released on close; hidden details do not maintain a second full
record list. Focus acknowledgement refreshes open details. Workspace compilation
passed; native sizing, scrolling, accessibility and focus acceptance remain pending.

Fixed a program-status exit race: once the PTY owner exits or reading completes,
late buffered working/blocked/idle reports no longer recreate live work. Final
done/error reports can still arrive and remain until acknowledged. Reaper cleanup
now precedes publishing shell-exit status. A focused engine test passed for exit,
late active reports, a final completion result, repeated cleanup and acknowledgement.
The latest macOS bundle built successfully before this follow-up fix. A native
status/notification smoke attempt was again blocked by the locked Mac; no UI result
is claimed and native verification remains pending.

OSC 7501 now has tab UI projection. The store returns one owned summary only when
its revision changes, prioritizing blocked, error, done, working, then idle; the
latest updated record wins ties. Inherited app names resolve at read time, and a
count reports additional records without copying the whole store. Tabs show state,
blocked kind, optional progress and additional-record count, with app/title/message
on hover. Unfocused blocked/error/done reports mark attention. Focus acknowledgement
refreshes the tab summary synchronously, avoiding stale completion labels when no
new PTY bytes arrive. Workspace compilation and a focused priority/inheritance/
revision test passed. Native labels, hover accessibility, attention and lifecycle
acceptance remain pending; the UI does not yet expose every hierarchical record.

OSC 7501 callbacks now populate the record store from Ghostty-validated reports,
with bounded string copies and removal of invisible display-direction formatting
from titles/messages. Identity strings are rejected rather than truncated if
over-limit. Semantic prompt-start callbacks remove transient states; PTY child
reaping and reader completion also perform transient cleanup. Terminal focus
acknowledges done/error records. Reset is handled by Ghostty's root clear callback.
Workspace compilation and all four focused program-status tests passed, including
actual OSC decoding, blocked kind/base64 message, inheritance, prompt cleanup,
subtree clear, reset and a nonempty support-query reply. UI status projection is
not yet connected. Native process-exit ordering, focused-terminal completion and
full reply-content acceptance remain pending.

OSC 7501 storage groundwork now implements whole-record replacement, hierarchical
clear, dynamic nearest-ancestor app inheritance, prompt/exit cleanup and terminal
acknowledgement cleanup. It retains at most 64 records (the protocol's allowed
lower bound), evicting the least recently updated ID. Repeated updates refresh
eviction order; missing clears do not increment the revision. Three focused tests
passed for replacement/inheritance/subtree boundaries, lifecycle cleanup and
bounded eviction. The rules come from the pinned libghostty-vt terminal.h contract.
This is a tested storage component only: the callback, prompt/exit wiring and UI
are not yet connected, and rmux does not advertise OSC 7501 support yet.

OSC 9;4 progress reporting now flows from Ghostty's callback through the shared
metadata read into tab labels. Running percentages, error, paused and indeterminate
states are distinguished; indeterminate uses a static Working label rather than an
animation. Callback state is a small Copy value with no allocation, and unchanged
reports do not notify the UI. Remove reports and terminal reset clear progress.
Workspace compilation and a focused protocol test passed for running/error/paused,
indeterminate, remove/reset and separation from BEL/desktop notifications. Native
tab appearance remains pending. This does not implement program-status OSC 7501
or the CLI/RPC status/progress/log interfaces.

Empty-source cleanup during tab transfers no longer briefly focuses a surviving
source pane. Layout removal is now separate from focus selection: explicit close
selects a survivor, while same-workspace moves, cross-workspace moves and pane joins
remove source ownership without changing focus. Their destination selection stays
the only focus transition. This prevents a neighboring unread terminal from being
acknowledged merely because a different terminal was moved. Split collapse and
WorkspaceEmpty lifecycle events remain shared. Workspace compilation passed;
native unread-preservation regression verification remains pending.

Notification cards now have Dismiss controls. Dismissal checks the displayed
message revision before acknowledging it, so a stale card cannot dismiss a newer
OSC message. It hides that revision and clears the terminal's attention marker
without focusing it; later revisions appear again. The click stops propagation
to the card's focus action. Dismissal state stays with the live terminal and does
not add message storage. Panes containing unread tabs now show an amber border;
non-benchmark panes reserve the same one-pixel border when read, avoiding geometry
changes when attention arrives. Benchmark geometry is unchanged. Workspace
compilation passed; native dismissal, ring appearance and focus acceptance remain
pending. Dismissal acknowledges the terminal's aggregate attention, including BEL.

View → Toggle Notifications and its palette action now open a right-hand panel
listing the latest OSC message for each live terminal in the window. Cards show
workspace/terminal labels, unread state, title and body. Clicking resolves the
workspace/terminal entity IDs against current state, selects the matching tab,
updates zoom if needed and focuses it without visiting other tabs first. Closed
terminals disappear; moved terminals are found under their current workspace.
Panel content is collected only while open, using existing metadata notifications
for updates. No separate retained history or background timer was introduced.
The panel is 340 logical pixels, capped at 45% of the window, with its own scroll
region and close control. Formatting and workspace compilation passed; native
layout, message readability and click/focus acceptance remain pending. The panel
does not persist or provide native OS notification delivery.

Ghostty's desktop-notification callback now receives OSC 9 and OSC 777 requests.
The engine copies bounded title/body input (256/1024 bytes before lossy UTF-8
conversion), strips control characters and keeps only the latest Arc-owned message
and revision. UI metadata fetches share the engine lock and only clone changed
messages; coalesced PTY updates mark unfocused terminals unread through existing
tab/workspace markers and next-unread navigation. No OS service, timer or growing
history queue was added. Message display/history and OSC 99 remain absent.
Workspace compilation and a focused engine protocol test passed for OSC 9/777,
unchanged revisions, distinct BEL handling, OSC 9;4 progress exclusion, bounded
message copying and owned-message lifetime. The initial oversized OSC test was
discarded by Ghostty's 2048-byte parser buffer; the fixture now exercises our cap
within that parser limit. Native marker acceptance remains pending.

Last Workspace now returns to the previous visited workspace, then toggles back
on repeated invocation. View and the command palette expose the action without
assigning a new default shortcut. History stores one entity ID per window (no
retained workspace/session), and explicit switches, new-workspace creation,
next-unread navigation and tab transfers update it. Sidebar reordering leaves the
identity intact; closing the remembered target clears it. Existing remembered-pane
focus is restored by the common selection path. Workspace compilation passed;
native navigation acceptance remains pending. Full pane/tab focus history and
last-prompt navigation remain unimplemented, so W12 is only partially implemented.

New terminal tabs and splits now inherit the active source terminal's cell metrics,
so changing font size does not reset readability when adding adjacent work. New
workspaces retain the default size; benchmark splits explicitly retain default
metrics to keep their established fixture comparable. Workspace compilation passed.
The native zoom/grid check was attempted after a successful bundle build, but CUA
reported that the Mac was locked and automatic unlock failed. No native zoom
result is claimed; zoom/selection/IME and restored-size interaction checks remain
pending until the desktop is available. Implementation work can continue independently.

Layout snapshots now record each terminal's font size and apply it when creating
restored views, before their first paint. The optional version-1 field defaults to
14 for older files; validation rejects nonfinite and out-of-range (8–40) sizes
before restore starts shells. The five focused snapshot tests passed, including
font-size round trip, mixed old/new tab fields, invalid size rejection and atomic
save preservation. Native saved-layout zoom appearance remains pending. This
supersedes the earlier font-zoom slice's persistence limitation.

Per-terminal font zoom now supports Increase/Decrease/Reset actions through the
Edit menu, command palette and terminal-context Cmd+=/-/0 (Ctrl equivalents on
Linux/Windows). Sizes are bounded to 8–40 and reset to 14. Cell width/height scale
from the existing 8.5×20 default geometry. Shared CellMetrics now drive terminal
row shaping, backgrounds, decorations, cursor, PTY rows/columns, mouse reporting,
selection, wheel scrolling and IME preedit placement/hit testing. Row-cache keys
include cell metrics so font changes reshape retained rows; drag autoscroll ends
on zoom. Text-field UI remains at 14. Terminal entities retain zoom when moved;
new terminals use defaults, and layout snapshots do not yet persist font size.
A focused metrics test and macOS bundle build passed after compile fixes. Native
zoom/selection/IME, font-family configuration and DPI acceptance remain pending.

Palette search now supports per-word subsequence matching (for example `splt blw`
finds Split Below). Contiguous matches rank before fuzzy matches; fuzzy candidates
prefer fewer skipped characters, then earlier matches, with original command order
breaking ties. Empty queries retain original order. Normalized titles are cached
only for the lifetime of the open palette; query changes perform ranking without
background workers or idle polling. Matching walks Unicode scalar values rather
than slicing at arbitrary byte offsets; accent folding and locale collation are
not implemented. Two focused tests passed for ranking, word order, repeated-letter
rejection, empty queries and Unicode subsequences. An initial ranking fixture did
not contain the requested subsequence and was corrected. Native fuzzy-search
interaction acceptance remains pending.

Sidebar workspace rows now accept terminal-tab drops from another workspace in
the same window. Hover acceptance validates the source workspace, pane and terminal
IDs against current entities; cross-window and same-workspace drops are rejected.
The transfer method now accepts explicit source identities, so dragging an inactive
tab moves that tab rather than the currently active terminal. Palette and next-
workspace actions resolve their active terminal before calling the same method.
Existing workspace-row drag reordering remains on its own branch. Weak owner
handles in drag closures avoid retaining the window root. Formatting and workspace
compilation passed; native drag interaction remains pending. No background work
or new idle polling was added.

The command palette now includes named tab-transfer destinations for every other
workspace in the current window. It captures names/positions on open, retains
stable workspace entity IDs and resolves the destination at execution. A removed
target or the current workspace is a no-op. The Next Workspace transfer action
uses the same transfer method. macOS bundle compilation passed.

Native macOS debug targeted-transfer smoke passed: create three workspaces, set
`RMUX_WORKSPACE_MARK=alive` in Workspace 1's only terminal and print PID `75454`.
Search `move tab workspace 3` in the palette and execute the named result. Workspace
1 disappears, Workspace 3 remains selected (now sidebar position 2), and its pane
has the original destination tab followed by the selected transferred tab. Printing
the marker/PID returns `alive` and `75454`. This verifies the last-tab/empty-source
path and a nonadjacent destination for this fixture. Multi-pane, zoomed destination,
duplicate-name and cross-platform acceptance remain pending; cross-window detachment
is not implemented.

Move Tab to Next Workspace now transfers the active terminal entity into the
next sidebar workspace's remembered pane, wrapping at the end. It preserves the
tab label and session, appends after existing destination tabs and switches focus
directly to the moved terminal. A zoomed destination shows the target pane. Empty
source panes collapse through existing layout removal; an emptied workspace is
removed through the existing WorkspaceEmpty subscription. A one-workspace window
is unchanged. The action is available through View and the command palette.
Workspace compilation passed; native multi-workspace continuity, last-tab removal
and restore acceptance remain pending. Cross-window detachment is still absent.

Join Pane into Next appends the focused pane's tabs to the next pane in traversal
order (wrapping), preserving their order and active terminal. Batch transfer
rebinds metadata subscriptions and selects only the original active terminal,
avoiding focus/attention acknowledgement for every intermediate tab. The empty
source split collapses; zoom and divider drag end. Single-pane invocation is a
no-op. The action is in View and the command palette. macOS bundle build passed.

Native macOS debug smoke passed for a break/swap/join sequence: create a second
tab, set `RMUX_MOVE_MARK=alive` and print shell PID `74420`; use the palette to move
the tab into a right split, swap the focused pane to the left, then join it into
the next pane. The UI returned to one pane with two tabs, the moved tab selected.
Printing the marker and PID again returned `alive` and `74420`, confirming shell
continuity through this sequence. Focus styling followed the terminal at each
step. This does not cover multi-tab batch order, unread markers, below splits,
restored layouts or other platforms. No performance conclusion is drawn from
the debug Metal HUD. Full W09 acceptance remains pending.

Move Tab to Split Right / Below now moves a multi-tab pane's active terminal into
a new adjacent pane. The new pane retains the existing TerminalView, session,
label and terminal state; no shell is spawned or shutdown handle added. The
source tab is removed only after the layout accepts the destination. Traversal
order, metadata subscriptions and focus are updated; zoom and divider drag end.
A single-tab pane is unchanged. Both actions are in View and the command palette.
Formatting and the macOS bundle build passed with existing dependency warnings.
Native session continuity and interaction acceptance remain pending.

View → Swap Pane with Next and its command-palette action exchange the focused
pane with the next leaf in layout order, wrapping at the end. The operation
retains pane/tab/session entities and focus, preserves split axes and weights,
updates the traversal vector used by cycling and layout snapshots, cancels any
divider drag and exits zoom. A single pane or missing focused pane is a no-op.
The focused nested-layout test passed, covering geometry preservation and missing
targets. Native focus/session continuity after a swap remains pending; break/join
operations are still absent. No shortcut was assigned for this new action.

The command palette now includes workspace-name results with sidebar positions
to distinguish duplicate names. Selecting a result resolves the stored workspace
entity identity against the current window before switching, so reordering cannot
redirect it to another workspace and a removed target becomes a no-op. Results
are captured when the palette opens; native workspace-search acceptance remains
pending. Formatting and workspace compilation passed with existing dependency
warnings. This adds no background search worker or idle polling.

The feature ledger now records 20 families as `in_progress`, each with a partial
implementation description, source paths and remaining gaps. The other 112 remain
`not_started`; no family or phase is accepted. The roadmap defines this partial
status explicitly and the inventory no longer claims all implementation is absent.
JSON count and referenced source-path checks passed.

File → Close Workspace and the command palette now close the active workspace as
a unit. Explicit close and last-pane removal share one removal path: discard the
workspace's owned views/subscriptions, select a surviving workspace or close the
window when none remain. Terminal session drops use the existing asynchronous
reaper; no new process-killing mechanism was added. Bundle compilation passed;
native multi-pane close/cleanup verification remains pending. No new shortcut was
assigned, avoiding changes to the current Close Tab/Close Window bindings.

Workspace rows now show the focused/remembered pane's active terminal directory
as secondary text when the shell reports a valid local OSC 7 URI. Ghostty's PWD
callback increments a revision; metadata reads decode the path only after a
change, under the existing shared metadata lock. Terminal reset invalidates both
title and directory. UI labels filter control characters; shell inheritance keeps
the original path bytes. No filesystem polling or shell startup edits were added.
A focused engine test passed for OSC reporting, unchanged ordinary output and reset;
macOS bundle compiled. Native sidebar directory switching is pending. Git branch,
remote-directory metadata and automatic shell integration remain unfinished.

Startup redraw investigation: after a fresh CUA launch, invoking the window's AX
Raise action displayed the shell prompt and current title without clicking the
terminal. GPUI 0.2.2's macOS `window_did_change_occlusion_state` stops its display
link for occluded windows and restarts it when visible; CUA documents background
app launch. Together these support an automation visibility explanation for the
previous stale captures, rather than evidence that normal startup requires a
terminal click. Future native smoke tests should raise the app before judging
rendering. No forced redraw timer or framework patch was introduced. This does
not establish a startup latency measurement or cover every foreground launch path.

Native command-palette smoke passed: Cmd+Shift+P opened the overlay, typing `split`
filtered to two actions, Down selected Split Below, and Enter dismissed the palette
and split the original pane below, focusing the new terminal. The new pane's blue
accent and the other pane's muted accent were visually confirmed. Initial launch
again needed a click before content redraw resumed; that observation remains open.
Palette rows now also show shortcut labels resolved from GPUI's highest-precedence
binding in the original focus context, captured once when opening the palette.
This avoids a second hard-coded shortcut map. The shortcut-label addition builds
but has not had a native visual check.

A first command palette now opens with Cmd+Shift+P (Ctrl+Shift+P elsewhere) and
View → Command Palette. It lists 22 implemented actions, filters case-insensitive
title substrings for every query word, supports wrapped Up/Down selection,
Enter/click execution, Escape/outside-click dismissal and scroll-to-selection.
The shared text field supplies IME/editing, with arrow navigation disabled during
composition. Dismissal restores the previous focus before dispatching actions,
so pane actions target their original terminal. The overlay occludes mouse input
to content underneath. Bundle compilation passed; native rendering/action routing
are pending. Fuzzy ranking, action shortcut labels, dynamic enablement, custom
commands and workspace/file search remain unfinished.

Selected terminal tabs now distinguish keyboard focus: the focused pane in the
active window uses a bright blue accent and brighter text, while other panes keep
a muted selected-tab accent. Focus-in/out (including descendant find fields) and
window activation invalidate pane metadata so the indicator follows keyboard
navigation. This changes color only, without resizing the terminal viewport.
macOS bundle builds passed; native visual acceptance is still pending.

The PTY update path now reads bell/title metadata under one engine mutex lock,
instead of two, and emits at most one tab metadata event when both change in the
same coalesced update. Unchanged titles retain the allocation-free revision check;
title retrieval errors do not suppress bell delivery. This is a scoped overhead
reduction, not a new throughput/latency measurement. macOS bundle compilation
passed; existing native bell evidence predates this refactor.

View → Next Unread Terminal (Cmd+Shift+U; Ctrl+Alt+U elsewhere) now navigates to an
attention-marked terminal across panes, hidden tabs and workspaces in the current
window. Search starts in the active workspace, then wraps in sidebar order; pane
and tab ordering break ties rather than notification timestamps. The selected
workspace/tab are revealed, saved pane focus is updated, and existing pane zoom
follows the target. Navigation avoids briefly focusing an unrelated tab and thus
acknowledging its bell. No unread target is a no-op. Desktop check and bundle build
passed; native navigation is pending. Cross-window notification navigation and a
persistent notification history are still unfinished.

Workspace rows now aggregate terminal attention through pane/workspace metadata
events. Hidden workspaces retain an amber indicator until their requesting terminal
is focused; tab removal/transfer also invalidates the aggregate. No polling is
added. Native smoke passed: `sleep 3; printf '\a'` in workspace 1, then Cmd+2,
produced a sidebar marker after the delay; Cmd+1 returned and cleared it. Numbered
workspace selection and rendered shortcut hints were observed for positions 1/2.
Initial post-launch captures were stale until clicking the window; subsequent
redraws worked. Startup activation/redraw delay remains an unresolved observation,
not a proven renderer deadlock. Other attention scenarios remain unverified.

Terminal BEL now increments a wrapping counter in Ghostty's synchronous callback.
The existing coalesced PTY update path reads that counter and marks an unfocused
terminal as needing attention; pane tabs show an amber dot and clear it on active
window terminal focus. This adds no polling, timer or event queue per bell.
Tab title/attention updates share a `TabMetadataChanged` event. A focused engine
test covers multiple BELs, terminal reset and OSC title terminators (which must
not ring). Desktop compilation passed. Native attention rendering/clearing is
unverified. Workspace-level badges, notification center, sounds, OS notifications
and agent notification protocols remain unfinished.

Layout restore now allows one pending operation per originating window, shows
an opening status with Cancel, and uses an owned cancellation token. Closing the
originating window cancels its preparation; checks run before file access, between
shell launches, after preparation and before publishing the new window. Cancelling
retains the pending-operation guard until the worker returns, avoiding overlapping
replacement batches. Already prepared sessions drop through normal cleanup.
The focused cancellation test verifies owner-drop cancellation before file access;
desktop check passed. In-flight OS reads/spawns are not forcibly interrupted, and
native cancellation plus mid-batch process cleanup remain unverified.

Direct workspace selection now binds Cmd+1–8 to the first eight workspaces and
Cmd+9 to the last (Ctrl+Alt+1–9 on Linux/Windows). Missing numbered workspaces are
ignored. Sidebar rows show corresponding position hints, recalculated after
reordering. A scroll handle reveals the active workspace after selection,
creation, restoration and keyboard reordering without continuous scrolling or
polling. Desktop compilation passed; native key routing and hint rendering remain
unverified.

Native layout restore smoke passed for a two-workspace/four-shell fixture opened
through the macOS picker: `Restore Beta` became active, its left/right split was
1:2, sidebar width was 260px, and the saved second tab in the left pane was focused
with a `docs` prompt while the right pane showed `crates`. This exposed a hidden
selected tab in a narrow tab strip. Pane tab strips now use GPUI's scroll handle
to reveal the active item after restore, selection, reorder, close, transfer and
title changes. The correction builds; native tab-reveal verification is pending.
The fixture is in `target/restore-smoke.json` (local disposable build output).

File → Open Layout in New Window… now uses the native file picker, bounds reads
to 2 MiB plus one byte, validates the saved schema, and prepares all fresh shells
on the background executor. It creates a new window only after preparation
succeeds, with saved workspace names/order, active pane/tab/workspace, nested split
weights, tab labels, reported directories and sidebar visibility/width. Partial
preparation is dropped on error; existing windows are not replaced. Restored panes
reuse the normal close/move subscriptions and shutdown registry. Desktop check
and macOS bundle build passed. The native restore case above passed; resource
rollback and large-layout responsiveness remain unverified; automatic saves/recovery remain
unfinished. Saved directories that cannot start a shell fail the restore rather
than silently substituting another directory.

Pane zoom is available through View → Toggle Pane Zoom and Cmd+Shift+Enter on
macOS (Ctrl+Shift+Enter elsewhere). It mounts only the selected pane while retaining
the original split tree, weights and terminal entities. A visible restore strip
returns to the layout; pane cycling retains zoom and selects the next pane.
Adding/closing a pane restores the split view. Directional focus is currently
inactive while zoomed; zoom state is not exported in layout snapshots. A native
smoke check verified the shortcut expanding the right pane and clicking Restore
returning to two panes with the marker output preserved. Process identity was not
independently measured, and cycling/structural changes while zoomed remain untested.

The workspace sidebar now has a draggable 5px divider, a 160–480px preferred-width
range, double-click reset to 200px, and viewport clamping that reserves terminal
space in narrow windows. Drag events are captured before terminal mouse handlers,
without timers or background polling; hiding the sidebar cancels a drag. Save
Layout records the preferred width with a backwards-compatible default for earlier
version-1 files. Snapshot tests cover round-trip, invalid widths and legacy files.
Native drag/reset interaction passed: the divider moved from x=404 to x=544
in a 2× screenshot, then double-click returned it to its original position.

UI parity with cmux is an explicit active-goal requirement: spacing, hierarchy,
clean visual design and shortcuts must be compared against the reference rather
than inferred from feature availability. The current prototype is not visually
accepted. A read-only native cmux comparison confirmed right-aligned pane controls,
an accent above the selected tab, and an active-workspace header. The first pass
adds a 32px tab strip with fixed 28px new-tab/split-right/split-down controls,
shortcut tooltips, vector split glyphs and a selected-tab accent. The prototype
shortcut banner is replaced by the current workspace name. Native testing verified
both split buttons, including clicking Split Down in an unfocused left pane while
the right pane had focus: only the left pane split. Tooltip text and the tab accent
were visually observed. The workspace-header replacement was also visually checked
in the zoom/sidebar smoke run. Browser controls, sidebar metadata,
titlebar integration, accessibility and full visual parity remain unfinished.

Layout persistence now has a versioned JSON schema and File → Save Layout… using
the native save picker. It captures the current window's workspace names/order,
active workspace/pane/tab, nested split proportions, tab labels and available
shell directories. Disk encoding/writing runs in the background and replaces the
destination through a synced temporary file. Limits cover versions, depth, tab
counts, file size, indices and finite positive weights. Format round-trip and
invalid-state tests passed; native file-picker interaction remains unverified.
Loading/restoring is now implemented as described above; automatic saves and crash
recovery are not implemented yet.
Snapshots describe fresh shells, not process or scrollback resumption. Non-UTF-8
directory paths currently produce an explicit serialization error rather than a
lossy saved path.

New tabs and splits inherit the active terminal's shell-reported current directory
through Ghostty's OSC 7 state and `Session::shell_in`. Directory reads happen only
when creating the new surface, without polling or per-output string allocation.
The local file-URI parser decodes percent escapes, supports Unix non-UTF-8 path
bytes, recognizes localhost/current host, and rejects remote hosts, UNC paths,
malformed escapes and embedded NUL. Absent/unusable reports use the existing shell
startup behavior; no shell files are modified. Two focused tests passed for URI
decoding/rejection and live OSC report/reset handling; desktop compilation passed.
Native shell inheritance and Windows path handling remain unverified. Automatic
shell integration installation and persisted directory restoration are unfinished.

The shared find/rename text field now supports Shift+Left/Right and Shift+Home/End
selection with a fixed anchor, reversed caret placement, and matching native
UTF-16 selection direction. macOS Command+Left/Right moves to the text edge.
A focused test passed for astral Unicode boundaries, shrinking/reversing a
selection and collapsing it without Shift; the desktop test target compiled.
Movement is by Unicode scalar, not full grapheme clusters; word motion and mouse
drag selection in text fields remain unfinished.

Tab drag now also moves an existing terminal entity into another pane in the same
workspace. The workspace coordinates removal/insertion, activates the moved tab,
and collapses an empty source pane. Title subscriptions are owned per tab,
removed from the old pane and installed in the destination; closed tabs also
release their observer. This avoids accumulating detached title observers across
moves. The PTY, scrollback, find state and terminal view are retained. Scope IDs
reject cross-workspace/window drops. Desktop compilation passed; native transfer
and process-identity verification remain pending.

Drag reordering is implemented for tabs within their pane and workspace rows
within their window, with a labeled drag preview and highlighted drop targets.
Drops resolve stable source/target entity IDs, preserve existing entities and
activate the moved item. Workspace drag is disabled while its rename editor is
open. A desktop compile check passed; native drag behavior remains pending.
Cross-workspace/cross-window moves, insertion-edge previews and drag autoscroll
remain unfinished.

Workspace rename is implemented through Cmd+Shift+R / Ctrl+Shift+R and File →
Rename Workspace. The sidebar opens an inline editor with the existing name
selected; Enter saves, Escape cancels, and both return focus to the remembered
terminal pane. Empty names retain the existing name; saved names are trimmed,
control-filtered and limited to 128 characters. Workspace selection updates the
native window title. Find and rename share a text-input entity with existing
Unicode/IME, paste/copy and select-all behavior. Desktop compilation passed;
native rename and post-refactor find behavior remain pending. Names are not yet
persisted across restarts.

Directional pane focus now uses current visible terminal bounds, preferring
aligned neighbors and then nearest facing edges/centers. Cmd+Ctrl+Arrow on macOS
and Alt+Shift+Arrow elsewhere navigate without wrapping; layout-order cycling
remains available. Native View menu entries expose the four directions. A focused
test passed for a resized nested layout, all directions, outer edges and an
unlaid-out pane; the desktop test target compiled. Native shortcut behavior has
not yet been checked.

Separate native window creation and closing are now implemented. Cmd+Shift+N /
Ctrl+Alt+N creates an independent workspace window; Cmd+Shift+W / Ctrl+Alt+W closes
the focused window. The app-wide shutdown registry remains shared, and closing
the last window takes the existing quit/cleanup path. Added native menus for the
main file, edit and layout actions, plus an explicit window title. One desktop
compile check passed. Multi-window native lifecycle validation, moving workspaces
between windows and window restoration remain outstanding.

A short macOS debug smoke check now confirms the basic find flow, match count and
highlight, Escape, tab creation/content retention, nested splits, divider dragging
and last-tab pane collapse. The test app exited. See
[the scoped native evidence record](benchmarks/p1-native-smoke.md) for the build
hash, scenarios and unchecked cases. This supersedes the earlier "native pending"
notes only for those specific interactions; no full feature or phase is accepted.

Per the user's latest direction, Windows validation is deferred and short feature
iterations take priority over broad validation. P1 layout work now proceeds while
P0 acceptance gaps remain visible. Nested row/column splits replace the flat
normal-mode layout; split right/below targets the focused pane, close collapses
empty branches, and surviving pane entities retain their sessions. Focus cycling
follows tree order. The existing equal-width benchmark fixture is preserved.
One focused layout test passes and compiles the desktop target. Sidebar workspaces
are now implemented with create/select/cycle actions, sidebar collapse, retained
workspace entities and last-pane focus restoration. Only the active workspace is
mounted in the visible layout; inactive terminals stay alive. An empty workspace
is removed, and the window closes only after the last workspace is gone. A single
desktop compile check passed for this iteration; native UI verification was not
run.

Per-pane tabs now support creation, selection, closing and keyboard reordering.
Only the selected terminal is mounted; hidden tab entities retain their sessions
and scrollback. Closing the last tab removes its pane and collapses its split.
Workspace order can also be changed by keyboard without recreating sessions.
The benchmark fixture hides tab chrome. Shell OSC 0/2 titles update tab labels
through Ghostty callbacks; unchanged titles do not allocate a new string or
notify the tab strip, and displayed titles are bounded to 128 characters. Full
terminal reset clears the title. A targeted test covers fragmented OSC input,
ordinary-output suppression, Unicode title bounds and reset; it passed, as did
the desktop compile check. Native tab UI and shortcut verification remain pending.
Cross-container drag movement and persistence remain unfinished. Windows
validation remains deferred per user instruction.

W03 divider resizing is now implemented for nested row/column splits. Dragging
updates only adjacent branch weights; pane and terminal entities stay intact.
Double-click equalizes the adjacent branches. Dragging clamps each adjacent
branch to 48 logical pixels when space permits (half the pair when it does not).
Closing a branch preserves surviving relative weights, and structural layout
changes discard any active drag. Benchmark dividers remain thin and noninteractive.
Two focused layout tests passed, including resizing bounds, surviving weights,
and nested collapse; this also compiled the desktop target. Native drag behavior
has not yet been verified, so W03 is not marked verified.

The already-running Linux desktop link attempt finished with missing target
libraries `xcb`, `xkbcommon`, and `xkbcommon-x11`. No further platform validation
or library installation was pursued in this iteration.

## P0 — in progress

### Latest P1 search iteration

Added an on-demand search engine wrapper around Ghostty's incremental search API.
Each step performs at most one bounded feed and one bounded tick. Feed, query
replacement, navigation and teardown serialize with the PTY parser; scan ticks
use copied data without holding the terminal mutex. Search exposes match counts,
selected index and pending status, refresh after terminal changes, and older/newer
navigation that scrolls to and highlights the selected match. Empty queries release
results. Queries are capped at 4096 bytes. Current matching follows Ghostty:
ASCII case-insensitive, otherwise byte-exact; regex and match-case controls are
not implemented.

Two focused engine tests passed for scrollback counts, bidirectional navigation,
query clearing, output refresh, Unicode across soft wraps and alternate-screen
transitions. The on-demand background worker is now implemented: latest-query
coalescing, bounded navigation, coalesced refresh/wakeup/result notifications,
revision-tagged results, and cancellation on drop. It waits on a channel when
caught up and checks cancellation between bounded scan steps. A focused worker
test passed for query replacement, navigation, output refresh and shutdown.
The find bar is now integrated into each terminal: Cmd+F / Ctrl+Shift+F opens it,
Enter and Shift+Enter navigate older/newer matches, and Escape or × closes it and
returns focus to the terminal. It shows selected/total counts and pending state,
refreshes after PTY output or terminal resize, and highlights the selected match.
The query field uses native text-input/IME hooks, UTF-16 range conversion, a
4096-byte limit, horizontal caret scrolling, paste/copy and select-all. A focused
persistent-editor Unicode/IME commit test and desktop compilation passed. Native
find interactions, full editing conventions, all-match highlights and search
performance remain unverified or unfinished; search parity is not claimed.

### Latest P1 selection iteration

Terminal selection now uses Ghostty's tracked gesture state and owned active
selection. Implemented drag selection, native double/triple-click word/line
behavior, Alt-drag rectangles, Shift override for mouse-reporting applications,
per-row highlight ranges, and Cmd+C / Ctrl+Shift+C clipboard copy. Extraction
uses the Ghostty formatter with soft-wrap unwrapping and a 16 MiB output limit.
Gesture/event handles are reused; clipboard text is allocated only on copy.
Fractional cell widths are normalized before passing integer geometry to Ghostty.
Two targeted tests passed for Unicode word/line copy, retained selection during
output, selection clearing/highlighting, fractional-width drag, rectangles and
release. Desktop compilation also passed. Native pointer/clipboard behavior,
keyboard selection and selection performance remain unverified or unfinished;
full selection parity is not claimed.

Selection edge autoscroll is now implemented using Ghostty's autoscroll gesture
ticks. Pointer capture keeps an active drag working outside its original pane.
A 30 ms one-shot timer is rearmed only while dragging beyond a vertical edge
with history available in that direction. Release, focus loss, pointer reentry,
gesture invalidation and reaching the history boundary stop the timer. Three
focused selection tests pass, including scrolling to both ends of history and
stopping; desktop compilation passed. Native pointer capture and timer behavior
remain unverified.

The macOS prototype builds and runs using mise-managed Rust 1.99.1, GPUI 0.2.2, portable-pty 0.9.0 and the official checksum-pinned libghostty-vt 1.0.1 library. Apple Metal Toolchain installation was explicitly approved by the user and completed. No Homebrew installation is part of setup.

Implemented:

- Owned Ghostty FFI wrapper with dirty-row snapshots and shared unchanged rows.
- PTY reader, bounded asynchronous input queue, coalesced redraw notifications and child cleanup on session drop.
- GPU text drawing with cell positions, colors, basic styles, Unicode clusters and a cursor.
- Faint text blending, strikethrough, overline, underline color and curly underline mapping. Terminal-requested bar/block/underline/hollow cursor shapes and explicit cursor colors are carried into drawing; block cursors repaint visible cell text in contrast and preserve hidden-text behavior.
- Bounded per-visible-row shaping cache: unchanged engine row identities reuse shaped text; changed rows rebuild. DPI/background changes invalidate the cache; pane movement applies offsets at paint time. Background spans are batched independently of text and include wide/blank cells.
- Interactive login shells, terminal resize, basic clipboard paste and side-by-side panes.
- Terminal-state-aware control keys through Ghostty's encoder, including application cursor mode and modifiers; bracketed paste with control-byte sanitization.
- Project-local library bootstrap and macOS application bundle tasks.
- Release bundles and an opt-in clean-shell benchmark workload (`RMUX_BENCH_PANES`), plus read-only macOS idle sampling scripts.
- Portable PTY pressure harness (`mise run bench-pty`) with 1/4/16 sessions, distinct response markers, per-request deadlines and parsed-output progress checks. Initial macOS results are recorded in [PTY baseline](benchmarks/pty-baseline.md); GUI latency and repeatability are not established.
- Opt-in fixed-size render diagnostics via `RMUX_RENDER_METRICS` and F12: pane paint/prepaint counts and CPU durations, row-cache reuse/rebuild counts, parsed bytes, notifications and focus state. No idle polling or automatic file writes. Workspace check passes; native metric capture remains pending. See [diagnostic procedure](benchmarks/render-diagnostics.md).
- Child exit status reporting, rejection of input after exit, and application shutdown when the last window closes.
- Focused-pane close and forward/backward focus cycling. Session construction now arms cleanup before spawning workers and starts the reaper first, so partial startup errors retain cleanup ownership. Already-reported exited children are not signaled again on normal session drop.
- Linux source bootstrap with mise-pinned Zig 0.17.0; successful x86_64 Linux GNU library cross-build on macOS. Windows checksum-pinned package bootstrap and DLL bundle scripts are present but not native-tested. Build scripts now resolve the project-local dependency on all target OSes.
- Linux/Windows shortcut and font defaults, plus Windows branches for PTY/ConPTY session tests. Native Windows/Linux desktop validation is still pending.
- Ghostty-backed scrollback navigation through wheel/trackpad, Shift+PageUp/PageDown and Shift+Home/End; a position marker appears in history. Accepted input returns to the live viewport without adding a history buffer or idle timer.
- Ghostty mouse encoder integration for left/middle/right press/release, motion and vertical wheel reports. Tracking modes and wire format come from terminal state; Shift+wheel overrides reporting for local history. Motion reports do not force the viewport to live output. Encoder/geometry state is retained so same-cell motion can be deduplicated.

Verification completed November 19, 2026:

- Workspace builds with build-time Metal shaders.
- Terminal parsing/style/Unicode/cursor/query-reply and real PTY output tests pass.
- A focused interactive-input-after-resize test passes.
- Input encoding tests cover application cursor keys, Ctrl+C, Shift+Tab, native-text fallback, paste newline conversion and bracketed-paste framing/sanitization.
- A shell exit-status test verifies nonzero exit reporting and input rejection after exit.
- Native window smoke check confirms a shell prompt, executed command output and a second pane via Cmd+D. Explicit interactive shell startup fixed the initially blank window.
- Cached-renderer smoke check confirms red backgrounds across wide/blank cells, combining characters, underlined trailing spaces, and reflow after a split. Workspace check and debug bundle build pass. A before/after throughput measurement has not yet been made; earlier benchmark numbers refer to the earlier binary.
- Scrollback tests cover history retained during new output and alternate-screen transitions. Native scrollback smoke check reached the oldest fixture output and returned to the live prompt on paste. Selection/copy, search and a draggable scrollbar are still pending.
- Seven terminal tests now pass. Mouse coverage checks disabled reporting, SGR press/release/wheel bytes, tracking-dependent motion suppression, repeated-cell deduplication, fractional-width cell coordinates, pixel reporting and mode disable. Workspace check passes. Native TUI mouse interaction, multi-button chords, horizontal wheel and mouse cursor-shape requests remain unverified or unfinished.
- The eighth terminal test exercises eight sequential create/drop cycles and verifies both shell exit reporting and release of the terminal state. Debug bundle build passes. Native pane-close/focus-cycle verification was attempted but the computer-use tool failed and reset its session; those UI actions remain unverified. Startup thread-failure injection, stubborn children/descendants, writer-thread retention and long-run resource plateaus still need coverage.
- Nine terminal tests now pass, including faint/style extraction, colored curly underline, hidden text, DECSCUSR cursor shapes, OSC cursor color and cursor visibility. Workspace check passes. New decoration/cursor drawing has not received a native visual check. Double/dotted/dashed underlines currently fall back to a single line; text/cursor blinking is not yet scheduled.

These are functional checks, not performance benchmarks. The debug Metal HUD is not a reliable baseline for release memory, idle rendering or input latency.

IME composition now retains UTF-16 selection and marked ranges, applies explicit
partial replacements, preserves surrounding pending text, and normalizes ranges
without splitting surrogate pairs. Unmarking commits pending text once; an empty
marked-text replacement cancels it. Four desktop unit tests pass, covering Japanese
composition, partial replacement, emoji offsets, commit and cancellation. The
desktop test target compiles and links with GPUI. `mise run test` now includes both
workspace crates; all 13 tests pass. Candidate bounds and point hit testing now
use the same shaped glyph positions as preedit rendering, converting between
UTF-8 byte indices and UTF-16 platform offsets. The caret follows the composition
selection; a background and selection highlight keep it readable over terminal
output. These native geometry/drawing changes still need an input-method UI check.
Multiline/wrapped preedit, bidirectional selection geometry, and reconversion of
already-sent terminal input remain unfinished.

Keyboard encoding now accepts press/repeat/release actions. A focused test checks
Kitty event reporting after protocol negotiation and suppression after restoring
legacy mode, including releasing Ctrl before the letter key. The desktop pairs
releases only with encoded presses, and releases outstanding keys when the pane
loses focus. Releases do not return the viewport to live output. Encoding uses a
stack scratch buffer, avoiding a heap allocation for suppressed releases. All 14
workspace tests passed, the additional modifier-release assertion passed, and
the final workspace check passed. Native release/focus transitions remain
unverified. Printable Kitty press reporting, physical/non-US key mapping and
window-deactivation cleanup were still open at that check; native text and IME still own ordinary
printable input.

Focus reporting now uses Ghostty's focus encoder when DEC mode 1004 is enabled.
Pane-focus and window-activation subscriptions share a deduplicated focus state;
losing focus releases tracked keys before sending focus-out, including when
switching away from the application. Focus reports and releases preserve the
scrollback viewport and add no polling or idle timers. A focused test verifies
disabled/enabled/disabled reporting and both focus sequences; all 15 workspace
tests pass. Native activation/pane-switch ordering remains unverified, as do
mouse-button release reporting on deactivation and input-method focus transitions.

Next short iterations:

1. Complete keyboard protocol integration for printable keys, key releases, non-US layouts and IME; control keys and bracketed paste now use Ghostty.
2. Measure release idle CPU, memory and output throughput at 1/4/16 panes with repeatable workloads.
3. Improve shutdown/error reporting and test shell lifecycle.
4. Validate Linux/Windows dependencies and rendering on those platforms.
5. Begin P1 workspace sidebar and nested split layout after the P0 gates are met.

Still missing: selection/copy, scrollback search, complete terminal styles, robust IME behavior, accessibility, complete mouse interactions, persisted workspaces and native validation of cross-platform shortcuts/packaging. Feature inventory acceptance criteria remain authoritative; scaffolding does not imply feature parity.

`mise run bench-engine` builds a release-only engine harness and emits JSON for 1/4/16 terminals. It uses 160×48 cells, 4 MiB scrollback budgets, styled Unicode output and periodic snapshots after warm-up. This isolates parsing/snapshot cost; it does not measure PTY throughput, input-to-display latency, CPU at idle or GPU memory.

An initial [release app measurement](benchmarks/app-baseline.md) now covers one/eight idle clean shells and basic shutdown. Idle CPU exceeded the provisional target and memory did not establish a stable baseline; P0 performance is still open. Before P1, add visibility/redraw instrumentation, resolve idle cost and validate the embedding path on Linux/Windows as required by the roadmap.

A subsequent [instrumented one-pane run](benchmarks/render-diagnostics.md) captured
zero additional terminal paints across 92 seconds and approximately 0.02% of one
core over a nested 60-second interval. This supplies native evidence that the
terminal can remain render-idle; it is not a controlled comparison with the earlier
high CPU observation. No speculative renderer patch was made. Multi-pane native
diagnostics, physical memory attribution and cross-platform gates remain open.

The same instrumented release then ran eight idle panes: zero additional paints
over 92 seconds, approximately 0.02% frontend CPU over 60 seconds, and physical
footprint 49.8M to 49.9M with a 275.4M startup peak. Native reverse focus cycling
and closing removed the expected pane and its shell; the other seven stayed
alive until normal quit removed all recorded processes. See the diagnostics
report for evidence and limitations. These observations do not establish latency
under GUI output pressure, long-run retention or cross-platform acceptance.

Row text batching is now isolated from GPUI shaping so its column/style decisions
can be checked without a native window. Two focused tests pass for color-run
boundaries, wide-cell tails, trailing blank trimming, underlined spaces and empty
decorated cell representations. Empty decorated cells retain a column in the
renderer, and batching stops at noncontiguous columns. Ghostty's erase-character
operation clears underline styling, so no erase-related rendering bug is claimed.
Native decoration appearance remains a separate verification requirement.

Synchronized output (DEC 2026) now retains the previous snapshot until Ghostty
reports the hold released. Parsing and PTY replies continue while held. A 250 ms
deadline clears an abandoned hold; the UI schedules at most one pending timeout
per pane and suppresses output-triggered redraw notifications during the hold.
There is no idle polling. The callback state shares the existing stable allocation
used for PTY replies. A focused test covers unchanged row ownership while held,
release, repeated-enable behavior, timeout recovery, a new hold after timeout,
resize and full reset. All 18 workspace tests pass. Native timer/presentation
behavior under a synchronized-output workload still needs verification; the prior
release idle measurements precede this change.

Native macOS synchronized-output smoke check passed using
`scripts/fixtures/synchronized-output.sh`: the baseline appeared first, then the
held text appeared while the producer was still in its ten-second no-output
interval, before the release line. The release line and shell prompt subsequently
appeared, and normal quit closed the test app. This verifies timeout-driven native
recovery without another PTY write, not the exact 250 ms presentation latency or
absence of partial frames within a normally released transaction. The debug
binary SHA-256 was
`70956955f2fa6f6eaa092f227ec3993709c72a241abfd8889e75527bea0f4d59`.

macOS pane teardown now uses a kqueue process/user-event wakeup. The reaper retains
exclusive child ownership until natural exit or a close request; on close it can
invoke portable-pty's owning-child termination path (SIGHUP, grace period, then
forced termination). The UI does not wait through that grace period, and no idle
polling was introduced. This avoids adding a delayed raw-PID kill racing with
reaping/PID reuse. Each live macOS session currently owns one additional kqueue
descriptor, marked close-on-exec. Existing session tests pass, plus a focused
macOS test verifies a shell ignoring SIGHUP terminates and releases terminal state
after pane drop. Linux/Windows retain their existing kill paths. Process-group
descendants and app-wide graceful shutdown that waits for all reapers remain open;
pane teardown does not prove those cases.

The rmux Quit action and last-window-close path now request all registered shell
shutdowns before invoking GPUI quit, then asynchronously await reaper completion
with a three-second upper bound. Requests are issued together; the UI remains
event-driven while waiting. Lightweight shutdown handles retain neither PTYs nor
terminal state, and finished handles are pruned when adding a session. This also
covers recently closed panes whose reapers are still working. The stubborn-shell
test now retains a shutdown handle and verifies completion plus terminal release.
All 19 workspace tests passed before that additional assertion; the strengthened
focused test passed afterward. Native application-quit verification is pending.
GPUI's final OS-driven shutdown hook only allows 100 ms, so externally initiated
termination that bypasses rmux's Quit action, forced termination, and descendants
remain outside this guarantee. Linux's existing SIGHUP-only path may reach the
three-second deadline for a stubborn child and still needs escalation support.

Native quit follow-up: debug binary
`df35a9fd10fc7aea635cd41a237de7a187274c1f4ad998c2d40a183d94600168`
exited normally with both clean test-shell PIDs removed. The attempted interactive
SIGHUP trap could not be confirmed because native automation returned inconsistent
window state. A subsequent fixture launch via bundle `SHELL` override actually
started `/bin/zsh -i -l`; it was also quit and its recorded PID removed. Neither
attempt verifies native stubborn-shell quit. The reusable
`scripts/fixtures/hangup-resistant-shell.sh` wrapper is syntax-checked and remains
available for a launch with a verified environment. No broad shutdown guarantee
is inferred from these normal-exit observations.

Linux now has a process-wakeup implementation using `pidfd_open`, `eventfd` and
blocking `poll` with no periodic timeout. It uses the same exclusive owning-child
reaper and termination escalation as macOS, rather than a cloned SIGHUP-only
signaller. File descriptors are close-on-exec; failures to obtain a process
descriptor abort session startup with an explicit error instead of silently
falling back to weaker cleanup. This requires a kernel and sandbox policy that
permit pidfds. The stubborn-shell test is enabled for Linux as well as macOS.
All five session tests still pass on macOS after separating the platform backends.
Linux compilation/runtime validation is pending. Existing OrbStack Docker was
available, but its existing test image contains no mise or compiler; no tool
installation was performed. This backend does not establish Linux desktop support.

The Linux terminal backend now passes `cargo check` and its unit-test executable
cross-links for x86_64 GNU Linux using mise-managed Rust 1.99.1 and Zig 0.17.0.
The Rust target was installed through the project-local mise target option.
Reproducible tasks are `check-linux-terminal` and `build-linux-tests`. Execution
in the existing ARM64 container failed before tests started because the x86_64
dynamic loader is absent; Zig rejected a fully static glibc link. Runtime Linux
tests and desktop compilation remain pending. These are compilation results,
not evidence of working Linux PTYs or rendering.

ARM64 Linux runtime milestone: all 13 terminal-core tests passed in an existing
Debian 13.7 ARM64 container on OrbStack, including real PTY output/input/resize,
exit reporting, repeated session teardown, and a shell ignoring SIGHUP. The
container used no network, mounted test artifacts read-only, and installed no
tools. Rust's ARM64 target was installed through mise; Ghostty was cross-built
with the pinned Zig. Zig rejected Rust's Cortex-A53 erratum linker flag, so the
run explicitly opted into removing that flag for this Apple Silicon test host;
the portable default still rejects it. This is not a portable ARM64 release
artifact. [Execution record](benchmarks/linux-arm64-terminal-tests.txt).
Linux desktop rendering, Wayland/X11, native IME and Windows remain unverified.

Windows terminal cross-check now passes for `x86_64-pc-windows-msvc`, including
test code. The target was installed through mise, and the already-downloaded
Ghostty Windows package was checksum-verified before extraction. The first check
caught eight compiler errors caused by assuming unsigned C enum bindings in key
arithmetic and color-tag matching. Those operations now use the generated
`GhosttyKey` and `GhosttyStyleColorTag` types. All 13 terminal tests pass on macOS
after the fix. `mise run check-windows-terminal` preserves this check. Windows
linking, DLL loading, ConPTY execution and desktop rendering remain unverified.

The full ARM64 Linux desktop now passes `cargo check`, including GPUI's default
X11 and Wayland paths. Native dependency compilation uses mise-managed Zig via
the target compiler/archiver wrappers; Rust target triples are translated at that
boundary. The check exposed an upstream xattr 0.2.3/libc compatibility error.
The licensed `vendor/xattr` patch maps missing-attribute errors to ENODATA on
Linux/Android while preserving native ENOATTR on macOS/BSD. The macOS workspace
also checks successfully afterward (old xattr deprecation warnings remain).
`mise run check-linux-desktop` records the build setup. Linking, shared-library
resolution, native display/IME behavior and runtime xattr behavior remain open.
Upstream xattr tests were not run: Cargo rejects dependency tests with development
dependencies outside the workspace; the crate was not added to product workspace
members solely to bypass that restriction.
