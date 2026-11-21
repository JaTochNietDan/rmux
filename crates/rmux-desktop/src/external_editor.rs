//! Explicit external-editor argv; file paths are data, never shell source.
use anyhow::{Result, ensure};
use std::{path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::atomic::{AtomicUsize, Ordering}};
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Editor {
    pub program: PathBuf,
    pub args: Vec<String>,
}
impl Editor {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.program.is_absolute() && self.program.as_os_str().len() <= 8192, "file_editor.program must be an absolute path up to 8192 bytes");
        ensure!(self.args.len() <= 64 && self.args.iter().all(|arg| arg.len() <= 8192 && !arg.contains('\0')), "file_editor.args must contain at most 64 arguments, each up to 8192 bytes without NULs");
        ensure!(self.args.iter().filter(|arg| arg.as_str() == "{file}").count() == 1, "file_editor.args must contain exactly one standalone {{file}} argument");
        Ok(())
    }
    fn command(&self, path: &Path) -> Result<Command> {
        self.validate()?;
        ensure!(path.is_absolute() && path.as_os_str().len() <= 8192, "Editor file path must be absolute and up to 8192 bytes");
        let mut command = Command::new(&self.program);
        for arg in &self.args {
            if arg == "{file}" { command.arg(path); } else { command.arg(arg); }
        }
        command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        Ok(command)
    }
    pub fn launch(&self, path: &Path) -> Result<RunningEditor> {
        let permit = Permit::acquire()?;
        ensure!(std::fs::symlink_metadata(path)?.is_file(), "External editing requires a regular file, not a directory or symbolic link");
        let child = self.command(path)?.spawn()?;
        Ok(RunningEditor { child, _permit: permit })
    }
}
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
struct Permit;
impl Permit {
    fn acquire() -> Result<Self> {
        ACTIVE.try_update(Ordering::AcqRel, Ordering::Acquire, |n| (n < 8).then_some(n + 1))
            .map_err(|_| anyhow::anyhow!("Eight editor launches are still active"))?;
        Ok(Self)
    }
}
impl Drop for Permit { fn drop(&mut self) { ACTIVE.fetch_sub(1, Ordering::AcqRel); } }
pub struct RunningEditor { child: Child, _permit: Permit }
impl RunningEditor {
    pub async fn reap(mut self) {
        loop {
            match self.child.try_wait() { Ok(None) => {}, _ => return }
            gpui::Timer::after(std::time::Duration::from_secs(1)).await;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_paths_remain_single_literal_arguments() -> Result<()> {
        let editor = Editor { program: std::env::current_exe()?, args: vec!["--".into(), "{file}".into()] };
        let path = std::env::temp_dir().join("spaces $(touch nope); 'quoted'.sh");
        let command = editor.command(&path)?;
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, [std::ffi::OsStr::new("--"), path.as_os_str()]);
        for args in [vec![], vec!["{file}", "{file}"], vec!["prefix{file}"], vec!["{file}", "bad\0arg"]] {
            assert!(Editor { program: editor.program.clone(), args: args.into_iter().map(String::from).collect() }.validate().is_err());
        }
        assert!(editor.command(Path::new("relative.txt")).is_err());
        Ok(())
    }
}
