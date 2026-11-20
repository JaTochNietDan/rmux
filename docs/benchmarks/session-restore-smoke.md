# Native multi-window session restore

## Group discovery and style extension

Recorded 2026-11-20 with `mise run smoke-session-restore`, exit 0.
Executable SHA-256:
`31ec15fb8862d1d130825f9ab34c6783dd8f3820098c4bfba373bc1f7dc6c61c`.
Window/group discovery and workspace membership agree on live anchor/member IDs
after explicit restore and forced-stop relaunch. The fixture also retains custom
RGB color and a folder emoji through automatic capture and final Quit. The
fixture remains anchor-only; native multi-member operations and visuals are not
covered by these assertions.

## Workspace group persistence extension

Recorded 2026-11-20 with `mise run smoke-session-restore`, exit 0.
Executable SHA-256:
`cc63577e401643b51f80777cdd740891b2178068d0acf60f06d366dc96e7b737`.
An anchor-only group with a Unicode name, stable ID 17, collapsed/pinned state
and explicit non-generated provenance survives automatic capture, forced-stop
relaunch and final Quit checkpoint unchanged. Existing explicit restore,
preflight, zoom, metadata and child cleanup checks also pass. This does not
verify native group creation, cwd inheritance, multi-member collapse behavior,
drag/drop or visual layout.

## Pane zoom persistence extension

Recorded 2026-11-20 with `mise run smoke-session-restore`, exit 0.
Executable SHA-256:
`d9bafacdc0369e022f3b1d81a5a52e5accde0187e96c44bba25e8d9fc555595a`.
The second workspace starts with its second pane zoomed. Explicit restore,
automatic capture and forced-stop/relaunch retain that zoom target. Unzooming
through RPC restores the identical pane IDs/tree and 1:3 weights. The focused
format test covers backward defaults and invalid indices. Visual focus is pending.

## Failed recovery preservation extension

Recorded 2026-11-20. Desktop build and extended native smoke passed via mise.
Executable SHA-256:
`97809265d0adaf9c75f192e1bf96fe98ef5823ea6abdbdb033aad388a591c94e`.
Malformed JSON and unsupported versions each produced one fresh window and a
logged recovery error. Normal Quit exited 0 and left the original bytes intact.
The fresh-window notice is implemented but not visually inspected. Existing
automatic restore, ownership, directory, geometry and forced-stop cases passed.

## Window geometry extension

Recorded 2026-11-20 using `mise run smoke-session-restore`, exit 0.
Executable SHA-256:
`d4335e729f04d61bb1aa275d5edd353989d9aabeceff694a6e1243667d30c917`.
Captured checkpoints contain positive window dimensions and windowed mode for
both windows; forced-stop/relaunch and existing assertions passed. Focused unit
tests cover bounds clamping, negative display coordinates, optional old-file
geometry and validation/mode round trips. Exact position, native resize events,
display reconnection and fullscreen/maximized behavior are not visually verified.

## Exclusive recovery ownership extension

Recorded 2026-11-20. Recovery unit tests and the extended native smoke passed.
Executable SHA-256:
`e979b2f4f2a37ca521e9dfd20634ccea6755c1ea1d590a70cb0c92901837ad75`.
A second native process uses the same session_file, reports recovery disabled,
opens only one fresh window, and quits without replacing the owner's checkpoint.
The owner then undergoes the existing forced-stop/relaunch scenario successfully,
demonstrating lock release on process death. All tracked fixture shells exit.
This establishes local macOS behavior, not network-filesystem or Windows locking.

## Forced-stop and relaunch extension

Recorded 2026-11-20 against the cached-directory executable below
(`c93e556c1e36cbb0a82b93261935c5c6264279fe6b618ab40987e4319ba933a7`).
`mise exec -- python3 scripts/smoke-session-restore.py` exited 0.
After a completed automatic checkpoint, the harness sends SIGKILL to its own
test app, checks that file bytes are unchanged, and relaunches with the same
settings. Two windows, names, weighted split and four tab labels return; all four
shell PIDs are fresh. The reported Unicode directory survives the next normal
quit/save. Both generations of fixture shells are gone after cleanup. This tests
recovery from a completed checkpoint, not power failure or interruption during
atomic replacement, and does not resume old processes or scrollback.

## Cached directory capture extension

Recorded 2026-11-20. Desktop build and extended smoke passed via mise.
Executable SHA-256:
`c93e556c1e36cbb0a82b93261935c5c6264279fe6b618ab40987e4319ba933a7`.
The automatic checkpoint preserves all four explicitly requested startup
directories before any shell reports OSC 7. One fixture then reports a local
path containing spaces and CJK characters; that path appears in a subsequent
checkpoint before Quit. Both startup modes and cleanup passed. Snapshot capture
now reads the UI metadata cache without taking each terminal's parser mutex.
This is not a throughput or frame-latency measurement.

## Change-triggered checkpoint extension

Recorded 2026-11-20. Built with mise and ran
`mise exec -- python3 scripts/smoke-session-restore.py`, exit 0.
Executable SHA-256:
`e94bec2dc3e0ff20a6314b25dd60829808a18f73e359ba8557186fcb1e16b83b`.
The automatic case renames a live workspace to `Checkpoint 世界` and waits up to
four seconds for the session file to contain that name before requesting Quit.
The update appeared, the final checkpoint retained it, and shell cleanup passed.
The explicit restore case also passed. Forced-crash/relaunch and high-frequency
change/capture performance remain unmeasured.

## Automatic recovery and quit checkpoint extension

Recorded 2026-11-20 with `mise run smoke-session-restore`, exit 0.
Executable SHA-256:
`bb86eb7291b99206a53d8d4f7094dff46dc579b36337eebb618da5cb5c10ab68`.
Both explicit and configured automatic startup cases passed. The automatic case
uses an isolated session_file setting and confirm_close:never, requests normal
quit through a system.quit notification, waits for exit 0, and reads the replaced
checkpoint. Both workspace names and sidebar widths survive, and captured tab
settings demonstrate that the original fixture was actually replaced. All tracked
shells exit. This does not cover confirmation dialogs, OS-originated quit,
corrupt-file fallback or crash checkpoints.

Recorded 2026-11-20 on macOS ARM64 with `mise run smoke-session-restore`.
After adding the later-window preflight case, reran the unchanged binary using
`mise exec -- python3 scripts/smoke-session-restore.py`; exit 0.

Executable SHA-256:
`307a36384d99cfa9b60879fab1635ae6d0534e411f718d6ca9e3321489b5f2f3`.

The isolated fixture verified:

- `--check-session` validates without starting fixture shells.
- An invalid directory in the second window returns exit 2 before either window
  starts shells.
- Two windows restore with distinct Unicode workspace names, pinned state,
  descriptions, icons and colors.
- A vertical split retains its 1:3 weights; four tab labels and the selected
  second tab in a multi-tab pane are restored.
- Four distinct live shell PIDs are discoverable, and all exit after the test
  application terminates.

No user configuration is changed. This is structural/runtime evidence, not
visual layout, OS window activation, pane-focus, latency, partial-spawn rollback
or automatic checkpoint acceptance.
