//! Bounded, inert import of Ghostty-style terminal color assignments.
use crate::preferences::HexColor;
use anyhow::{Context, Result, ensure};
use std::{io::Read, path::Path};

#[derive(Clone, Default)]
pub struct Theme {
    pub colors: [Option<HexColor>; 3],
    pub palette: Option<[HexColor; 16]>,
    pub palette_entries: [Option<HexColor>; 16],
    pub selection_background: Option<HexColor>,
    pub selection_foreground: Option<HexColor>,
    pub cursor_text: Option<HexColor>,
}

pub fn read(path: &Path) -> Result<Theme> {
    ensure!(path.is_absolute(), "theme_file must be an absolute path");
    ensure!(std::fs::symlink_metadata(path)?.is_file(), "theme_file must be a regular file, not a symbolic link");
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    ensure!(file.metadata()?.is_file(), "theme_file must be a regular file");
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "theme_file exceeds 64 KiB");
    parse(std::str::from_utf8(&bytes).context("theme_file must be UTF-8")?)
}

fn parse(text: &str) -> Result<Theme> {
    let mut theme = Theme::default();
    let mut palette = [None; 16];
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let parse_line = || -> Result<(&str, HexColor, Option<usize>)> {
            let (key, value) = line.split_once('=').context("expected key = value")?;
            let key = key.trim();
            ensure!(matches!(key, "foreground" | "background" | "cursor-color" | "cursor-text" | "selection-background" | "selection-foreground" | "palette"), "unsupported theme setting: {key}");
            let (value, slot) = if key == "palette" {
                let (slot, color) = value.split_once('=').context("expected palette = N=RRGGBB")?;
                let slot = slot.trim();
                ensure!(!slot.is_empty() && slot.bytes().all(|b| b.is_ascii_digit()), "palette index must be decimal 0–15");
                let slot: usize = slot.parse()?;
                ensure!(slot < 16, "only ANSI palette indices 0–15 are supported");
                (color, Some(slot))
            } else { (value, None) };
            let value = value.trim();
            let value = value.strip_prefix('#').unwrap_or(value);
            ensure!(value.len() == 6 && value.bytes().all(|b| b.is_ascii_hexdigit()), "expected RRGGBB or #RRGGBB color");
            let color = HexColor::try_from(format!("#{value}")).map_err(anyhow::Error::msg)?;
            Ok((key, color, slot))
        };
        let (key, color, slot) = parse_line().with_context(|| format!("theme line {}", index + 1))?;
        if let Some(slot) = slot { palette[slot] = Some(color); }
        else if key == "cursor-text" { theme.cursor_text = Some(color); }
        else if key == "selection-foreground" { theme.selection_foreground = Some(color); }
        else if key == "selection-background" { theme.selection_background = Some(color); }
        else { theme.colors[match key { "foreground" => 0, "background" => 1, _ => 2 }] = Some(color); }
    }
    theme.palette_entries = palette;
    if palette.iter().all(Option::is_some) {
        theme.palette = Some(palette.map(Option::unwrap));
    }
    ensure!(theme.palette_entries.iter().any(Option::is_some) || theme.selection_background.is_some() || theme.selection_foreground.is_some() || theme.cursor_text.is_some() || theme.colors.iter().any(Option::is_some), "theme contains no supported colors");
    Ok(theme)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_file_rejects_oversize_and_nonregular_sources() -> Result<()> {
        let root = tempfile::tempdir()?;
        assert!(read(root.path()).is_err());
        let path = root.path().join("theme");
        std::fs::write(&path, "#".repeat(65537))?;
        assert!(read(&path).is_err());
        #[cfg(unix)] {
            let link = root.path().join("link");
            std::os::unix::fs::symlink(&path, &link)?;
            assert!(read(&link).is_err());
        }
        Ok(())
    }
    #[test]
    fn color_import_reports_unsupported_values_and_retains_sparse_palette() -> Result<()> {
        let text = format!("# theme\nforeground = aabbcc\nbackground=#010203\ncursor-color=445566\n{}", (0..16).map(|i| format!("palette={i}=112233\n")).collect::<String>());
        let theme = parse(&text)?;
        assert_eq!(String::from(theme.colors[0].unwrap()), "#aabbcc");
        assert_eq!(String::from(parse("selection-background=010203")?.selection_background.unwrap()), "#010203");
        assert_eq!(String::from(theme.palette.unwrap()[15]), "#112233");
        let sparse = parse("palette=1=123456")?;
        assert!(sparse.palette.is_none());
        assert!(sparse.palette_entries[0].is_none());
        assert_eq!(String::from(sparse.palette_entries[1].unwrap()), "#123456");
        for invalid in ["", "foreground=red", "background=##112233", "palette=16=112233", "command=echo hello", "cursor-opacity=0.5"] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
        Ok(())
    }
}
