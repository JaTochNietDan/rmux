#!/bin/bash
# Invoke through mise so this uses the project's pinned Zig.
set -eu
target="${RMUX_ZIG_TARGET:-x86_64-linux-gnu}"
case "$target" in
    x86_64-linux-gnu|aarch64-linux-gnu) ;;
    *) echo 'Unsupported Linux linker target' >&2; exit 1 ;;
esac
# Zig 0.17 rejects this Rust-supplied Cortex-A53 workaround flag. Never drop it
# for portable artifacts. The opt-in is only for tests on a known non-A53 host.
args=()
for arg in "$@"; do
    # cc-rs supplies Rust triples, whereas Zig omits the vendor component.
    if { [ "$target" = aarch64-linux-gnu ] && [ "$arg" = '--target=aarch64-unknown-linux-gnu' ]; } ||
       { [ "$target" = x86_64-linux-gnu ] && [ "$arg" = '--target=x86_64-unknown-linux-gnu' ]; }; then
        continue
    fi
    if [ "$target" = aarch64-linux-gnu ] && [ "${RMUX_LOCAL_NON_A53_TEST:-}" = 1 ] &&
       [ "$arg" = '-Wl,--fix-cortex-a53-843419' ]; then
        continue
    fi
    args+=("$arg")
done
exec zig cc -target "$target" "${args[@]}"
