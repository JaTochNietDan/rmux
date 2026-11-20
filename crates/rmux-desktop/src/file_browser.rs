//! On-demand local directory snapshots and bounded inert text previews.
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
#[derive(Default)]
pub struct Cancellation(pub Arc<AtomicBool>);
impl Drop for Cancellation {
    fn drop(&mut self) { self.0.store(true, Ordering::Release); }
}
fn check_cancel(canceled: &AtomicBool) -> anyhow::Result<()> {
    anyhow::ensure!(!canceled.load(Ordering::Acquire), "File loading canceled");
    Ok(())
}
pub const MAX_ENTRIES: usize = 2048;
pub struct Entry { pub path: PathBuf, pub directory: bool, pub symlink: bool }
pub struct Listing { pub path: PathBuf, pub entries: Vec<Entry>, pub truncated: bool }
pub struct Preview { pub path: PathBuf, pub lines: Vec<String>, pub truncated: bool }
#[cfg(test)]
fn preview(path: PathBuf) -> anyhow::Result<Preview> { preview_cancellable(path, &AtomicBool::new(false)) }
pub fn preview_cancellable(path: PathBuf, canceled: &AtomicBool) -> anyhow::Result<Preview> {
    use std::io::Read;
    check_cancel(canceled)?;
    anyhow::ensure!(path.is_absolute(), "Preview requires an absolute file path");
    anyhow::ensure!(std::fs::symlink_metadata(&path)?.file_type().is_file(), "Preview supports regular files only");
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    check_cancel(canceled)?;
    let mut file = options.open(&path)?;
    anyhow::ensure!(file.metadata()?.is_file(), "Preview supports regular files only");
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    while bytes.len() < 65537 {
        check_cancel(canceled)?;
        let limit = chunk.len().min(65537 - bytes.len());
        let count = match file.read(&mut chunk[..limit]) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 { break; }
        bytes.extend_from_slice(&chunk[..count]);
    }
    check_cancel(canceled)?;
    let mut truncated = bytes.len() > 65536;
    bytes.truncate(65536);
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if truncated && error.error_len().is_none() => std::str::from_utf8(&bytes[..error.valid_up_to()])?,
        Err(_) => anyhow::bail!("Preview requires UTF-8 text"),
    };
    anyhow::ensure!(!text.contains('\0'), "Binary files cannot be previewed as text");
    let mut source = text.lines();
    let lines = source.by_ref().take(512).map(str::to_owned).collect();
    truncated |= source.next().is_some();
    Ok(Preview { path, lines, truncated })
}
#[cfg(test)]
fn list(path: PathBuf) -> anyhow::Result<Listing> { list_cancellable(path, &AtomicBool::new(false)) }
pub fn list_cancellable(path: PathBuf, canceled: &AtomicBool) -> anyhow::Result<Listing> {
    check_cancel(canceled)?;
    anyhow::ensure!(path.is_absolute(), "File browser requires an absolute directory");
    let mut entries = Vec::new();
    let mut truncated = false;
    for entry in std::fs::read_dir(&path)? {
        check_cancel(canceled)?;
        let entry = entry?;
        if entries.len() == MAX_ENTRIES { truncated = true; break; }
        let kind = entry.file_type()?;
        entries.push(Entry { path: entry.path(), directory: kind.is_dir(), symlink: kind.is_symlink() });
    }
    check_cancel(canceled)?;
    entries.sort_by_cached_key(|entry| (!entry.directory, entry.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase(), entry.path.clone()));
    Ok(Listing { path, entries, truncated })
}

#[cfg(test)]
mod tests {
    #[test]
    fn dropping_load_owner_cancels_before_filesystem_access() {
        let owner = super::Cancellation::default();
        let token = owner.0.clone();
        assert!(!token.load(super::Ordering::Acquire));
        drop(owner);
        let path = std::path::PathBuf::from("relative-invalid-path");
        assert_eq!(super::list_cancellable(path.clone(), &token).err().unwrap().to_string(), "File loading canceled");
        assert_eq!(super::preview_cancellable(path, &token).err().unwrap().to_string(), "File loading canceled");
    }
    #[test]
    fn text_preview_bounds_unicode_and_rejects_binary() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("script.sh");
        std::fs::write(&path, "echo do-not-run\r\n世界\n")?;
        let preview = super::preview(path.clone())?;
        assert_eq!(preview.lines, ["echo do-not-run", "世界"]);
        assert!(!preview.truncated);
        std::fs::write(&path, format!("{}界", "x".repeat(65535)))?;
        let preview = super::preview(path.clone())?;
        assert!(preview.truncated);
        assert_eq!(preview.lines[0].len(), 65535);
        std::fs::write(&path, "line\n".repeat(513))?;
        let preview = super::preview(path.clone())?;
        assert_eq!(preview.lines.len(), 512);
        assert!(preview.truncated);
        for bytes in [vec![0xff], vec![b'a', 0, b'b']] {
            std::fs::write(&path, bytes)?;
            assert!(super::preview(path.clone()).is_err());
        }
        assert!(super::preview(root.path().to_owned()).is_err());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn discovered_links_are_not_traversed() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        std::os::unix::fs::symlink("missing", root.path().join("dangling"))?;
        std::os::unix::fs::symlink(root.path(), root.path().join("loop"))?;
        let listing = super::list(root.path().to_owned())?;
        assert_eq!(listing.entries.len(), 2);
        assert!(listing.entries.iter().all(|entry| entry.symlink && !entry.directory));
        assert!(super::preview(root.path().join("loop")).is_err());
        Ok(())
    }
    #[test]
    fn directory_snapshot_is_bounded_and_keeps_paths_literal() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        std::fs::create_dir(root.path().join("z directory"))?;
        std::fs::write(root.path().join("a 世界.sh"), "not executed")?;
        std::fs::write(root.path().join(".hidden"), "hidden")?;
        let listing = super::list(root.path().to_owned())?;
        assert_eq!(listing.entries.len(), 3);
        assert!(listing.entries[0].directory);
        assert!(listing.entries.iter().any(|entry| entry.path.ends_with("a 世界.sh")));
        assert!(!listing.truncated);
        for index in 0..super::MAX_ENTRIES { std::fs::write(root.path().join(format!("item-{index}")), "")?; }
        let listing = super::list(root.path().to_owned())?;
        assert_eq!(listing.entries.len(), super::MAX_ENTRIES);
        assert!(listing.truncated);
        assert!(super::list(root.path().join("a 世界.sh")).is_err());
        assert!(super::list("relative".into()).is_err());
        Ok(())
    }
}
