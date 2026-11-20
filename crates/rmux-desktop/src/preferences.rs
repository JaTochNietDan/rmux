//! Bounded startup configuration. No watcher or automatic file creation.
use crate::close_policy::ClosePolicy;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct HexColor(u32);
impl TryFrom<String> for HexColor {
    type Error = &'static str;
    fn try_from(text: String) -> std::result::Result<Self, Self::Error> {
        let hex = text.strip_prefix('#').ok_or("color must use #RRGGBB")?;
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("color must use #RRGGBB");
        }
        Ok(Self(u32::from_str_radix(hex, 16).map_err(|_| "invalid color")?))
    }
}
impl From<HexColor> for String {
    fn from(color: HexColor) -> Self { format!("#{:06x}", color.0) }
}
impl HexColor {
    fn rgb(self) -> rmux_terminal::Color {
        rmux_terminal::Color((self.0 >> 16) as u8, (self.0 >> 8) as u8, self.0 as u8)
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTheme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnfocusedCursor {
    #[default]
    Hollow,
    Hidden,
}
impl UnfocusedCursor {
    pub fn shape(
        self,
        focused: bool,
        composing: bool,
        terminal: rmux_terminal::CursorShape,
    ) -> Option<rmux_terminal::CursorShape> {
        use rmux_terminal::CursorShape;
        if focused {
            Some(if composing { CursorShape::Bar } else { terminal })
        } else {
            match self {
                Self::Hollow => Some(CursorShape::Hollow),
                Self::Hidden => None,
            }
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CustomCommand {
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: Option<std::path::PathBuf>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub commands: Vec<CustomCommand>,
    pub keybindings: Vec<crate::shortcuts::Binding>,
    pub group_new_workspace_placement: crate::workspace_groups::Placement,
    pub session_file: Option<std::path::PathBuf>,
    pub font_size: f32,
    pub terminal_padding: u8,
    pub font_family: String,
    pub scrollback_mib: u16,
    pub copy_on_select: bool,
    pub macos_option_as_alt: bool,
    pub unfocused_cursor: UnfocusedCursor,
    pub foreground_color: Option<HexColor>,
    pub background_color: Option<HexColor>,
    pub cursor_color: Option<HexColor>,
    pub ansi_palette: Option<[HexColor; 16]>,
    pub terminal_theme: TerminalTheme,
    pub confirm_close: ClosePolicy,
    #[serde(skip)]
    source: Option<std::path::PathBuf>,
    #[serde(skip)]
    required: bool,
    #[serde(skip)]
    policy_override: Option<ClosePolicy>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            keybindings: Vec::new(),
            session_file: None,
            group_new_workspace_placement: crate::workspace_groups::Placement::default(),
            font_size: 14.,
            terminal_padding: 8,
            font_family: crate::FONT_FAMILY.into(),
            scrollback_mib: 4,
            copy_on_select: false,
            macos_option_as_alt: true,
            unfocused_cursor: UnfocusedCursor::default(),
            foreground_color: None,
            background_color: None,
            cursor_color: None,
            ansi_palette: None,
            terminal_theme: TerminalTheme::default(),
            confirm_close: ClosePolicy::Active,
            source: None,
            required: false,
            policy_override: None,
        }
    }
}
/// Follow changed defaults only for font attributes still using the previous
/// defaults. Per-terminal size and family overrides are independent.
pub fn updated_font(current_size: f32, current_family: &str, previous_size: f32, previous_family: &str, next: &Preferences) -> (f32, String) {
    let size = if current_size == previous_size { next.font_size } else { current_size };
    let family = if current_family == previous_family { &next.font_family } else { current_family };
    (size, family.to_owned())
}

impl gpui::Global for Preferences {}
#[derive(Default)]
pub struct ReloadState(pub bool);
impl gpui::Global for ReloadState {}
impl Preferences {
    pub fn terminal_palette(&self) -> Option<[rmux_terminal::Color; 16]> {
        self.ansi_palette.map(|colors| colors.map(HexColor::rgb)).or_else(|| {
            match self.terminal_theme {
                TerminalTheme::Dark => None,
                TerminalTheme::Light => Some([
                    0x24292f, 0xa82020, 0x176b35, 0x805c00,
                    0x0755a8, 0x7839a5, 0x096b72, 0x57606a,
                    0x626b75, 0xc12c2c, 0x24833f, 0x916a00,
                    0x1669bf, 0x9047b5, 0x147e84, 0x6e7781,
                ].map(|value| HexColor(value).rgb())),
            }
        })
    }
    pub fn terminal_colors(&self) -> [Option<rmux_terminal::Color>; 3] {
        let defaults = match self.terminal_theme {
            TerminalTheme::Dark => [None; 3],
            TerminalTheme::Light => [Some(HexColor(0x24292f)), Some(HexColor(0xffffff)), Some(HexColor(0x0755a8))],
        };
        let overrides = [self.foreground_color, self.background_color, self.cursor_color];
        std::array::from_fn(|index| overrides[index].or(defaults[index]).map(HexColor::rgb))
    }
    /// Called only by the explicit Open Settings File UI action.
    pub fn prepare_file(&self) -> Result<std::path::PathBuf> {
        use std::io::Write;
        let path = self
            .source
            .clone()
            .context("No configuration path is available")?;
        if path.try_exists()? {
            ensure!(path.is_file(), "configuration path is not a file");
            return Ok(path);
        }
        let parent = path.parent().context("configuration path has no parent")?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&serde_json::to_vec_pretty(&Self::default())?)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                ensure!(path.is_file(), "configuration path is not a file");
            }
            Err(error) => return Err(error.into()),
        }
        Ok(path)
    }
    pub fn close_policy_overridden(&self) -> bool { self.policy_override.is_some() }

