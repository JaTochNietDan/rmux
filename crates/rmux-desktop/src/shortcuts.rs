use super::*;
use anyhow::{Result, ensure};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub key: String,
    pub action: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub context: Scope,
}
#[derive(Clone, Copy, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope { #[default] Terminal, Window }
impl Scope { fn context(self) -> Option<&'static str> { match self { Self::Terminal => Some("Terminal"), Self::Window => None } } }
struct Defaults(Vec<KeyBinding>);
impl Global for Defaults {}
fn action(name: &str) -> Option<Box<dyn Action>> {
    Some(match name {
        "new_workspace" => Box::new(NewWorkspace), "new_window" => Box::new(NewWindow),
        "new_tab" => Box::new(NewTab), "split_right" => Box::new(Split), "split_down" => Box::new(SplitBelow),
        "focus_left" => Box::new(FocusLeft), "focus_right" => Box::new(FocusRight),
        "focus_up" => Box::new(FocusUp), "focus_down" => Box::new(FocusDown),
        "next_workspace" => Box::new(NextWorkspace), "previous_workspace" => Box::new(PreviousWorkspace),
        "next_tab" => Box::new(NextTab), "previous_tab" => Box::new(PreviousTab),
        "tab_picker" => Box::new(ShowTabPicker), "palette" => Box::new(ShowCommandPalette),
        "toggle_zoom" => Box::new(TogglePaneZoom), "toggle_sidebar" => Box::new(ToggleSidebar),
        "settings" => Box::new(ToggleSettings), "notifications" => Box::new(ToggleNotifications),
        "next_unread" => Box::new(NextUnreadTerminal), "find" => Box::new(Find),
        "reload_settings" => Box::new(ReloadSettings), "disabled" => Box::new(NoAction {}),
        _ => return None,
    })
}
pub fn compile(settings: &Preferences) -> Result<Vec<KeyBinding>> {
    ensure!(settings.keybindings.len() <= 128, "at most 128 custom keybindings are allowed");
    let mut used = std::collections::HashSet::new();
    let mut sequences: Vec<(Option<&str>, Vec<String>)> = Vec::new();
    settings.keybindings.iter().map(|binding| {
        ensure!(binding.key.len() <= 128, "keybinding exceeds 128 bytes");
        let strokes = binding.key.split_whitespace().map(Keystroke::parse).collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!((1..=2).contains(&strokes.len()), "keybinding requires one or two strokes");
        for stroke in &strokes {
            ensure!(!stroke.key.chars().any(char::is_control) && rmux_terminal::keyboard::key_identity(&stroke.key).is_some(), "unknown shortcut key: {}", stroke.key);
            if let Some(number) = stroke.key.strip_prefix('f').and_then(|text| text.parse::<u8>().ok()) {
                ensure!(stroke.key == format!("f{number}"), "use canonical function-key spelling: f{number}");
            }
            let function = stroke.key.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()).is_some_and(|n| (1..=24).contains(&n));
            ensure!(stroke.modifiers.control || stroke.modifiers.platform || stroke.modifiers.alt || function, "custom strokes require Ctrl/Cmd/Alt or a function key");
        }
        let sequence: Vec<_> = strokes.iter().map(ToString::to_string).collect();
        let canonical = sequence.join(" ");
        ensure!(used.insert((binding.context.context(), canonical.clone())), "duplicate shortcut in the same context: {canonical}");
        for (scope, previous) in &sequences {
            if *scope == binding.context.context() && previous.len() != sequence.len()
                && (previous.starts_with(&sequence) || sequence.starts_with(previous)) {
                anyhow::bail!("shortcut prefix conflict in the same context: {} and {canonical}", previous.join(" "));
            }
        }
        sequences.push((binding.context.context(), sequence));
        let target = match (&binding.action, &binding.command) {
            (Some(name), None) => action(name).ok_or_else(|| anyhow::anyhow!("unknown shortcut action: {name}"))?,
            (None, Some(name)) => {
                ensure!(settings.commands.iter().any(|command| command.name == *name), "shortcut command is not configured: {name}");
                Box::new(RunCustomCommand { name: name.clone() }) as Box<dyn Action>
            }
            _ => anyhow::bail!("keybinding requires exactly one action or command"),
        };
        let context = binding.context.context().map(|context| KeyBindingContextPredicate::parse(context).unwrap().into());
        Ok(KeyBinding::load(&binding.key, target, context, false, None, &DummyKeyboardMapper)?)
    }).collect()
}
pub fn initialize(cx: &mut App) {
    let defaults = cx.key_bindings().borrow().bindings().cloned().collect();
    cx.set_global(Defaults(defaults));
    install(cx);
}
pub fn install(cx: &mut App) {
    let bindings = compile(cx.global::<Preferences>()).expect("validated shortcuts");
    let defaults = cx.global::<Defaults>().0.clone();
    cx.clear_key_bindings();
    cx.bind_keys(defaults);
    cx.bind_keys(bindings);
}

