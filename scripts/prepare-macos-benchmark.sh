#!/bin/sh
# Run after mise run bundle-release. Creates a separate, disposable local bundle.
set -eu
cd "$(dirname "$0")/.."
panes="${1:-8}"
case "$panes" in 1|4|8|16) ;; *) echo 'use 1, 4, 8 or 16 panes' >&2; exit 1 ;; esac
bundle="$PWD/target/release/rmux-bench-$panes.app"
mkdir -p "$bundle/Contents/MacOS"
cp target/release/rmux.app/Contents/MacOS/rmux "$bundle/Contents/MacOS/rmux"
cp target/release/rmux.app/Contents/Info.plist "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier dev.rmux.benchmark.p$panes" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :LSEnvironment dict' "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:RMUX_BENCH_PANES string $panes" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Add :LSEnvironment:MTL_HUD_ENABLED string 0' "$bundle/Contents/Info.plist"
if [ -n "${RMUX_RENDER_METRICS:-}" ]; then
    /usr/bin/plutil -insert LSEnvironment.RMUX_RENDER_METRICS -string "$RMUX_RENDER_METRICS" "$bundle/Contents/Info.plist"
fi
# The benchmark identifier and launch environment differ from the source bundle.
# Seal them only after all plist edits are complete.
codesign --force --sign - "$bundle"
codesign --verify "$bundle"
printf '%s\n' "$bundle"
