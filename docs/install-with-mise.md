# Install with mise

rmux releases are currently GitHub prereleases. Mise excludes them from `latest`
unless prereleases are enabled, which causes the “no versions found” error in
[issue #2](https://github.com/rmuxapp/rmux/issues/2).

For macOS Apple Silicon, add this to your mise configuration:

```toml
[tools."github:rmuxapp/rmux"]
version = "latest"
prerelease = true
asset_pattern = "rmux-*-macos-arm64.zip"
strip_components = 0
bin_path = "rmux.app/Contents/MacOS"
```

Run `mise install`, then `mise exec -- rmux` or `mise exec -- rmuxctl --help`.
Preserving the `.app` directory keeps its resources and framework paths together.
For Finder installation and Sparkle-managed updates, prefer the DMG instead;
use mise to manage versions of a mise installation.

On Linux x86-64, use the same version/prerelease settings with
`asset_pattern = "rmux-*-linux-x86_64.tar.gz"`. On Windows x86-64 use
`asset_pattern = "rmux-*-windows-x86_64.zip"`. Omit `strip_components` and
`bin_path` on those platforms so mise detects the archive's top-level directory.

Mise may also hide newly published releases under a configured minimum release
age. Wait for that interval to pass; this is separate from prerelease filtering.

The macOS archive was installed successfully in isolated mise directories,
including the preserved .app layout, and rmuxctl executed successfully.
Native Windows/Linux mise execution and macOS GUI launch acceptance are pending.
See the [mise GitHub backend documentation](https://mise.jdx.dev/dev-tools/backends/github.html).
