//! On-demand local directory snapshots and bounded inert text previews.
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
#[derive(Default)]
struct LoadLimit(AtomicUsize);
pub struct Permit(&'static LoadLimit);
impl LoadLimit {
    fn acquire(&'static self) -> anyhow::Result<Permit> {
        self.0.try_update(Ordering::AcqRel, Ordering::Acquire, |count| (count < 4).then_some(count + 1))
            .map_err(|_| anyhow::anyhow!("Four file reads are still running; retry after one finishes"))?;
        Ok(Permit(self))
    }
}
impl Permit {
    pub fn acquire() -> anyhow::Result<Self> {
        static LIMIT: LoadLimit = LoadLimit(AtomicUsize::new(0));
        LIMIT.acquire()
    }
}
impl Drop for Permit {
    fn drop(&mut self) { self.0.0.fetch_sub(1, Ordering::AcqRel); }
}
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
/// A bounded display excerpt; the original retained line remains the copy source.
pub fn preview_line_title(text: &str, query: &str) -> String {
    let query = query.to_lowercase();
    let mut start = 0;
    if !query.trim().is_empty() {
        let normalized = text.to_lowercase();
        if let Some(found) = query.split_whitespace().filter_map(|word| normalized.find(word)).min() {
            // Lowercasing can expand a scalar (e.g. İ); map folded bytes back
            // to an original scalar index before choosing the excerpt.
            let mut folded_bytes = 0;
            let mut position: usize = 0;
            for ch in text.chars() {
                let width: usize = ch.to_lowercase().map(char::len_utf8).sum();
                if folded_bytes + width > found { break; }
                folded_bytes += width;
                position += 1;
            }
            start = position.saturating_sub(32);
        }
    }
    let mut chars = text.chars().skip(start);
    let mut title = if start > 0 { "…".into() } else { String::new() };
    title.extend(chars.by_ref().take(1024).map(|ch| if ch.is_control() { ' ' } else { ch }));
    if chars.next().is_some() { title.push('…'); }
    if title.is_empty() { title.push_str("(empty line)"); }
    title
}
pub fn entered_directory(text: &str) -> Result<PathBuf, &'static str> {
    if text.is_empty() || text.len() > 4096 || text.contains('\0') {
        return Err("Enter a directory path of 1–4096 bytes without NUL characters");
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() { return Err("Enter an absolute directory path"); }
    Ok(path)
}
pub struct Entry { pub path: PathBuf, pub directory: bool, pub symlink: bool }
pub struct Listing { pub path: PathBuf, pub entries: Vec<Entry>, pub truncated: bool }
pub struct Preview { pub path: PathBuf, pub lines: Vec<String>, pub contents: String, pub truncated: bool, pub partial_line: Option<usize> }
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
    let lines: Vec<String> = source.by_ref().take(512).map(str::to_owned).collect();
    let more_lines = source.next().is_some();
    let partial_line = (truncated && !more_lines && !text.ends_with('\n'))
        .then(|| lines.len().checked_sub(1)).flatten();
    truncated |= more_lines;
    // Retain original separators, including CRLF and a trailing newline, while
    // applying exactly the same line cap as the displayed preview.
    let contents = text.split_inclusive('\n').take(512).collect();
    Ok(Preview { path, lines, contents, truncated, partial_line })
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
    #[ignore = "manual file loader performance baseline"]
    fn file_loader_baseline() -> anyhow::Result<()> {
        let root = tempfile::tempdir()?;
        for index in 0..super::MAX_ENTRIES {
            std::fs::write(root.path().join(format!("entry-{index:04}-世界.txt")), "")?;
        }
        let preview_file = tempfile::NamedTempFile::new()?;
        // More than both limits, with Unicode near the byte boundary.
        std::fs::write(preview_file.path(), "x世界\n".repeat(16384))?;
        let mut directory_times = Vec::new();
        let mut preview_times = Vec::new();
        let mut retained_text = 0;
        for _ in 0..40 {
            let start = std::time::Instant::now();
            let listing = super::list(root.path().to_owned())?;
            directory_times.push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(listing.entries.len(), super::MAX_ENTRIES);
            let start = std::time::Instant::now();
            let preview = super::preview(preview_file.path().to_owned())?;
            preview_times.push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(preview.lines.len(), 512);
            assert!(preview.truncated);
            retained_text = preview.lines.iter().map(String::len).sum::<usize>() + preview.contents.len();
        }
        directory_times.sort_by(f64::total_cmp);
        preview_times.sort_by(f64::total_cmp);
        println!("{}", serde_json::json!({
            "benchmark":"file_loader", "samples":40, "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
            "os":std::env::consts::OS, "arch":std::env::consts::ARCH,
            "directory_entries":super::MAX_ENTRIES, "directory_p50_ms":directory_times[20], "directory_p95_ms":directory_times[37],
            "preview_input_bytes":131072, "preview_lines":512, "preview_retained_text_bytes":retained_text,
            "preview_p50_ms":preview_times[20], "preview_p95_ms":preview_times[37]
        }));
        Ok(())
    }
    #[test]
    fn directory_input_preserves_literal_paths_and_rejects_relative_expansion() {
        let root = std::env::temp_dir();
        let path = root.join("space 世界 $(literal) ");
        assert_eq!(super::entered_directory(path.to_str().unwrap()).unwrap(), path);
        for text in ["", "relative", "~/project", "$HOME/project", "bad\0path"] {
            assert!(super::entered_directory(text).is_err());
        }
        assert!(super::entered_directory(&"x".repeat(4097)).is_err());
    }
    #[test]
    fn file_reads_share_a_bounded_budget_until_workers_release_it() {
        static LIMIT: super::LoadLimit = super::LoadLimit(super::AtomicUsize::new(0));
        let mut active: Vec<_> = (0..4).map(|_| LIMIT.acquire().unwrap()).collect();
        assert!(LIMIT.acquire().is_err());
        let canceled = super::Cancellation::default();
        drop(canceled);
        assert!(LIMIT.acquire().is_err(), "cancel request must not release a still-running worker's permit");
        active.pop();
        let replacement = LIMIT.acquire().unwrap();
        assert!(LIMIT.acquire().is_err());
        drop(active);
        drop(replacement);
        assert_eq!(LIMIT.0.load(super::Ordering::Acquire), 0);
    }
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
        assert_eq!(preview.contents, "echo do-not-run\r\n世界\n");
        assert!(!preview.truncated);
        assert_eq!(preview.partial_line, None);
        std::fs::write(&path, format!("{}界", "x".repeat(65535)))?;
        let preview = super::preview(path.clone())?;
        assert!(preview.truncated);
        assert_eq!(preview.lines[0].len(), 65535);
        assert_eq!(preview.contents, "x".repeat(65535));
        assert_eq!(preview.partial_line, Some(0));
        std::fs::write(&path, "line\n".repeat(513))?;
        let preview = super::preview(path.clone())?;
        assert_eq!(preview.lines.len(), 512);
        assert_eq!(preview.contents, "line\n".repeat(512));
        assert!(preview.truncated);
        assert_eq!(preview.partial_line, None);
        std::fs::write(&path, format!("{}\nmore", "x".repeat(65535)))?;
        assert_eq!(super::preview(path.clone())?.partial_line, None);
        std::fs::write(&path, format!("{}{}", "line\n".repeat(512), "x".repeat(65536)))?;
        assert_eq!(super::preview(path.clone())?.partial_line, None);
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
