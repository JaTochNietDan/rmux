//! Explicit supported command subset; reject unsupported flags rather than dropping them.
use serde_json::{Map, Value};
use std::ffi::OsString;

pub const HELP: &str = "Commands (JSON output; explicit opaque IDs required):
  notify --surface ID [--title TITLE] [--body BODY]
  list-notifications [--window ID] [--workspace ID]
  dismiss-notification --surface ID --revision NUMBER
  close-workspace --workspace ID
  new-window [--activate]
  new-workspace --window ID [--name NAME] [--description TEXT] [--cwd PATH]
                [--focus true|false] [--activate]
  list-windows | list-workspaces | list-panes | list-panels
  select-workspace --workspace ID [--activate]
  rename-workspace --workspace ID NAME
  focus-panel --panel ID [--activate]
  move-panel --panel ID --workspace ID [--activate]
  move-workspace --workspace ID --window ID [--activate]
  split-pane --pane ID --direction right|down [--cwd PATH] [--focus true|false] [--activate]
  set-pane-zoom --pane ID --zoomed true|false
  send-key --surface ID KEY  (Enter, Up, Ctrl-c, Alt-Left, Shift-Tab)
  send --surface ID [--paste] TEXT
  read-screen --surface ID [--scrollback | --selection]
Use -- before positional text beginning with a dash. List commands list the
whole process unless narrowed with --window (workspaces/panes/panels) or
--workspace (panes/panels). No implicit target, ref/index lookup or --force yet.
";

pub fn help(command: &str) -> Result<String, String> {
    let (arguments, description) = match command {
        "dismiss-notification" => ("--surface ID --revision NUMBER", "Dismiss one retained message using its surface ID and revision from list-notifications. A missing or already dismissed message returns dismissed:false."),
        "notify" => ("--surface ID [--title TITLE] [--body BODY]", "Add a notification to a live terminal's history."),
        "list-notifications" => ("[--window ID] [--workspace ID]", "List retained notifications, newest first within each terminal."),
        "close-workspace" => ("--workspace ID", "Close a workspace through the app's close policy. No force option is supported."),
        "new-window" => ("[--activate]", "Create a window. Activating it requires --activate."),
        "new-workspace" => ("--window ID [--name NAME] [--description TEXT] [--cwd PATH] [--focus true|false] [--activate]", "Create a workspace in an existing window. Selection and activation are opt-in."),
        "list-windows" => ("", "List windows in the connected app process."),
        "list-workspaces" => ("[--window ID]", "List workspaces, optionally limited to one window."),
        "list-panes" | "list-panels" => ("[--window ID] [--workspace ID]", "List panes or terminal surfaces, optionally limited to a window or workspace."),
        "select-workspace" => ("--workspace ID [--activate]", "Select a workspace; --activate also activates its window."),
        "rename-workspace" => ("--workspace ID NAME", "Set the workspace's manual name. Quote names containing spaces."),
        "focus-panel" => ("--panel ID [--activate]", "Focus an existing terminal surface; --activate also activates its window."),
        "send-key" => ("--surface ID KEY", "Send one key tap, such as Enter, Up, Ctrl-c, Alt-Left or Shift-Tab."),
        "send" => ("--surface ID [--paste] TEXT", "Send literal text. --paste applies terminal paste encoding. Use -- before text starting with a dash."),
        "read-screen" => ("--surface ID [--scrollback | --selection]", "Read visible text by default, retained history with --scrollback, or selected text with --selection."),
        "move-panel" => ("--panel ID --workspace ID [--activate]", "Move a live terminal to another workspace, preserving its process. Destination selection changes; window activation requires --activate."),
        "move-workspace" => ("--workspace ID --window ID [--activate]", "Move a workspace and its live terminals to another window. Window activation requires --activate."),
        "split-pane" => ("--pane ID --direction right|down [--cwd PATH] [--focus true|false] [--activate]", "Create a terminal beside an explicit pane. Inherits its directory and font; focus and OS activation are opt-in."),
        "set-pane-zoom" => ("--pane ID --zoomed true|false", "Set pane zoom explicitly without toggling. Zooming selects its terminal within the window, without OS window activation."),
        "completions" => ("bash|zsh|fish", "Print an offline shell completion script. No app connection is made."),
        _ => return Err(format!("no local help for {command}; use --help for supported commands or system.capabilities to discover RPC methods")),
    };
    let prefix = if command == "completions" { "rmuxctl" } else { "rmuxctl [--socket PATH]" };
    Ok(format!("Usage: {prefix} {command} {arguments}\n\n{description}\n\nTargets use explicit opaque IDs from list commands. RPC commands print JSON.\nExit 0: success; 1: RPC error; 2: usage/transport failure.\n"))
}

