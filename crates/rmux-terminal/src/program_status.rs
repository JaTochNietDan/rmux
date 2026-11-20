//! Owned OSC 7501 records. Protocol parsing remains Ghostty's responsibility.
use crate::ffi::*;
use std::collections::VecDeque;
use std::ffi::c_void;

pub(crate) unsafe extern "C" fn report(
    _: GhosttyTerminal,
    userdata: *mut c_void,
    report: *const GhosttyTerminalProgramStatus,
) {
    if report.is_null() {
        return;
    }
    // SAFETY: synchronous Ghostty callback with borrowed strings and our stable
    // exclusively accessed userdata. Reject over-limit identity strings rather
    // than truncating them into another record's ID.
    unsafe {
        let report = &*report;
        let copy = |value: &GhosttyString, limit: usize| -> Option<String> {
            if value.len > limit {
                return None;
            }
            if value.len == 0 {
                return Some(String::new());
            }
            if value.ptr.is_null() {
                return None;
            }
            Some(
                String::from_utf8_lossy(std::slice::from_raw_parts(value.ptr, value.len))
                    .into_owned(),
            )
        };
        let Some(id) = copy(&report.id, 128) else {
            return;
        };
        let callbacks = &mut *userdata.cast::<super::Callbacks>();
        let records = &mut callbacks.program_status;
        if report.state == GHOSTTY_PROGRAM_STATUS_STATE_CLEAR {
            records.clear(&id);
            return;
        }
        let state = match report.state {
            GHOSTTY_PROGRAM_STATUS_STATE_IDLE => State::Idle,
            GHOSTTY_PROGRAM_STATUS_STATE_WORKING => State::Working,
            GHOSTTY_PROGRAM_STATUS_STATE_DONE => State::Done,
            GHOSTTY_PROGRAM_STATUS_STATE_BLOCKED => State::Blocked,
            GHOSTTY_PROGRAM_STATUS_STATE_ERROR => State::Error,
            _ => return,
        };
        // The reader can drain buffered bytes after the child reaper reports
        // exit. Preserve final done/error reports but never resurrect live work.
        if callbacks.program_finished && !matches!(state, State::Done | State::Error) {
            return;
        }
        let kind = match report.kind {
            GHOSTTY_PROGRAM_STATUS_KIND_PERMISSION => Kind::Permission,
            GHOSTTY_PROGRAM_STATUS_KIND_QUESTION => Kind::Question,
            GHOSTTY_PROGRAM_STATUS_KIND_AUTH => Kind::Auth,
            _ => Kind::None,
        };
        let (Some(app), Some(title), Some(message)) = (
            copy(&report.app, 32),
            copy(&report.title, 192),
            copy(&report.message, 2048),
        ) else {
            return;
        };
        let display = |text: String| {
            text.chars().filter(|ch| !ch.is_control() && !matches!(*ch, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')).collect()
        };
        records.put(Record {
            id,
            state,
            kind,
            progress: (0..=100)
                .contains(&report.progress)
                .then_some(report.progress as u8),
            app,
            title: display(title),
            message: display(message),
        });
    }
}

pub(crate) unsafe extern "C" fn prompt(
    _: GhosttyTerminal,
    userdata: *mut c_void,
    event: *const GhosttyTerminalSemanticPrompt,
) {
    // SAFETY: same callback ownership contract as report above.
    unsafe {
        if !event.is_null() && (*event).kind == GHOSTTY_SEMANTIC_PROMPT_PROMPT_START {
            (*userdata.cast::<super::Callbacks>())
                .program_status
                .finish_active();
        }
    }
}

pub const RECORD_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    None,
    Permission,
    Question,
    Auth,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: String,
    pub state: State,
    pub kind: Kind,
    pub progress: Option<u8>,
    pub app: String,
    pub title: String,
    pub message: String,
}

/// Oldest updated first, including repeated updates to an existing ID.
/// The protocol permits 64–256 records; we choose its lower memory bound.
#[derive(Default)]
pub struct Records {
    records: VecDeque<Record>,
    revision: u64,
}
#[derive(Clone, Debug)]
pub struct Summary {
    pub record: Record,
    pub count: usize,
}
impl Records {
    pub fn has_active_work(&self) -> bool {
        self.records
            .iter()
            .any(|record| matches!(record.state, State::Working | State::Blocked))
    }
    pub fn summary_since(&self, revision: u64) -> Option<(u64, Option<Summary>)> {
        if revision == self.revision {
            return None;
        }
        let selected = self.records.iter().max_by_key(|record| match record.state {
            State::Blocked => 5,
            State::Error => 4,
            State::Done => 3,
            State::Working => 2,
            State::Idle => 1,
        });
        let summary = selected.map(|selected| {
            let mut record = selected.clone();
            record.app = self.effective_app(&record.id).unwrap_or("").to_owned();
            Summary {
                record,
                count: self.records.len(),
            }
        });
        Some((self.revision, summary))
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Record> {
        self.records.iter()
    }

    pub fn put(&mut self, record: Record) {
        self.records.retain(|previous| previous.id != record.id);
        if self.records.len() == RECORD_LIMIT {
            self.records.pop_front();
        }
        self.records.push_back(record);
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn clear(&mut self, id: &str) {
        self.retain(|record| {
            !id.is_empty()
                && record.id != id
                && !record
                    .id
                    .strip_prefix(id)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        });
    }

    /// End transient state at a new shell prompt or process exit. Completed and
    /// failed work remains available until the user acknowledges the terminal.
    pub fn finish_active(&mut self) {
        self.retain(|record| matches!(record.state, State::Done | State::Error));
    }
    pub fn acknowledge(&mut self) {
        self.retain(|record| !matches!(record.state, State::Done | State::Error));
    }
    fn retain(&mut self, keep: impl FnMut(&Record) -> bool) {
        let previous = self.records.len();
        self.records.retain(keep);
        if previous != self.records.len() {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Resolve inheritance at read time so changing a parent affects existing
    /// children, including when an intermediate parent has no record.
    pub fn effective_app(&self, id: &str) -> Option<&str> {
        let mut candidate = id;
        loop {
            if let Some(record) = self
                .records
                .iter()
                .find(|record| record.id == candidate && !record.app.is_empty())
            {
                return Some(&record.app);
            }
            if candidate.is_empty() {
                return None;
            }
            candidate = candidate.rsplit_once('/').map_or("", |(parent, _)| parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str, state: State, app: &str) -> Record {
        Record {
            id: id.into(),
            state,
            kind: Kind::None,
            progress: None,
            app: app.into(),
            title: String::new(),
            message: String::new(),
        }
    }
    #[test]
    fn summary_prioritizes_attention_and_skips_unchanged_revisions() {
        let mut records = Records::default();
        assert!(records.summary_since(0).is_none());
        records.put(record("", State::Working, "build"));
        records.put(record("test", State::Blocked, ""));
        records.put(record("format", State::Working, ""));
        let (revision, summary) = records.summary_since(0).unwrap();
        let summary = summary.unwrap();
        assert_eq!(summary.record.id, "test");
        assert_eq!(summary.record.app, "build");
        assert_eq!(summary.count, 3);
        assert!(records.summary_since(revision).is_none());
        records.put(record("deploy", State::Blocked, ""));
        assert_eq!(
            records
                .summary_since(revision)
                .unwrap()
                .1
                .unwrap()
                .record
                .id,
            "deploy"
        );
        records.clear("");
        assert!(records.summary_since(revision).unwrap().1.is_none());
    }
    #[test]
    fn replacement_inheritance_and_subtree_clear() {
        let mut records = Records::default();
        records.put(record("", State::Working, "build"));
        records.put(record("test", State::Working, "runner"));
        records.put(record("test/unit/rust", State::Blocked, ""));
        records.put(record("testing", State::Idle, ""));
        assert_eq!(records.effective_app("test/unit/rust"), Some("runner"));
        records.put(record("test", State::Working, ""));
        assert_eq!(records.effective_app("test/unit/rust"), Some("build"));
        assert_eq!(records.iter().count(), 4);
        records.clear("test");
        assert_eq!(
            records.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["", "testing"]
        );
        let revision = records.revision();
        records.clear("missing");
        assert_eq!(records.revision(), revision);
        records.clear("");
        assert_eq!(records.iter().count(), 0);
    }
    #[test]
    fn prompt_exit_and_acknowledgement_preserve_the_right_states() {
        let mut records = Records::default();
        assert!(!records.has_active_work());
        for (id, state) in [
            ("idle", State::Idle),
            ("work", State::Working),
            ("wait", State::Blocked),
            ("done", State::Done),
            ("error", State::Error),
        ] {
            records.put(record(id, state, ""));
        }
        assert!(records.has_active_work());
        records.finish_active();
        assert!(!records.has_active_work());
        assert_eq!(
            records.iter().map(|r| r.state).collect::<Vec<_>>(),
            vec![State::Done, State::Error]
        );
        records.put(record("new", State::Working, ""));
        assert!(records.has_active_work());
        records.acknowledge();
        assert_eq!(
            records.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["new"]
        );
    }
    #[test]
    fn limit_evicts_least_recently_updated_record() {
        let mut records = Records::default();
        for id in 0..RECORD_LIMIT {
            records.put(record(&id.to_string(), State::Working, ""));
        }
        records.put(record("0", State::Done, ""));
        records.put(record("extra", State::Working, ""));
        assert_eq!(records.iter().count(), RECORD_LIMIT);
        assert!(records.iter().any(|r| r.id == "0"));
        assert!(!records.iter().any(|r| r.id == "1"));
    }
}
