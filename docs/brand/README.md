# rmux mark

The violet panes and inset terminal prompt form the rmux mark. `rmux.svg` is
the vector master. `rmux.icns` is the macOS bundle asset, generated at standard
16–1024 pixel resolutions with transparency.

To regenerate on macOS using the existing Apple SDK:

```sh
mise exec -- swift scripts/make-app-icon.swift target/branding/rmux.iconset
mise exec -- iconutil -c icns target/branding/rmux.iconset -o docs/brand/rmux.icns
mise exec -- python scripts/make-windows-icon.py
```

Keep the renderer's geometry and colors in sync with the SVG. The generated
ICNS is committed so ordinary builds only copy it into the bundle. Packaging
sets `CFBundleIconFile` before code signing. The website keeps a local copy of
the SVG in its separate repository.

Windows embeds `rmux.ico` as resource 1, the icon GPUI loads for its windows.
Linux packages include the SVG and a matching `rmux.desktop` launcher; native
windows use `rmux` as their application ID for desktop grouping.
