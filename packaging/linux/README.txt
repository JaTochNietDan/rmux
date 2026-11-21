Linux desktop launcher (optional)

You can always run ./rmux from this extracted folder.

For an application-menu entry, keep this entire folder in a permanent location
and make rmux available on your graphical session's PATH. Then run these commands
from the extracted folder:

  mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
  cp rmux.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/rmux.desktop"
  cp rmux.svg "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/rmux.svg"

Your desktop may need a new login to refresh its launcher. The launcher is hidden
when rmux is not on PATH. This does not install or update the executable.

To remove this launcher, delete only the two copied rmux.desktop and rmux.svg
files above. Keep the extracted shell-integration directory beside the binary.
