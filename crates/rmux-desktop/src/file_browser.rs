//! On-demand local directory snapshots. Never reads contents or follows discovered links.
use std::path::PathBuf;
pub const MAX_ENTRIES: usize = 2048;
pub struct Entry { pub path: PathBuf, pub directory: bool, pub symlink: bool }
pub struct Listing { pub path: PathBuf, pub entries: Vec<Entry>, pub truncated: bool }
pub fn list(path: PathBuf) -> anyhow::Result<Listing> {
    anyhow::ensure!(path.is_absolute(), "File browser requires an absolute directory");
    let mut entries = Vec::new();
    let mut truncated = false;
    for entry in std::fs::read_dir(&path)? {
        let entry = entry?;
        if entries.len() == MAX_ENTRIES { truncated = true; break; }
        let kind = entry.file_type()?;
        entries.push(Entry { path: entry.path(), directory: kind.is_dir(), symlink: kind.is_symlink() });
    }
    entries.sort_by_cached_key(|entry| (!entry.directory, entry.path.file_name().unwrap_or_default().to_string_lossy().to_lowercase(), entry.path.clone()));
    Ok(Listing { path, entries, truncated })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn discovered_links_are_not_traversed() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        std::os::unix::fs::symlink("missing", root.path().join("dangling"))?;
        std::os::unix::fs::symlink(root.path(), root.path().join("loop"))?;
        let listing = super::list(root.path().to_owned())?;
        assert_eq!(listing.entries.len(), 2);
        assert!(listing.entries.iter().all(|entry| entry.symlink && !entry.directory));
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
