#!/bin/sh
# Downloads a project-local library dependency, not a globally installed tool.
set -eu
cd "$(dirname "$0")/.."
version=1.0.1
case "$(uname -s)" in
  Linux) exec sh scripts/build-ghostty-source.sh ;;
  Darwin) ;;
  *) printf '%s\n' 'On Windows run mise run bootstrap-windows.'; exit 1 ;;
esac
archive=".deps/ghostty-vt-$version.xcframework.zip"
expected=c0f9631277f726154c8f521954d17cdc41aac4626fc0df8cf925a19ee58e829b
mkdir -p .deps
if [ ! -f "$archive" ]; then
  curl --fail --location --retry 2 "https://release.files.ghostty.org/libghostty-vt/$version/ghostty-vt-$version.xcframework.zip" -o "$archive.part"
  mv "$archive.part" "$archive"
fi
printf '%s  %s\n' "$expected" "$archive" | shasum -a 256 -c -
if [ ! -d .deps/ghostty-vt.xcframework ]; then unzip -q "$archive" -d .deps; fi
printf '%s\n' 'Ghostty VT 1.0.1 ready in .deps/ghostty-vt.xcframework'
