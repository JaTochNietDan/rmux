# P1 native UI smoke check

Recorded 2026-11-20 04:02 UTC on the local Apple M4 Max/macOS 27.2 machine.
Built with `mise run bundle` (debug profile). Bundle executable SHA-256:
`4d3fce6b1f669a819d169a862e15f6ddae86f3408466a2ad0278722a63481451`.

This was a short interaction check, not a performance run or a full cmux parity
comparison. The existing Metal HUD was visible; its debug-process numbers are not
release measurements. The upstream inventory baseline remains cmux commit
`a76816e4f0788003758b9434ab702068a640fb5d`; no side-by-side comparison was performed.

Evidence: native app screenshots and action results in this Codex chat.

| Scenario | Expected | Observed |
| --- | --- | --- |
| Run `printf 'alpha beta beta\n'`, open Cmd+F, enter `beta`, press Enter | Query stays in find field; count includes echoed command and output; selected match highlights | Field showed `beta`, count `1 / 4`, output match highlighted; query did not appear at shell prompt |
| Escape, Cmd+T, Cmd+D, Cmd+Shift+D | Find closes; second tab and nested right/below splits appear | Two tabs in left pane and two vertically split panes on right |
| Drag vertical divider left | Adjacent split proportions change without replacing contents | Divider moved from screenshot x≈1199 to x≈980; right nested layout retained |
| Click original left tab | Prior contents retained | Original command, output and selected match returned |
| Focus bottom-right terminal and Cmd+W | Last tab closes its pane; remaining right pane fills branch | Right split collapsed to one full-height pane; left contents remained |
| Cmd+Q | App exits | `pgrep -fl '/rmux.app/Contents/MacOS/rmux'` returned no matching process |

The AX tree exposed only the native window controls, not the custom terminal/tab/
find controls. Accessibility work remains outstanding. Unicode/IME search input,
selection clipboard operations, edge autoscroll, double-click divider equalization,
tab reordering, workspace switching, native Linux/Windows behavior and performance
were not checked in this run. Process identity continuity was not measured; the
retained-tab check establishes visible content retention only.
