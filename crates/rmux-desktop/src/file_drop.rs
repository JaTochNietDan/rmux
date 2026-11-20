//! Local path insertion; never opens file contents or executes a command.
use std::path::PathBuf;
use anyhow::{Result, ensure};

pub fn quoted_paths(paths: &[PathBuf]) -> Result<String> {
    ensure!(!paths.is_empty() && paths.len() <= 256, "Drop between 1 and 256 files");
    let mut text = String::new();
    for path in paths {
        ensure!(path.is_absolute(), "Dropped paths must be absolute");
        let path = path.to_str().ok_or_else(|| anyhow::anyhow!("Dropped path is not UTF-8"))?;
        ensure!(!path.chars().any(char::is_control), "Dropped paths cannot contain control characters");
        // Three extra bytes replace a quote with the four-byte POSIX '\'' sequence.
        let needed = path.len().checked_add(path.bytes().filter(|byte| *byte == b'\'').count() * 3)
            .and_then(|len| len.checked_add(3)).ok_or_else(|| anyhow::anyhow!("Dropped paths are too long"))?;
        ensure!(text.len() + needed <= 64 * 1024, "Quoted dropped paths exceed 64 KiB");
        text.push('\'');
        for part in path.split_inclusive('\'') {
            if let Some(part) = part.strip_suffix('\'') { text.push_str(part); text.push_str("'\\''"); }
            else { text.push_str(part); }
        }
        text.push('\'');
        text.push(' ');
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn paths_are_literal_bounded_and_never_submit_a_line() -> Result<()> {
        use std::os::unix::ffi::OsStringExt;
        let paths = vec![PathBuf::from("/tmp/a b"), PathBuf::from("/tmp/it's $(echo nope);世界")];
        let text = quoted_paths(&paths)?;
        assert_eq!(text, "'/tmp/a b' '/tmp/it'\\''s $(echo nope);世界' ");
        // Round-trip the quoted fixture through a real POSIX shell.
        let output = std::process::Command::new("/bin/sh").arg("-c")
            .arg(format!("set -- {text}; printf '%s\\0' \"$@\"")).output()?;
        assert!(output.status.success());
        assert_eq!(output.stdout, "/tmp/a b\0/tmp/it's $(echo nope);世界\0".as_bytes());
        for paths in [vec![], vec![PathBuf::from("relative")], vec![PathBuf::from("/tmp/a\nb")], vec![PathBuf::from("/x");257], vec![PathBuf::from(format!("/{}", "x".repeat(65536)))], vec![PathBuf::from(std::ffi::OsString::from_vec(vec![b'/',255]))]] {
            assert!(quoted_paths(&paths).is_err());
        }
        Ok(())
    }
}