pub fn expand(command: &str, args: Vec<OsString>) -> Result<(String, Value), String> {
    let (method, target, positional) = match command {
        "move-panel" => ("surface.move_to_workspace", Some(("--panel", "surface_id")), None),
        "move-workspace" => ("workspace.move_to_window", Some(("--workspace", "workspace_id")), None),
        "split-pane" => ("pane.split", Some(("--pane", "pane_id")), None),
        "set-pane-zoom" => ("pane.zoom.set", Some(("--pane", "pane_id")), None),
        "list-notifications" => ("notification.list", None, None),
        "dismiss-notification" => ("notification.dismiss", Some(("--surface", "surface_id")), None),
        "notify" => ("notification.send", Some(("--surface", "surface_id")), None),
        "close-workspace" => (
            "workspace.close",
            Some(("--workspace", "workspace_id")),
            None,
        ),
        "new-window" => ("window.create", None, None),
        "new-workspace" => ("workspace.create", Some(("--window", "window_id")), None),
        "list-windows" => ("window.list", None, None),
        "list-workspaces" => ("workspace.list", None, None),
        "list-panes" => ("pane.list", None, None),
        "list-panels" => ("surface.list", None, None),
        "select-workspace" => (
            "workspace.select",
            Some(("--workspace", "workspace_id")),
            None,
        ),
        "rename-workspace" => (
            "workspace.rename",
            Some(("--workspace", "workspace_id")),
            Some("name"),
        ),
        "focus-panel" => ("surface.focus", Some(("--panel", "surface_id")), None),
        "send-key" => (
            "surface.send_key",
            Some(("--surface", "surface_id")),
            Some("key"),
        ),
        "send" => (
            "surface.send_text",
            Some(("--surface", "surface_id")),
            Some("text"),
        ),
        "read-screen" => ("surface.read_text", Some(("--surface", "surface_id")), None),
        _ => {
            if args.len() > 1 {
                return Err("quote the JSON object as one argument".into());
            }
            let params = match args.first() {
                Some(value) => serde_json::from_str(value.to_str().ok_or("JSON must be UTF-8")?)
                    .map_err(|e| e.to_string())?,
                None => Value::Object(Map::new()),
            };
            if !params.is_object() {
                return Err("parameters must be a JSON object".into());
            }
            return Ok((command.into(), params));
        }
    };
    let mut params = Map::new();
    let mut literal = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg
            .into_string()
            .map_err(|_| "command arguments must be UTF-8")?;
        if !literal && arg == "--" {
            literal = true;
            continue;
        }
        if !literal && arg.starts_with('-') {
            if command == "dismiss-notification" && arg == "--revision" {
                let raw = args.next().ok_or("--revision requires an unsigned integer")?.into_string().map_err(|_| "revision must be UTF-8")?;
                if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) { return Err("--revision expects an unsigned integer".into()); }
                let revision: u64 = raw.parse().map_err(|_| "revision exceeds unsigned 64-bit range")?;
                if params.insert("revision".into(), Value::from(revision)).is_some() { return Err("duplicate --revision".into()); }
                continue;
            }
            let destination = match (command, arg.as_str()) {
                ("move-panel", "--workspace") => Some("workspace_id"),
                ("move-workspace", "--window") => Some("window_id"),
                _ => None,
            };
            if let Some(key) = destination {
                let value = args.next().ok_or("destination requires an ID")?.into_string().map_err(|_| "ID must be UTF-8")?;
                if value.is_empty() || value.len() > 128 || value.starts_with('-') { return Err("invalid destination ID".into()); }
                if params.insert(key.into(), Value::String(value)).is_some() { return Err("duplicate destination".into()); }
                continue;
            }
            if command == "split-pane" && arg == "--direction" {
                let value = args.next().ok_or("--direction requires right or down")?.into_string().map_err(|_| "direction must be UTF-8")?;
                if !matches!(value.as_str(), "right" | "down") { return Err("--direction expects right or down".into()); }
                if params.insert("direction".into(), Value::String(value)).is_some() { return Err("duplicate --direction".into()); }
                continue;
            }
            if command == "set-pane-zoom" && arg == "--zoomed" {
                let value = match args.next().as_deref().and_then(|value| value.to_str()) {
                    Some("true") => true,
                    Some("false") => false,
                    _ => return Err("--zoomed expects true or false".into()),
                };
                if params.insert("zoomed".into(), Value::Bool(value)).is_some() { return Err("duplicate --zoomed".into()); }
                continue;
            }
            if command == "notify" && matches!(arg.as_str(), "--title" | "--body") {
                let value = args
                    .next()
                    .ok_or("notification option requires text")?
                    .into_string()
                    .map_err(|_| "text must be UTF-8")?;
                if params
                    .insert(arg.trim_start_matches("--").into(), Value::String(value))
                    .is_some()
                {
                    return Err("duplicate notification option".into());
                }
                continue;
            }
            if (command == "new-workspace" && matches!(arg.as_str(), "--name" | "--description" | "--cwd" | "--focus"))
                || (command == "split-pane" && matches!(arg.as_str(), "--cwd" | "--focus"))
            {
                let value = args
                    .next()
                    .ok_or("option requires a value")?
                    .into_string()
                    .map_err(|_| "value must be UTF-8")?;
                let key = arg.trim_start_matches("--");
                let value = if key == "focus" {
                    Value::Bool(match value.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return Err("--focus expects true or false".into()),
                    })
                } else {
                    Value::String(value)
                };
                if params.insert(key.into(), value).is_some() {
                    return Err("duplicate option".into());
                }
                continue;
            }
            if matches!(
                command,
                "list-workspaces" | "list-panes" | "list-panels" | "list-notifications"
            ) && (arg == "--window" || (arg == "--workspace" && command != "list-workspaces"))
            {
                let key = if arg == "--window" {
                    "window_id"
                } else {
                    "workspace_id"
                };
                let value = args
                    .next()
                    .ok_or("filter requires an ID")?
                    .into_string()
                    .map_err(|_| "ID must be UTF-8")?;
                if value.is_empty() || value.len() > 128 || value.starts_with('-') {
                    return Err("invalid filter ID".into());
                }
                if params.insert(key.into(), Value::String(value)).is_some() {
                    return Err("duplicate filter".into());
                }
                continue;
            }
            if let Some((flag, key)) = target {
                if arg == flag {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} requires an ID"))?
                        .into_string()
                        .map_err(|_| "ID must be UTF-8")?;
                    if value.is_empty() || value.len() > 128 || value.starts_with('-') {
                        return Err("invalid target ID".into());
                    }
                    if params.insert(key.into(), Value::String(value)).is_some() {
                        return Err(format!("duplicate {flag}"));
                    }
                    continue;
                }
            }
            let (key, value) = match (command, arg.as_str()) {
                ("send", "--paste") => ("paste", Value::Bool(true)),
                (
                    "select-workspace" | "focus-panel" | "new-window" | "new-workspace" | "move-panel" | "move-workspace" | "split-pane",
                    "--activate",
                ) => ("activate", Value::Bool(true)),
                ("read-screen", "--scrollback") => ("scope", Value::String("history".into())),
                ("read-screen", "--selection") => ("scope", Value::String("selection".into())),
                _ => return Err(format!("unsupported option for {command}: {arg}")),
            };
            if params.insert(key.into(), value).is_some() {
                return Err(format!("duplicate or conflicting option: {arg}"));
            }
        } else {
            let key = positional.ok_or("unexpected positional argument")?;
            if params.insert(key.into(), Value::String(arg)).is_some() {
                return Err("quote text as one argument".into());
            }
        }
    }
    if let Some((flag, key)) = target {
        if !params.contains_key(key) {
            return Err(format!("{command} requires {flag} ID"));
        }
    }
    let required = match command {
        "dismiss-notification" => Some(("revision", "--revision")),
        "split-pane" => Some(("direction", "--direction")),
        "move-panel" => Some(("workspace_id", "--workspace")),
        "move-workspace" => Some(("window_id", "--window")),
        "set-pane-zoom" => Some(("zoomed", "--zoomed")),
        _ => None,
    };
    if let Some((key, flag)) = required {
        if !params.contains_key(key) { return Err(format!("{command} requires {flag}")); }
    }
    if let Some(key) = positional {
        if !params.contains_key(key) {
            return Err(format!("{command} requires {key}"));
        }
    }
    if command == "send-key" {
        let original = params.remove("key").unwrap().as_str().unwrap().to_owned();
        let mut key = original.as_str();
        while let Some((prefix, rest)) = key.split_once('-') {
            let field = match prefix.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "c" => "control",
                "alt" | "meta" | "m" => "alt",
                "shift" | "s" => "shift",
                _ => break,
            };
            if params.insert(field.into(), Value::Bool(true)).is_some() {
                return Err("duplicate key modifier".into());
            }
            key = rest;
        }
        if key.is_empty() {
            return Err("send-key requires a key after modifiers".into());
        }
        let canonical = if key.chars().count() == 1 {
            key.to_owned()
        } else {
            match key.to_ascii_lowercase().as_str() {
                "return" => "enter".into(),
                "esc" => "escape".into(),
                "bspace" => "backspace".into(),
                other => other.into(),
            }
        };
        params.insert("key".into(), Value::String(canonical));
    }
    Ok((method.into(), Value::Object(params)))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }
    #[test]
    fn explicit_targets_and_literal_text() {
        let (method, params) =
            expand("send", args(&["--surface", "42", "--", "--世界\n"])).unwrap();
        assert_eq!(method, "surface.send_text");
        assert_eq!(params["text"], "--世界\n");
        assert_eq!(params["surface_id"], "42");
        assert_eq!(
            expand(
                "send",
                args(&["--surface", "42", "--paste", "multiline\ntext"])
            )
            .unwrap()
            .1["paste"],
            true
        );
        assert_eq!(
            expand("read-screen", args(&["--selection", "--surface", "7"]))
                .unwrap()
                .1["scope"],
            "selection"
        );
        assert!(
            expand("select-workspace", args(&["--workspace", "3"]))
                .unwrap()
                .1
                .get("activate")
                .is_none()
        );
    }
    #[test]
    fn symbolic_key_modifiers_and_aliases() {
        let (method, params) =
            expand("send-key", args(&["--surface", "42", "Ctrl-Shift-Tab"])).unwrap();
        assert_eq!(method, "surface.send_key");
        assert_eq!(params["key"], "tab");
        assert_eq!(params["control"], true);
        assert_eq!(params["shift"], true);
        assert_eq!(
            expand("send-key", args(&["--surface", "42", "Return"]))
                .unwrap()
                .1["key"],
            "enter"
        );
        assert_eq!(
            expand("send-key", args(&["--surface", "42", "Ctrl--"]))
                .unwrap()
                .1["key"],
            "-"
        );
        assert!(expand("send-key", args(&["--surface", "42", "Ctrl-Ctrl-c"])).is_err());
    }
    #[test]
    fn dismissal_preserves_full_revision_precision() {
        assert_eq!(expand("dismiss-notification", args(&["--surface", "42", "--revision", "18446744073709551615"])).unwrap(),
            ("notification.dismiss".into(), serde_json::json!({"surface_id":"42","revision":u64::MAX})));
        for value in ["-1", "+1", "1.0", "1e3", "", "18446744073709551616"] {
            assert!(expand("dismiss-notification", args(&["--surface", "42", "--revision", value])).is_err());
        }
        assert!(expand("dismiss-notification", args(&["--surface", "42"])).is_err());
        assert!(expand("dismiss-notification", args(&["--surface", "42", "--revision", "1", "--revision", "2"])).is_err());
    }

    #[test]
    fn split_command_keeps_target_direction_and_focus_explicit() {
        assert_eq!(expand("split-pane", args(&["--pane", "12", "--direction", "down", "--cwd", "/work/project", "--focus", "false"])).unwrap(),
            ("pane.split".into(), serde_json::json!({"pane_id":"12","direction":"down","cwd":"/work/project","focus":false})));
        for input in [
            vec!["--pane", "12"],
            vec!["--direction", "right"],
            vec!["--pane", "12", "--direction", "left"],
            vec!["--pane", "12", "--direction", "down", "--direction", "right"],
            vec!["--pane", "12", "--direction", "down", "--name", "unsupported"],
        ] { assert!(expand("split-pane", args(&input)).is_err()); }
    }

    #[test]
    fn layout_commands_require_destinations_and_preserve_boolean_types() {
        assert_eq!(expand("move-panel", args(&["--panel", "42", "--workspace", "7"])).unwrap(),
            ("surface.move_to_workspace".into(), serde_json::json!({"surface_id":"42","workspace_id":"7"})));
        assert_eq!(expand("move-workspace", args(&["--workspace", "7", "--window", "9", "--activate"])).unwrap(),
            ("workspace.move_to_window".into(), serde_json::json!({"workspace_id":"7","window_id":"9","activate":true})));
        assert_eq!(expand("set-pane-zoom", args(&["--pane", "12", "--zoomed", "false"])).unwrap(),
            ("pane.zoom.set".into(), serde_json::json!({"pane_id":"12","zoomed":false})));
        for (command, input) in [
            ("move-panel", vec!["--panel", "42"]),
            ("move-workspace", vec!["--workspace", "7", "--window", ""]),
            ("move-panel", vec!["--panel", "42", "--workspace", "7", "--workspace", "8"]),
            ("set-pane-zoom", vec!["--pane", "12", "--zoomed", "yes"]),
            ("set-pane-zoom", vec!["--pane", "12"]),
            ("set-pane-zoom", vec!["--pane", "12", "--zoomed", "true", "--activate"]),
        ] { assert!(expand(command, args(&input)).is_err()); }
    }

    #[test]
    fn creation_keeps_selection_opt_in() {
        let (method, params) = expand(
            "new-workspace",
            args(&["--window", "42", "--name", "世界", "--focus", "false"]),
        )
        .unwrap();
        assert_eq!(method, "workspace.create");
        assert_eq!(params["focus"], false);
        assert_eq!(params["name"], "世界");
        assert!(params.get("activate").is_none());
        assert_eq!(
            expand("new-window", args(&["--activate"])).unwrap().1["activate"],
            true
        );
        assert!(
            expand(
                "new-workspace",
                args(&["--window", "42", "--focus", "maybe"])
            )
            .is_err()
        );
        assert!(expand("new-workspace", args(&["--name", "Missing window"])).is_err());
    }

    #[test]
    fn unsupported_or_ambiguous_options_never_silently_succeed() {
        for (cmd, options) in [
            (
                "read-screen",
                vec!["--surface", "7", "--selection", "--scrollback"],
            ),
            ("send", vec!["--surface", "7", "--force", "hello"]),
            ("select-workspace", vec![]),
            ("list-workspaces", vec!["--workspace", "4"]),
            (
                "rename-workspace",
                vec!["--workspace", "3", "--workspace", "4", "name"],
            ),
        ] {
            assert!(expand(cmd, args(&options)).is_err());
        }
    }
}