#[cfg(test)]
mod tests {
    use super::compile;
    use crate::{preferences::Preferences, ShowCommandPalette, ToggleSettings};
    use gpui::{KeyBinding, KeyContext, Keymap, Keystroke};
    fn settings(value: serde_json::Value) -> Preferences { serde_json::from_value(value).unwrap() }
    #[test]
    fn override_precedence_context_and_disable_use_real_keymap() {
        let defaults = vec![KeyBinding::new("ctrl-shift-p", ShowCommandPalette, None)];
        let mut map = Keymap::new(defaults.clone());
        let prefs = settings(serde_json::json!({"keybindings":[{"key":"ctrl-shift-p","action":"settings"}]}));
        map.add_bindings(compile(&prefs).unwrap());
        let keys = [Keystroke::parse("ctrl-shift-p").unwrap()];
        let terminal = [KeyContext::parse("Terminal").unwrap()];
        assert!(map.bindings_for_input(&keys, &terminal).0[0].action().as_any().is::<ToggleSettings>());
        assert!(map.bindings_for_input(&keys, &[KeyContext::parse("Settings").unwrap()]).0[0].action().as_any().is::<ShowCommandPalette>());
        let disabled = settings(serde_json::json!({"keybindings":[{"key":"ctrl-shift-p","action":"disabled"}]}));
        map.clear(); map.add_bindings(defaults.clone()); map.add_bindings(compile(&disabled).unwrap());
        assert!(map.bindings_for_input(&keys, &terminal).0.is_empty());
        map.clear(); map.add_bindings(defaults);
        assert_eq!(map.bindings().len(), 1);
        assert!(map.bindings_for_input(&keys, &terminal).0[0].action().as_any().is::<ShowCommandPalette>());
    }
    #[test]
    fn invalid_and_ambiguous_shortcuts_are_rejected() {
        for value in [
            serde_json::json!([{"key":"x","action":"settings"}]),
            serde_json::json!([{"key":"","action":"settings"}]),
            serde_json::json!([{"key":"ctrl-x","action":"missing"}]),
            serde_json::json!([{"key":"ctrl-entter","action":"settings"}]),
            serde_json::json!([{"key":"f01","action":"settings"}]),
            serde_json::json!([{"key":"ctrl-k","action":"settings"},{"key":"ctrl-k ctrl-t","action":"palette"}]),
            serde_json::json!([{"key":"ctrl-k ctrl-t","action":"settings"},{"key":"ctrl-k","action":"palette"}]),
            serde_json::json!([{"key":"ctrl-X","action":"settings"},{"key":"ctrl-shift-x","action":"palette"}]),
            serde_json::json!([{"key":"ctrl-x","command":"missing"}]),
            serde_json::json!([{"key":"ctrl-x","action":"settings","command":"missing"}]),
            serde_json::json!([{"key":"ctrl-x","action":"settings"},{"key":"ctrl-x","action":"palette"}]),
        ] { assert!(compile(&settings(serde_json::json!({"keybindings":value}))).is_err()); }
        let valid = settings(serde_json::json!({"commands":[{"name":"Tests","argv":["mise","run","test"]}],"keybindings":[{"key":"ctrl-k ctrl-t","command":"Tests"}]}));
        assert_eq!(compile(&valid).unwrap().len(), 1);
        let shared_prefix = settings(serde_json::json!({"keybindings":[{"key":"ctrl-k ctrl-t","action":"settings"},{"key":"ctrl-k ctrl-p","action":"palette"}]}));
        assert_eq!(compile(&shared_prefix).unwrap().len(), 2);
    }
}
