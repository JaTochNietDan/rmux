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
    pub window: Option<gpui::AnyWindowHandle>,
    pub workspace: EntityId,
    pub terminal: EntityId,
    pub name: String,
    pub read: rmux_terminal::session::HistoryExport,
    pub probe: rmux_terminal::session::HistoryProbe,
}
pub struct Hit { pub window: Option<gpui::AnyWindowHandle>, pub workspace: EntityId, pub terminal: EntityId, pub name: String, pub snippet: String }
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    for job in jobs.into_iter().take(MAX_TERMINALS) {
        if canceled.load(Ordering::Relaxed) { break; }
        if std::time::Instant::now() >= deadline { report.limited += 1; continue; }
        let mut needs_probe = false;
        let mut snippet = None;
        match (job.read)() {
            Ok(Some((text, limited))) => {
                report.scanned += 1;
                snippet = matching_line(&text, &query);
                needs_probe = limited && snippet.is_none();
            }
            Ok(None) => { report.scanned += 1; needs_probe = true; }
            _ => report.unavailable += 1,
        }
        if needs_probe {
            let terminal_deadline = deadline.min(std::time::Instant::now() + std::time::Duration::from_millis(250));
            match (job.probe)(&query, canceled, terminal_deadline) {
                Ok(Some(true)) => snippet = Some("Match in retained history · Enter opens Find".into()),
                Ok(Some(false)) => {},
                Ok(None) => report.limited += 1,
                Err(_) => report.unavailable += 1,
            }
        }
        if let Some(snippet) = snippet {
            report.hits.push(Hit { window: job.window, workspace: job.workspace, terminal: job.terminal, name: job.name, snippet });
        }
    }
    report
}
#[cfg(test)]
mod tests {
    #[test]
    fn truncated_misses_probe_history_and_report_unfinished_scans() {
        let jobs = (0..3).map(|index| super::Job {
            window: None, workspace: gpui::EntityId::from(0x100000001), terminal: gpui::EntityId::from(0x100000002), name: index.to_string(),
            read: Box::new(|| Ok(Some(("ordinary prefix".into(), true)))),
            probe: Box::new(move |query, _, _| {
                assert_eq!(query, "needle");
                Ok(match index { 0 => Some(true), 1 => Some(false), _ => None })
            }),
        }).collect();
        let report = super::scan(jobs, "NEEDLE", &super::AtomicBool::new(false));
        assert_eq!((report.scanned, report.limited, report.unavailable, report.hits.len()), (3, 1, 0, 1));
        assert_eq!(report.hits[0].name, "0");
        assert!(report.hits[0].snippet.contains("retained history"));
    }
    #[test]
    fn snapshots_report_partial_busy_and_canceled_work() {
        use std::sync::{Arc, atomic::{AtomicUsize, AtomicBool, Ordering}};
        let calls = Arc::new(AtomicUsize::new(0));
        let jobs = |count| (0..count).map(|index| {
            let calls = calls.clone();
            super::Job { window: None, workspace: gpui::EntityId::from(0x100000001), terminal: gpui::EntityId::from(0x100000002), name: index.to_string(),
                probe: Box::new(|_, _, _| Ok(None)),
                read: Box::new(move || { calls.fetch_add(1, Ordering::Relaxed); match index {
                    0 => Ok(Some(("NEEDLE 世界".into(), true))), 1 => Ok(None), _ => anyhow::bail!("closed"),
                } }) }
        }).collect();
        let report = super::scan(jobs(3), "needle", &AtomicBool::new(false));
        assert_eq!((report.scanned, report.limited, report.unavailable, report.hits.len()), (2, 1, 1, 1));
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
