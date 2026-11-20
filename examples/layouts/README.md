# Reusable terminal layouts

These version-1 layouts open fresh shells. Pane names describe their intended
use; they do not start programs or replay terminal commands.

| File | Arrangement | Suggested use |
| --- | --- | --- |
| [development.json](development.json) | Wide Editor pane on the left; Run above Output on the right | Edit code, run a build, and inspect output independently |
| [services.json](services.json) | Equal API, Worker, and Logs columns | Keep service commands and logs visible together |

Copy a file to your own layouts folder before customizing it. Set each tab's
`directory` to an existing absolute project path for repeatable startup. The
provided `null` value requests the normal platform/shell starting directory;
it does not infer a project or the directory of the JSON file. Shell startup
configuration still runs normally. Different panes may use different directories.

Use **File → Open Layout in New Window…** and select the file. Existing windows
remain open. Each invocation starts a separate set of shells. Split sizes are
relative weights, so the arrangement adapts to the window size. No window position,
font override, or machine-specific directory is embedded in these examples.

To make a layout available in the command palette, add this to your settings,
replacing the example path with the absolute path of your copy:

```json
{
  "layout_presets": [
    {"name":"Development", "path":"/absolute/path/development.json"}
  ],
  "keybindings": [
    {"key":"ctrl-k d", "preset":"Development"}
  ]
}
```

Use **Reload Settings**, then **Open Preset: Development** in the palette, or
press the configured chord. On Windows, use an absolute Windows path with JSON
backslash escaping, such as `C:\\Users\\you\\Layouts\\development.json`.
Preset files are read on invocation, so your next use picks up file edits.
Missing directories or invalid layouts produce an import error.

From a source checkout, validate the examples without opening a window:

```sh
mise exec -- cargo build -p rmux-desktop
mise exec -- target/debug/rmux --check-layout examples/layouts/development.json
mise exec -- target/debug/rmux --check-layout examples/layouts/services.json
```

The validator checks layout structure, not directory existence, shell startup,
or visual appearance. The native restore path checks directories before spawning.
After rearranging the app, use **Save Layout…** to capture your new setup.
