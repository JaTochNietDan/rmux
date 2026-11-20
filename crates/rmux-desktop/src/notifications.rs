//! Bounded per-terminal history, with read state independent of bell/status alerts.
use std::{collections::VecDeque, sync::{Arc, atomic::{AtomicU64, Ordering}}};
static ARRIVAL: AtomicU64 = AtomicU64::new(0);
use rmux_terminal::{DesktopNotification, NotificationVisibility};
pub struct Record { pub message: Arc<DesktopNotification>, pub unread: bool, pub arrival: u64 }
#[derive(Default)]
pub struct History { records: VecDeque<Record>, revision: u64, latest_unread: Option<(u64, Arc<DesktopNotification>)> }
impl History {
    pub fn push(&mut self, title: String, body: String, unread: bool) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        if self.records.len() == 32 {
            let removed = self.records.pop_front().unwrap();
            self.refresh_after_removal(&removed);
        }
        let message = Arc::new(DesktopNotification {
            revision: self.revision, title, body, visibility: NotificationVisibility::Always,
        });
        let arrival = ARRIVAL.fetch_add(1, Ordering::Relaxed);
        if unread { self.latest_unread = Some((arrival, message.clone())); }
        self.records.push_back(Record { message, unread, arrival });
        self.revision
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Record> { self.records.iter() }
    pub fn latest(&self) -> Option<&Arc<DesktopNotification>> { self.records.back().map(|record| &record.message) }
    pub fn contains(&self, revision: u64) -> bool { self.records.iter().any(|r| r.message.revision == revision) }
    pub fn dismiss(&mut self, revision: u64) -> bool {
        let Some(index) = self.records.iter().position(|r| r.message.revision == revision) else { return false; };
        let removed = self.records.remove(index).unwrap();
        self.refresh_after_removal(&removed);
        true
    }
    fn refresh_after_removal(&mut self, removed: &Record) {
        if self.latest_unread.as_ref().is_some_and(|(_, latest)| Arc::ptr_eq(latest, &removed.message)) {
            self.latest_unread = self.records.iter().rev().find(|record| record.unread).map(|record| (record.arrival, record.message.clone()));
        }
    }
    pub fn unread_preview(&self) -> Option<(u64, &Arc<DesktopNotification>)> {
        self.latest_unread.as_ref().map(|(arrival, message)| (*arrival, message))
    }
    pub fn unread(&self) -> bool { self.latest_unread.is_some() }
    pub fn mark_read(&mut self) {
        if self.latest_unread.take().is_some() {
            for record in &mut self.records { record.unread = false; }
        }
    }
    pub fn clear(&mut self) { self.records.clear(); self.latest_unread = None; }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_arrival_order_spans_terminals_and_survives_dismissal() {
        let mut first = History::default();
        let mut second = History::default();
        first.push("older".into(), String::new(), true);
        second.push("middle".into(), String::new(), true);
        let latest = first.push("newest".into(), String::new(), true);
        assert!(first.unread_preview().unwrap().0 > second.unread_preview().unwrap().0);
        first.dismiss(latest);
        assert!(first.unread_preview().unwrap().0 < second.unread_preview().unwrap().0);
        second.mark_read();
        assert!(second.unread_preview().is_none());
    }

    #[test]
    fn unread_preview_tracks_dismissal_read_and_eviction() {
        let mut history = History::default();
        let check = |history: &History| {
            let expected = history.iter().rev().find(|record| record.unread).map(|record| &record.message);
            assert_eq!(history.unread(), expected.is_some());
            assert_eq!(history.unread_preview().map(|(_, m)| m.revision), expected.map(|m| m.revision));
            if let (Some(actual), Some(expected)) = (history.unread_preview().map(|(_, m)| m), expected) { assert!(Arc::ptr_eq(actual, expected)); }
        };
        for i in 0..120 {
            let revision = history.push(i.to_string(), String::new(), i % 3 == 0);
            check(&history);
            if i % 5 == 0 { history.dismiss(revision); check(&history); }
            if i % 11 == 0 { history.mark_read(); check(&history); }
        }
        history.clear(); check(&history);
        history.push("unread".into(), String::new(), true);
        for _ in 0..32 { history.push(String::new(), String::new(), false); check(&history); }
        assert!(!history.unread());
    }

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
