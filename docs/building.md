# Building rmux

All development tool versions are pinned in `mise.toml`: Rust 1.99.1 and Zig 0.17.0. Run development commands through mise. Never use Homebrew; installations outside mise require explicit approval. Setup scripts download project-local library dependencies, not standalone tool installers. They do not install platform SDKs or system packages.

## macOS

### Linux terminal cross-check from macOS

The project Rust pin includes the `x86_64-unknown-linux-gnu` target. `mise install
rust` installs missing targets through mise's Rust backend; do not run a separate
rustup installation. See [mise target configuration](https://mise.jdx.dev/lang/rust.html#targets).

After building the cross-target Ghostty library with
`RMUX_GHOSTTY_TARGET=x86_64-linux-gnu mise exec -- sh scripts/build-ghostty-source.sh`,
run `mise run check-linux-terminal` and `mise run build-linux-tests`. The latter
uses the pinned Zig through `scripts/zig-linux-linker.sh`. Both underlying
commands passed on the macOS development host. This checks and links the terminal
crate, not GPUI or the desktop application. Execute the resulting test binary in
a compatible Linux environment to validate behavior.

The current executable requires `/lib64/ld-linux-x86-64.so.2`. The available ARM64
container image lacks that loader, so execution was not possible there. A static
glibc experiment was rejected by Zig; it is not part of the saved tasks. No
container tools or multi-architecture libraries were installed.

ARM64 Linux tests can be cross-linked with `mise run build-linux-arm64-tests`
after building Ghostty with `RMUX_GHOSTTY_TARGET=aarch64-linux-gnu`. Zig 0.17
currently rejects Rust's `--fix-cortex-a53-843419` linker flag. For tests on the
known Apple Silicon VM only, `RMUX_LOCAL_NON_A53_TEST=1` explicitly omits that
flag. Never use that opt-in to produce portable ARM64 release artifacts. The
[Linux execution record](benchmarks/linux-arm64-terminal-tests.txt) documents the
13 passing terminal tests and exact container/runtime limitations.

### Native build

With the existing Xcode/Metal toolchain available:

```sh
mise install
mise run bootstrap
mise run run
mise run bundle-release
```

The bootstrap verifies the pinned official Ghostty XCFramework checksum. The release bundle is `target/release/rmux.app`. Packaging applies a local ad-hoc code signature after assembling the executable, plist and resources; this is not Developer ID signing or notarization. The benchmark-bundle script signs again after changing its identifier and launch environment. The Apple Metal Toolchain installation was specifically approved on the current development machine; that approval does not authorize installation on another machine.

## Linux

`mise run check-linux-desktop` now cross-checks the ARM64 Linux desktop from
macOS using the project's Zig C/C++ compiler and archiver wrappers. Build the
ARM64 Ghostty dependency first as described above. GPUI's default X11 and Wayland
code passes this check. This does not link or launch the GUI, validate target
shared libraries, or exercise a display server.

GPUI's dependency chain currently requires `xattr` 0.2.3. A small licensed local
patch in `vendor/xattr` replaces its Linux/Android `ENOATTR` reference with
`ENODATA`, fixing compilation against the current libc crate. The patch rationale
and removal condition are in `vendor/xattr/RMUX-PATCH.md`.

```sh
mise install
mise run bootstrap
mise run check
mise run test
mise run run
```

Bootstrap verifies the official libghostty-vt 1.0.1 source archive and uses mise's Zig to build into `.deps/ghostty-vt`. It preserves SIMD support and uses ReleaseFast. Rust links the static archive from that directory. The source build can download Ghostty's hash-pinned Zig package dependencies.

The desktop build also requires the native compiler/linker, libclang for bindgen, and GPUI's Linux development libraries. The default GPUI build includes Wayland and X11; a working graphics driver and display session are required for runtime testing. These prerequisites are not provisioned by the bootstrap. Resolve their exact package names and approved installation mechanism on the target distribution. A library cross-build alone does not validate those prerequisites or the desktop app.

The initial Linux font default is DejaVu Sans Mono; the target must provide it for matching cell appearance. Configurable font metrics/fallback remain P0 work.

## Windows

`mise run check-windows-terminal` cross-checks the terminal crate and its test
code for `x86_64-pc-windows-msvc`. Mise manages that Rust target through the project
pin. The task expects the checksum-verified official package extracted at
`.deps/ghostty-vt-1.0.1-x86_64-windows-gnu`; the Windows bootstrap extracts that
package before copying its contents to the native dependency directory. This
check passed from macOS and caught enum signedness assumptions in the FFI wrapper.
It does not link an executable, load the DLL, run ConPTY, or validate GPUI.

From a native developer environment with the existing MSVC linker/Windows SDK and libclang available:

```powershell
mise install
mise run bootstrap-windows
mise run check
mise run test
mise run run
mise run bundle-windows
```

Bootstrap selects x86_64 or aarch64 from the Rust host triple, verifies its pinned package checksum, and copies headers/import library/DLL to `.deps/ghostty-vt`. `RMUX_GHOSTTY_ARCH` can explicitly override package architecture for cross-build preparation. Rust links `ghostty-vt.lib` dynamically; mise adds `.deps/ghostty-vt/bin` to the process PATH so development binaries/tests can find the DLL. The release bundle task copies the executable, DLL and available library symbols to `target/release/rmux-windows`.

The SDK and libclang are prerequisites, not silently installed by the scripts. PowerShell execution policy is not changed. The initial Windows font is Consolas. Shortcuts use Ctrl+Shift+D/V/Q for split/paste/quit on Linux and Windows; macOS keeps Command+D/V/Q.

Ghostty's Windows package uses the GNU ABI but supplies an import library compatible with MSVC. Its static archive is **not** compatible with MSVC's linker, which is why rmux uses the DLL on Windows. Upstream tests its x86_64 package at release; its arm64 package is built but not runtime-tested. [Official installation guide](https://lib.ghostty.org/vt/installing.html#installing-windows)

## Library cross-builds and overrides

To build a Linux library on macOS without replacing the native library:

```sh
RMUX_GHOSTTY_TARGET=x86_64-linux-gnu mise exec -- sh scripts/build-ghostty-source.sh
```

Supported source-build targets are x86_64/aarch64 Linux GNU and Windows GNU. Output goes into `.deps/ghostty-vt-TARGET`. This checks the Ghostty library build, not Rust cross-linking or GUI runtime behavior. For a custom native library installation, set `GHOSTTY_VT_DIR` to a directory containing `include/` and `lib/`; Windows also needs its `bin/` DLL directory on PATH or beside the executable.

Source and archive layout follow the [official Ghostty installation documentation](https://lib.ghostty.org/vt/installing.html). Checksums were pinned from the versioned [release checksum manifest](https://release.files.ghostty.org/libghostty-vt/1.0.1/SHA256SUMS). The scripts verify SHA-256; they do not claim a separate minisign verification.

## Validation status

macOS workspace builds, terminal tests and native smoke checks have run. Windows archive checksum and layout were inspected on macOS, but the PowerShell scripts and application have not run on Windows. Full Linux/Windows runtime checks, IME, DPI, output pressure and teardown are still required before P0 can pass.

The x86_64 Linux GNU source cross-build completed successfully on macOS using the pinned mise Zig 0.17.0. It produced `libghostty-vt.a`, an x86-64 ELF shared library, headers and pkg-config metadata. This is library compilation evidence only. The session tests now have Unix and Windows command branches, so a future native Windows test run will exercise ConPTY output, interactive resize/input and exit status rather than skipping the entire session module.

### Vendored macOS notification response cleanup

`vendor/mac-usernotifications` pins 0.3.1 with a response-guard lifetime fix.
See its `RMUX-PATCHES.md` for provenance. Focused tests do not contact the OS:

```sh
mise exec -- cargo test --manifest-path vendor/mac-usernotifications/Cargo.toml --target-dir target --lib pending_response
```

The standalone dependency test package is excluded from workspace tests so Linux
and Windows workspace commands do not attempt to compile Apple frameworks.
