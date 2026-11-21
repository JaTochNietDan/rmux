# rmux mark

The violet panes and inset terminal prompt form the rmux mark. `rmux.svg` is
the vector master. `rmux.icns` is the macOS bundle asset, generated at standard
16–1024 pixel resolutions with transparency.

To regenerate on macOS using the existing Apple SDK:

```sh
mise exec -- swift scripts/make-app-icon.swift target/branding/rmux.iconset
mise exec -- iconutil -c icns target/branding/rmux.iconset -o docs/brand/rmux.icns
```

Keep the renderer's geometry and colors in sync with the SVG. The generated
ICNS is committed so ordinary builds only copy it into the bundle. Packaging
sets `CFBundleIconFile` before code signing. The website keeps a local copy of
the SVG in its separate repository.
