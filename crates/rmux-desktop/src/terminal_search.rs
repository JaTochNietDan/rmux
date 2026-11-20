//! Explicit, bounded searches of terminal snapshots; never scans on keystrokes.
use std::sync::atomic::{AtomicBool, Ordering};
use gpui::EntityId;

pub const MAX_TERMINALS: usize = 256;
pub const MAX_BYTES: usize = 64 * 1024;
static RUNNING: AtomicBool = AtomicBool::new(false);
pub struct Permit;
impl Permit {
    pub fn acquire() -> Option<Self> {
        RUNNING.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).ok().map(|_| Self)
    }
}
impl Drop for Permit { fn drop(&mut self) { RUNNING.store(false, Ordering::Release); } }
pub struct Job {
    pub workspace: EntityId,
    pub terminal: EntityId,
    pub name: String,
    pub read: rmux_terminal::session::HistoryExport,
}
pub struct Hit { pub workspace: EntityId, pub terminal: EntityId, pub name: String, pub snippet: String }
#[derive(Default)]
pub struct Report { pub hits: Vec<Hit>, pub scanned: usize, pub unavailable: usize, pub limited: usize }
pub fn validate(query: &str) -> Result<(), &'static str> {
    if query.trim().is_empty() || query.len() > 4096 || query.chars().any(char::is_control) {
        Err("Enter 1–4096 bytes of search text without control characters")
    } else { Ok(()) }
}
fn matching_line(text: &str, query: &str) -> Option<String> {
    // Match Ghostty Find: ASCII case-insensitive, other Unicode bytes exact.
    text.lines().find(|line| line.to_ascii_lowercase().contains(query))
        .map(|line| crate::file_browser::preview_line_title(line, query))
}
pub fn scan(jobs: Vec<Job>, query: &str, canceled: &AtomicBool) -> Report {
    let mut report = Report::default();
    let query = query.to_ascii_lowercase();
    for job in jobs.into_iter().take(MAX_TERMINALS) {
        if canceled.load(Ordering::Relaxed) { break; }
        match (job.read)() {
            Ok(Some((text, limited))) => {
                report.scanned += 1;
                report.limited += usize::from(limited);
                if let Some(snippet) = matching_line(&text, &query) {
                    report.hits.push(Hit { workspace: job.workspace, terminal: job.terminal, name: job.name, snippet });
                }
            }
            _ => report.unavailable += 1,
        }
    }
    report
}
#[cfg(test)]
mod tests {
    #[test]
    fn snapshots_report_partial_busy_and_canceled_work() {
        use std::sync::{Arc, atomic::{AtomicUsize, AtomicBool, Ordering}};
        let calls = Arc::new(AtomicUsize::new(0));
        let jobs = |count| (0..count).map(|index| {
            let calls = calls.clone();
            super::Job { workspace: gpui::EntityId::from(0x100000001), terminal: gpui::EntityId::from(0x100000002), name: index.to_string(),
                read: Box::new(move || { calls.fetch_add(1, Ordering::Relaxed); match index {
                    0 => Ok(Some(("NEEDLE 世界".into(), true))), 1 => Ok(None), _ => anyhow::bail!("closed"),
                } }) }
        }).collect();
        let report = super::scan(jobs(3), "needle", &AtomicBool::new(false));
        assert_eq!((report.scanned, report.limited, report.unavailable, report.hits.len()), (1, 1, 2, 1));
        assert_eq!(report.hits[0].name, "0");
        calls.store(0, Ordering::Relaxed);
        assert!(super::scan(jobs(3), "needle", &AtomicBool::new(true)).hits.is_empty());
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        super::scan(jobs(300), "needle", &AtomicBool::new(false));
        assert_eq!(calls.load(Ordering::Relaxed), super::MAX_TERMINALS);
    }
    #[test]
    fn literal_search_matches_find_case_rules_and_bounds_snippets() {
        assert!(super::matching_line("before\nBUILD 世界\nafter", "build 世界").unwrap().contains("BUILD 世界"));
        assert!(super::matching_line("CAFÉ", "café").is_none());
        assert!(super::matching_line("a.b", "a.b").is_some());
        assert!(super::matching_line("axb", "a.b").is_none());
        let line = format!("{}needle{}", "x".repeat(3000), "y".repeat(3000));
        let snippet = super::matching_line(&line, "needle").unwrap();
        assert!(snippet.contains("needle") && snippet.chars().count() <= 1026);
        for query in ["", "  ", "a\nb", "\0"] { assert!(super::validate(query).is_err()); }
        assert!(super::validate(&"x".repeat(4097)).is_err());
    }
}
