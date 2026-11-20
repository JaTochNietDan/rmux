//! Order asynchronous terminal copies against every clipboard write in rmux.
use gpui::{App, ClipboardItem};
use std::sync::atomic::{AtomicU64, Ordering};
static GENERATION: AtomicU64 = AtomicU64::new(0);
pub fn begin() -> u64 { GENERATION.fetch_add(1, Ordering::Relaxed).wrapping_add(1) }
pub fn current(ticket: u64) -> bool { GENERATION.load(Ordering::Relaxed) == ticket }
pub fn write(item: ClipboardItem, cx: &mut App) {
    begin();
    cx.write_to_clipboard(item);
}

#[cfg(test)]
mod tests {
    #[test]
    fn newer_request_invalidates_previous_completion() {
        let older = super::begin();
        assert!(super::current(older));
        let newer = super::begin();
        assert!(!super::current(older));
        assert!(super::current(newer));
    }
}