    /// Compare precisely the session values written by Save Settings. A close
    /// policy supplied by the environment is intentionally not persisted.
    pub fn same_saved_appearance(&self, other: &Self) -> bool {
        self.font_size == other.font_size
            && self.terminal_padding == other.terminal_padding
            && self.terminal_theme == other.terminal_theme
            && self.unfocused_cursor == other.unfocused_cursor
            && self.copy_on_select == other.copy_on_select
            && self.group_new_workspace_placement == other.group_new_workspace_placement
            && (self.close_policy_overridden() || self.confirm_close == other.confirm_close)
    }

    pub fn save_appearance(&self) -> Result<std::path::PathBuf> {
        use std::io::Write;
        let path = self.source.clone().context("No configuration path is available")?;
        // Follow an existing config symlink instead of replacing the link itself.
        let path = if path.try_exists()? { path.canonicalize()? } else { path };
        let mut latest = Self::read(&path, false)?;
        latest.font_size = self.font_size;
        latest.terminal_padding = self.terminal_padding;
        latest.terminal_theme = self.terminal_theme;
        latest.unfocused_cursor = self.unfocused_cursor;
        latest.copy_on_select = self.copy_on_select;
        latest.group_new_workspace_placement = self.group_new_workspace_placement;
        if !self.close_policy_overridden() { latest.confirm_close = self.confirm_close; }
        let bytes = serde_json::to_vec_pretty(&latest)?;
        Self::decode(&bytes)?;
        let parent = path.parent().context("Configuration path has no parent")?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path)?;
        Ok(path)
    }

    pub fn scrollback_bytes(&self) -> usize {
        usize::from(self.scrollback_mib) * 1024 * 1024
    }
    pub fn reload(&self) -> Result<Self> {
        let mut next = match &self.source {
            Some(path) => Self::read(path, self.required)?,
            None => Self::default(),
        };
        if let Some(policy) = self.policy_override {
            next.confirm_close = policy;
        }
        next.source = self.source.clone();
        next.required = self.required;
        next.policy_override = self.policy_override;
        Ok(next)
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 64 * 1024, "configuration exceeds 64 KiB");
        let value: Self = serde_json::from_slice(bytes)?;
        ensure!(value.commands.len() <= 32, "at most 32 custom commands are allowed");
        let mut names = std::collections::HashSet::new();
        for command in &value.commands {
            ensure!(!command.name.trim().is_empty() && command.name.len() <= 128 && !command.name.chars().any(char::is_control), "command name must be nonempty, at most 128 bytes and contain no controls");
            ensure!(names.insert(command.name.trim().to_lowercase()), "duplicate custom command name");
            ensure!(!command.argv.is_empty() && command.argv.len() <= 64 && !command.argv[0].is_empty(), "command argv must contain a program and at most 64 entries");
            ensure!(command.argv.iter().all(|arg| !arg.contains('\0')) && command.argv.iter().map(String::len).sum::<usize>() <= 16 * 1024, "command arguments must fit 16 KiB without NULs");
            ensure!(command.env.len() <= 64 && command.env.iter().map(|(key, value)| key.len() + value.len()).sum::<usize>() <= 16 * 1024, "command environment must fit 64 entries and 16 KiB");
            for (key, value) in &command.env {
                ensure!(!key.is_empty() && key.len() <= 128 && key.bytes().enumerate().all(|(i, byte)| byte == b'_' || byte.is_ascii_alphabetic() || (i > 0 && byte.is_ascii_digit())) && !value.contains('\0'), "command environment requires portable variable names and values without NULs");
            }
            #[cfg(windows)]
            {
                let mut keys = std::collections::HashSet::new();
                ensure!(command.env.keys().all(|key| keys.insert(key.to_ascii_lowercase())), "duplicate case-insensitive environment variable");
            }
            ensure!(command.cwd.as_ref().is_none_or(|path| path.is_absolute() && path.as_os_str().len() <= 8192), "command cwd must be an absolute path up to 8192 bytes");
        }
        ensure!(value.terminal_padding <= 32, "terminal_padding must be between 0 and 32");
        ensure!(value.session_file.as_ref().is_none_or(|path| path.is_absolute() && path.as_os_str().len() <= 8192), "session_file must be an absolute path up to 8192 bytes");
        ensure!(
            value.scrollback_mib <= 256,
            "scrollback_mib must be between 0 and 256"
        );
        ensure!(
            !value.font_family.trim().is_empty()
                && value.font_family.len() <= 128
                && !value.font_family.chars().any(char::is_control),
            "font_family must be a nonempty name up to 128 bytes without controls"
        );
        ensure!(
            value.font_size.is_finite() && (8. ..=40.).contains(&value.font_size),
            "font_size must be between 8 and 40"
        );
        crate::shortcuts::compile(&value)?;
        Ok(value)
    }
    fn read(path: &Path, required: bool) -> Result<Self> {
        let file = match std::fs::File::open(path) {
            Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            result => result.with_context(|| format!("configuration {}", path.display()))?,
        };
        let mut bytes = Vec::new();
        file.take(64 * 1024 + 1).read_to_end(&mut bytes)?;
        Self::decode(&bytes).with_context(|| format!("configuration {}", path.display()))
    }
    pub fn load() -> Result<Self> {
        let explicit = std::env::var_os("RMUX_CONFIG");
        let path = if let Some(path) = &explicit {
            ensure!(!path.is_empty(), "RMUX_CONFIG must not be empty");
            Some(std::path::PathBuf::from(path))
        } else if cfg!(windows) {
            std::env::var_os("APPDATA").map(|root| Path::new(&root).join("rmux/config.json"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .filter(|root| !root.is_empty())
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|root| Path::new(&root).join(".config")))
                .map(|root| root.join("rmux/config.json"))
        };
        let mut preferences = Self {
            source: path.map(std::path::absolute).transpose()?,
            required: explicit.is_some(),
            ..Self::default()
        };
        if let Some(policy) = std::env::var_os("RMUX_CONFIRM_CLOSE") {
            preferences.policy_override = Some(
                ClosePolicy::parse(Some(policy.to_str().unwrap_or("invalid")))
                    .map_err(anyhow::Error::msg)?,
            );
        }
        preferences.reload()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_appearance_comparison_tracks_only_persisted_controls() {
        let saved = Preferences::default();
        let mut current = saved.clone();
        assert!(saved.same_saved_appearance(&current));
        current.font_size += 1.;
        assert!(!saved.same_saved_appearance(&current));
        current = saved.clone();
        current.copy_on_select = !current.copy_on_select;
        assert!(!saved.same_saved_appearance(&current));
        current = saved.clone();
        current.scrollback_mib += 1;
        assert!(saved.same_saved_appearance(&current));
    }

    #[test]
    fn group_placement_defaults_and_rejects_unknown_values() -> Result<()> {
        use crate::workspace_groups::Placement;
        assert_eq!(Preferences::decode(b"{}")?.group_new_workspace_placement, Placement::AfterCurrent);
        assert_eq!(Preferences::decode(br#"{"group_new_workspace_placement":"top"}"#)?.group_new_workspace_placement, Placement::Top);
        assert_eq!(Preferences::decode(br#"{"group_new_workspace_placement":"end"}"#)?.group_new_workspace_placement, Placement::End);
        assert!(Preferences::decode(br#"{"group_new_workspace_placement":"random"}"#).is_err());
        Ok(())
    }
    #[test]
    fn recovery_path_is_opt_in_and_requires_an_absolute_path() -> Result<()> {
        assert!(Preferences::decode(b"{}")?.session_file.is_none());
        assert!(Preferences::decode(br#"{"session_file":"relative.json"}"#).is_err());
        assert!(Preferences::decode(br#"{"session_file":""}"#).is_err());
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("session.json");
        let bytes = serde_json::to_vec(&serde_json::json!({"session_file":path}))?;
        assert_eq!(Preferences::decode(&bytes)?.session_file.as_ref(), Some(&path));
        Ok(())
    }
    #[test]
    fn theme_defaults_allow_explicit_color_and_palette_overrides() -> Result<()> {
        let light = Preferences::decode(br#"{"terminal_theme":"light"}"#)?;
        assert_eq!(light.terminal_colors()[1], Some(rmux_terminal::Color(255, 255, 255)));
        assert!(light.terminal_palette().is_some());
        let overridden = Preferences::decode(br##"{"terminal_theme":"light","background_color":"#010203"}"##)?;
        assert_eq!(overridden.terminal_colors()[1], Some(rmux_terminal::Color(1, 2, 3)));
        assert_eq!(overridden.terminal_colors()[0], light.terminal_colors()[0]);
        let value = serde_json::json!({"terminal_theme":"light", "ansi_palette": vec!["#112233"; 16]});
        assert_eq!(Preferences::decode(&serde_json::to_vec(&value)?)?.terminal_palette(), Some([rmux_terminal::Color(17, 34, 51); 16]));
        let dark = Preferences::decode(br#"{"terminal_theme":"dark"}"#)?;
        assert_eq!(dark.terminal_colors(), [None; 3]);
        assert_eq!(dark.terminal_palette(), None);
        assert!(Preferences::decode(br#"{"terminal_theme":"unknown"}"#).is_err());
        Ok(())
    }
    #[test]
    fn terminal_color_settings_validate_and_round_trip() -> Result<()> {
        let preferences = Preferences::decode(br##"{"foreground_color":"#AABBCC","background_color":"#010203","cursor_color":null}"##)?;
        assert_eq!(preferences.terminal_colors(), [Some(rmux_terminal::Color(170, 187, 204)), Some(rmux_terminal::Color(1, 2, 3)), None]);
        let serialized = serde_json::to_vec(&preferences)?;
        assert_eq!(Preferences::decode(&serialized)?.terminal_colors(), preferences.terminal_colors());
        for invalid in ["red", "123456", "#fff", "#11223344", "#gg0000"] {
            let value = serde_json::json!({"background_color": invalid});
            assert!(Preferences::decode(&serde_json::to_vec(&value)?).is_err());
        }
        assert_eq!(Preferences::decode(b"{}")?.terminal_colors(), [None; 3]);
        for count in [0, 15, 16, 17] {
            let value = serde_json::json!({"ansi_palette": vec!["#010203"; count]});
            let decoded = Preferences::decode(&serde_json::to_vec(&value)?);
            if count == 16 {
                assert_eq!(decoded?.terminal_palette(), Some([rmux_terminal::Color(1, 2, 3); 16]));
            } else {
                assert!(decoded.is_err());
            }
        }
        Ok(())
    }
    #[test]
    fn cursor_preferences_preserve_application_and_composition_shapes() -> Result<()> {
        use rmux_terminal::CursorShape;
        let defaults = Preferences::decode(b"{}")?;
        let hidden = Preferences::decode(br#"{"unfocused_cursor":"hidden"}"#)?;
        for shape in [CursorShape::Block, CursorShape::Bar, CursorShape::Underline, CursorShape::Hollow] {
            assert_eq!(defaults.unfocused_cursor.shape(true, false, shape), Some(shape));
            assert_eq!(hidden.unfocused_cursor.shape(true, false, shape), Some(shape));
            assert_eq!(defaults.unfocused_cursor.shape(true, true, shape), Some(CursorShape::Bar));
            assert_eq!(defaults.unfocused_cursor.shape(false, false, shape), Some(CursorShape::Hollow));
            assert_eq!(hidden.unfocused_cursor.shape(false, false, shape), None);
        }
        assert!(Preferences::decode(br#"{"unfocused_cursor":"blink"}"#).is_err());
        assert!(Preferences::decode(br#"{"unfocused_cursor":false}"#).is_err());
        Ok(())
    }
    #[test]
    fn custom_commands_validate_limits_identity_and_literal_arguments() -> Result<()> {
        let valid = serde_json::json!({"commands":[{"name":"Build 世界", "argv":["mise","exec","--","echo","a b","", "$(literal)"], "cwd":"/tmp"}]});
        let preferences = Preferences::decode(&serde_json::to_vec(&valid)?)?;
        assert_eq!(preferences.commands[0].argv[5], "");
        assert_eq!(preferences.commands[0].argv[6], "$(literal)");
        let with_env = serde_json::json!({"commands":[{"name":"Env","argv":["tool"],"env":{"VALID_1":"世界","EMPTY":""}}]});
        assert_eq!(Preferences::decode(&serde_json::to_vec(&with_env)?)?.commands[0].env["VALID_1"], "世界");
        for commands in [
            serde_json::json!([{"name":"Build","argv":[]}]),
            serde_json::json!([{"name":"Build","argv":[""]}]),
            serde_json::json!([{"name":"Build","argv":["tool"],"cwd":"relative"}]),
            serde_json::json!([{"name":"Build","argv":["tool"]},{"name":"build","argv":["other"]}]),
            serde_json::json!([{"name":"Build","argv":["tool","bad\u{0}argument"]}]),
            serde_json::json!([{"name":"Build","argv":vec!["tool";65]}]),
            serde_json::json!([{"name":"Build","argv":["tool"],"env":{"1BAD":"value"}}]),
            serde_json::json!([{"name":"Build","argv":["tool"],"env":{"BAD=KEY":"value"}}]),
            serde_json::json!([{"name":"Build","argv":["tool"],"env":{"VALID":"bad\u{0}value"}}]),
        ] { assert!(Preferences::decode(&serde_json::to_vec(&serde_json::json!({"commands":commands}))?).is_err()); }
        Ok(())
    }

    #[test]
    fn font_defaults_update_inherited_attributes_and_preserve_overrides() {
        let next = Preferences { font_size: 18., font_family: "Next Mono".into(), ..Preferences::default() };
        assert_eq!(updated_font(14., "Old Mono", 14., "Old Mono", &next), (18., "Next Mono".into()));
        assert_eq!(updated_font(22., "Old Mono", 14., "Old Mono", &next), (22., "Next Mono".into()));
        assert_eq!(updated_font(14., "Custom Mono", 14., "Old Mono", &next), (18., "Custom Mono".into()));
        assert_eq!(updated_font(22., "Custom Mono", 14., "Old Mono", &next), (22., "Custom Mono".into()));
    }

    #[test]
    fn appearance_save_merges_other_settings_and_preserves_invalid_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("config.json");
        let mut preferences = Preferences { source: Some(path.clone()), ..Preferences::default() };
        preferences.font_size = 19.;
        preferences.terminal_padding = 12;
        preferences.copy_on_select = true;
        preferences.group_new_workspace_placement = crate::workspace_groups::Placement::End;
        preferences.confirm_close = ClosePolicy::Never;
        std::fs::write(&path, br##"{"font_family":"Custom Mono","scrollback_mib":16,"background_color":"#112233","confirm_close":"always"}"##)?;
        preferences.save_appearance()?;
        let saved = Preferences::read(&path, true)?;
        assert_eq!(saved.font_size, 19.);
        assert_eq!(saved.terminal_padding, 12);
        assert!(saved.copy_on_select);
        assert_eq!(saved.group_new_workspace_placement, crate::workspace_groups::Placement::End);
        assert_eq!(saved.font_family, "Custom Mono");
        assert_eq!(saved.scrollback_mib, 16);
        assert_eq!(saved.confirm_close, ClosePolicy::Never);
        preferences.policy_override = Some(ClosePolicy::Always);
        preferences.confirm_close = ClosePolicy::Always;
        crate::settings_view::adjust(&mut preferences, "close", 1);
        assert_eq!(preferences.confirm_close, ClosePolicy::Always);
        preferences.save_appearance()?;
        assert_eq!(Preferences::read(&path, true)?.confirm_close, ClosePolicy::Never);
        assert_eq!(saved.terminal_colors()[1], Some(rmux_terminal::Color(0x11, 0x22, 0x33)));
        std::fs::write(&path, b"unfinished user edit")?;
        assert!(preferences.save_appearance().is_err());
        assert_eq!(std::fs::read(&path)?, b"unfinished user edit");
        Ok(())
    }

    #[test]
    fn explicit_settings_open_creates_defaults_without_overwriting() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("nested/config.json");
        let preferences = Preferences {
            source: Some(path.clone()),
            ..Preferences::default()
        };
        assert_eq!(preferences.prepare_file()?, path);
        assert_eq!(Preferences::read(&path, true)?.font_size, 14.);
        std::fs::write(&path, b"user text, even if temporarily invalid")?;
        preferences.prepare_file()?;
        assert_eq!(
            std::fs::read(&path)?,
            b"user text, even if temporarily invalid"
        );
        assert!(Preferences::default().prepare_file().is_err());
        Ok(())
    }
    #[test]
    fn reload_keeps_source_override_and_previous_valid_settings() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("config.json");
        let source = Preferences {
            source: Some(path.clone()),
            required: true,
            policy_override: Some(ClosePolicy::Never),
            ..Preferences::default()
        };
        std::fs::write(&path, r#"{"font_size":18,"confirm_close":"always"}"#)?;
        let current = source.reload()?;
        assert_eq!(current.font_size, 18.);
        assert_eq!(current.confirm_close, ClosePolicy::Never);
        std::fs::write(&path, r#"{"font_size":22}"#)?;
        assert_eq!(current.reload()?.font_size, 22.);
        std::fs::write(&path, r#"{"font_size":100}"#)?;
        assert!(current.reload().is_err());
        assert_eq!(current.font_size, 18.);
        std::fs::remove_file(&path)?;
        assert!(current.reload().is_err());
        Ok(())
    }
    #[test]
    fn config_defaults_validation_and_file_limits() -> Result<()> {
        let defaults = Preferences::decode(b"{}")?;
        assert_eq!(defaults.font_size, 14.);
        assert_eq!(defaults.terminal_padding, 8);
        assert!(!defaults.copy_on_select);
        assert!(defaults.macos_option_as_alt);
        assert!(!Preferences::decode(br#"{"macos_option_as_alt":false}"#)?.macos_option_as_alt);
        assert!(Preferences::decode(br#"{"copy_on_select":true}"#)?.copy_on_select);
        assert_eq!(defaults.confirm_close, ClosePolicy::Active);
        let custom = Preferences::decode(br#"{"font_size":18,"confirm_close":"always"}"#)?;
        assert_eq!(custom.font_size, 18.);
        assert_eq!(custom.confirm_close, ClosePolicy::Always);
        for invalid in [
            r#"{"font_size":7}"#,
            r#"{"terminal_padding":33}"#,
            r#"{"terminal_padding":-1}"#,
            r#"{"copy_on_select":"true"}"#,
            r#"{"macos_option_as_alt":"false"}"#,
            r#"{"font_size":41}"#,
            r#"{"confirm_close":"bad"}"#,
            r#"{"font_szie":18}"#,
            r#"{"scrollback_mib":257}"#,
            r#"{"scrollback_mib":-1}"#,
            r#"{"font_family":""}"#,
            r#"{"font_family":"bad\nname"}"#,
        ] {
            assert!(Preferences::decode(invalid.as_bytes()).is_err());
        }
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("config.json");
        assert_eq!(Preferences::read(&path, false)?.font_size, 14.);
        assert!(Preferences::read(&path, true).is_err());
        std::fs::write(&path, vec![b' '; 64 * 1024 + 1])?;
        assert!(Preferences::read(&path, true).is_err());
        Ok(())
    }
}
