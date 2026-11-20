//! Bounded per-terminal history, with read state independent of bell/status alerts.
use std::{collections::VecDeque, sync::Arc};
use rmux_terminal::{DesktopNotification, NotificationVisibility};
pub struct Record { pub message: Arc<DesktopNotification>, pub unread: bool }
#[derive(Default)]
pub struct History { records: VecDeque<Record>, revision: u64 }
impl History {
    pub fn push(&mut self, title: String, body: String, unread: bool) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        if self.records.len() == 32 { self.records.pop_front(); }
        self.records.push_back(Record { message: Arc::new(DesktopNotification {
            revision: self.revision, title, body, visibility: NotificationVisibility::Always,
        }), unread });
        self.revision
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Record> { self.records.iter() }
    pub fn latest(&self) -> Option<&Arc<DesktopNotification>> { self.records.back().map(|record| &record.message) }
    pub fn contains(&self, revision: u64) -> bool { self.records.iter().any(|r| r.message.revision == revision) }
    pub fn dismiss(&mut self, revision: u64) -> bool {
        let Some(index) = self.records.iter().position(|r| r.message.revision == revision) else { return false; };
        self.records.remove(index); true
    }
    pub fn unread(&self) -> bool { self.records.iter().any(|r| r.unread) }
    pub fn mark_read(&mut self) { for record in &mut self.records { record.unread = false; } }
    pub fn clear(&mut self) { self.records.clear(); }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_dismissal_preserves_other_unread_messages() {
        let mut history = History::default();
        let first = history.push("first".into(), String::new(), true);
        let second = history.push("second".into(), String::new(), true);
        assert!(history.dismiss(first));
        assert!(history.unread() && history.contains(second));
        assert!(!history.dismiss(first));
        history.mark_read(); assert!(!history.unread());
        for _ in 0..40 { history.push(String::new(), String::new(), false); }
        assert_eq!(history.iter().count(), 32);
        assert!(!history.contains(second));
        assert_eq!(history.latest().unwrap().revision, 42);
    }
}
