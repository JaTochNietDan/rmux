# Experimental browser

[← User guide](../user-guide.md)

## On this page

- [Find in a page](#find-in-a-page)
- [Web Inspector](#web-inspector)
- [Stop loading](#stop-loading)

## Find in a page

In a browser-enabled macOS build (`mise run browser`), use **Browser → Find in
Page** or Cmd+F. Type a phrase and press Enter for the next match, or Shift+Enter
for the previous one. Cmd+G/Cmd+Shift+G and the arrow buttons also navigate matches;
**Aa** toggles case sensitivity. Searches wrap and report **Match found** or
**No matches**. Escape in the Find field closes it and returns focus to the page.
The search runs when submitted, rather than on each keystroke. Native shortcut,
focus and highlight behavior still need visual acceptance. Browser windows remain
experimental and separate from workspace panes.

## Web Inspector

In experimental browser builds, **Browser → Toggle Web Inspector**, Cmd+Option+I,
or the **{}** toolbar button opens/closes WebKit's developer tools for that browser
window. Open a page first. Native inspector interaction still needs acceptance;
console output is not yet exposed through rmux's CLI/RPC.

## Stop loading

While a browser page loads, its Reload button becomes **Stop**. Click it or use
**Browser → Stop Loading** / Cmd+. to cancel. The page can be reloaded afterward;
Escape remains available for canceling address edits and closing Find. Native
slow-network/cancellation behavior remains pending acceptance.
