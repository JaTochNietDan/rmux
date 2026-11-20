//! Incremental search. Feed briefly locks the terminal; ticks scan copied data.
use super::*;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub matches: usize,
    /// Zero-based index, ordered newest to oldest.
    pub selected: Option<usize>,
    pub pending: bool,
}

#[derive(Clone, Copy)]
pub enum Direction {
    Older,
    Newer,
}

pub struct Search {
    handle: GhosttySearch,
    terminal: Arc<Mutex<Terminal>>,
}

// SAFETY: the search has one owner. All operations that access its bound
// terminal take the same mutex as the PTY parser. Ticks only read copied data.
unsafe impl Send for Search {}

impl Search {
    pub(crate) fn new(terminal: Arc<Mutex<Terminal>>) -> Result<Self> {
        let handle = {
            let guard = terminal.lock().unwrap_or_else(|e| e.into_inner());
            Self::register(&guard)?
        };
        Ok(Self { handle, terminal })
    }

    /// Only the worker uses this path. Closing Find must cancel registration
    /// even while a busy parser owns the terminal; no tracked refs exist yet.
    pub(crate) fn new_cancellable(
        terminal: Arc<Mutex<Terminal>>,
        canceled: &std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        use std::sync::{TryLockError, atomic::Ordering};
        let handle = loop {
            ensure!(!canceled.load(Ordering::Acquire), "search canceled");
            let guard = match terminal.try_lock() {
                Ok(guard) => guard,
                Err(TryLockError::Poisoned(error)) => error.into_inner(),
                Err(TryLockError::WouldBlock) => {
                    // Back off only during initialization contention; completed
                    // searches still sleep on their command channel at idle.
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
            };
            ensure!(!canceled.load(Ordering::Acquire), "search canceled");
            break Self::register(&guard)?;
        };
        Ok(Self { handle, terminal })
    }

    fn register(terminal: &Terminal) -> Result<GhosttySearch> {
        let mut handle = ptr::null_mut();
        // SAFETY: callers hold the parser mutex for registration.
        check(unsafe { ghostty_search_new(ptr::null(), &mut handle, terminal.terminal) })?;
        Ok(handle)
    }

    /// Ghostty currently matches ASCII case-insensitively and other bytes exactly.
    /// An empty query releases the results and leaves the search idle.
    pub fn set_query(&mut self, query: &str) -> Result<()> {
        ensure!(query.len() <= 4096, "search query exceeds 4096 bytes");
        let _terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
        let value = GhosttyString {
            ptr: query.as_ptr(),
            len: query.len(),
        };
        // SAFETY: Ghostty copies the query; dropping old tracked refs is locked.
        unsafe {
            check(ghostty_search_set(
                self.handle,
                GHOSTTY_SEARCH_OPT_NEEDLE,
                input(&value),
            ))
        }
    }

    /// Replace a query only if the parser is available. False leaves the current
    /// query intact, so callers can retry or supersede it without blocking.
    pub fn try_set_query(&mut self, query: &str) -> Result<bool> {
        ensure!(query.len() <= 4096, "search query exceeds 4096 bytes");
        let _terminal = match self.terminal.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(false),
        };
        let value = GhosttyString { ptr: query.as_ptr(), len: query.len() };
        // SAFETY: this owner holds the parser lock; Ghostty copies query bytes.
        check(unsafe { ghostty_search_set(self.handle, GHOSTTY_SEARCH_OPT_NEEDLE, input(&value)) })?;
        Ok(true)
    }

    /// Refresh after output, resize, viewport movement or alternate-screen changes.
    /// No periodic polling is needed when the terminal and query are unchanged.
    pub fn refresh(&mut self) -> Result<Progress> {
        {
            let _terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: feed is bounded and serialized with all terminal access.
            unsafe {
                check(ghostty_search_feed(self.handle))?;
            }
        }
        self.progress()
    }

    /// Feed without waiting behind terminal output. None means retry later;
    /// it does not change selection or move the viewport.
    pub fn try_refresh(&mut self) -> Result<Option<Progress>> {
        {
            let _terminal = match self.terminal.try_lock() {
                Ok(guard) => guard,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            };
            // SAFETY: the feed is serialized with all terminal access.
            check(unsafe { ghostty_search_feed(self.handle) })?;
        }
        self.progress().map(Some)
    }

    /// One incremental scan tick, with opportunistic access when a feed is needed.
    pub fn try_step(&mut self) -> Result<Option<Progress>> {
        let mut status = self.status()?;
        if status == GHOSTTY_SEARCH_STATUS_FEED_REQUIRED {
            if self.try_refresh()?.is_none() { return Ok(None); }
            status = self.status()?;
        }
        if status == GHOSTTY_SEARCH_STATUS_RUNNING {
            // SAFETY: only copied search-owned data is scanned by this owner.
            check(unsafe { ghostty_search_tick(self.handle, &mut status) })?;
        }
        self.progress().map(Some)
    }

    /// Do at most one bounded feed and one bounded scan tick. Callers should yield
    /// between steps and cancel/drop superseded searches rather than run to completion
    /// on the UI thread. The scan itself does not hold the terminal mutex.
    pub fn step(&mut self) -> Result<Progress> {
        let mut status = self.status()?;
        if status == GHOSTTY_SEARCH_STATUS_FEED_REQUIRED {
            self.refresh()?;
            status = self.status()?;
        }
        if status == GHOSTTY_SEARCH_STATUS_RUNNING {
            // SAFETY: the handle is exclusively owned; ticks never read the terminal.
            unsafe {
                check(ghostty_search_tick(self.handle, &mut status))?;
            }
        }
        self.progress()
    }

    pub fn select(&mut self, direction: Direction) -> Result<Progress> {
        self.select_if(direction, || true)
    }

    /// Recheck supersession after acquiring the parser, before moving selection.
    pub(crate) fn select_if(&mut self, direction: Direction, current: impl FnOnce() -> bool) -> Result<Progress> {
        Ok(self.select_inner(direction, current, false)?.expect("blocking selection acquires the parser"))
    }

    /// None leaves navigation queued when the parser is busy. Supersession is
    /// checked after acquisition, immediately before changing selection.
    pub(crate) fn try_select_if(&mut self, direction: Direction, current: impl FnOnce() -> bool) -> Result<Option<Progress>> {
        self.select_inner(direction, current, true)
    }

    fn select_inner(&mut self, direction: Direction, current: impl FnOnce() -> bool, nonblocking: bool) -> Result<Option<Progress>> {
        let progress = self.progress()?;
        if progress.matches == 0 { return Ok(Some(progress)); }
        {
            let mut terminal = if nonblocking {
                match self.terminal.try_lock() {
                    Ok(guard) => guard,
                    Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
                }
            } else { self.terminal.lock().unwrap_or_else(|e| e.into_inner()) };
            if !current() { return self.progress().map(Some); }
            // SAFETY: navigation, snapshot extraction and selection installation
            // are one transaction with no intervening terminal mutations.
            unsafe {
                let result = match direction {
                    Direction::Older => ghostty_search_select_next(self.handle),
                    Direction::Newer => ghostty_search_select_prev(self.handle),
                };
                if result == GHOSTTY_SUCCESS {
                    terminal.revision = terminal.revision.wrapping_add(1);
                    let mut selection = GhosttySelection::default();
                    check(ghostty_search_get(
                        self.handle,
                        GHOSTTY_SEARCH_DATA_SELECTED_MATCH,
                        out(&mut selection),
                    ))?;
                    check(ghostty_terminal_set(
                        terminal.terminal,
                        GHOSTTY_TERMINAL_OPT_SELECTION,
                        input(&selection),
                    ))?;
                } else if result != GHOSTTY_NO_VALUE {
                    check(result)?;
                }
            }
        }
        self.progress().map(Some)
    }

    fn status(&self) -> Result<GhosttySearchStatus> {
        let mut status = GHOSTTY_SEARCH_STATUS_COMPLETE;
        // SAFETY: status and count/index getters read only search-owned state.
        unsafe {
            check(ghostty_search_get(
                self.handle,
                GHOSTTY_SEARCH_DATA_STATUS,
                out(&mut status),
            ))?;
        }
        Ok(status)
    }

    pub fn progress(&self) -> Result<Progress> {
        let mut matches = 0usize;
        let mut selected = 0usize;
        unsafe {
            check(ghostty_search_get(
                self.handle,
                GHOSTTY_SEARCH_DATA_TOTAL_MATCHES,
                out(&mut matches),
            ))?;
            let result = ghostty_search_get(
                self.handle,
                GHOSTTY_SEARCH_DATA_SELECTED_INDEX,
                out(&mut selected),
            );
            if result != GHOSTTY_NO_VALUE {
                check(result)?;
            }
            Ok(Progress {
                matches,
                selected: (result == GHOSTTY_SUCCESS).then_some(selected),
                pending: self.status()? != GHOSTTY_SEARCH_STATUS_COMPLETE,
            })
        }
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        let _terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: the Arc keeps the terminal alive while tracked refs are released.
        unsafe {
            ghostty_search_free(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finish(search: &mut Search) -> Result<Progress> {
        for _ in 0..1000 {
            let progress = search.step()?;
            if !progress.pending {
                return Ok(progress);
            }
        }
        anyhow::bail!("small search fixture failed to converge")
    }

    #[test]
    fn opportunistic_feed_yields_to_parser_and_resumes_without_selection_changes() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(20, 3)?));
        terminal.lock().unwrap().write(b"needle\r\nother\r\nneedle");
        terminal.lock().unwrap().select_all()?;
        let mut search = Search::new(terminal.clone())?;
        search.set_query("needle")?;
        let mut guard = terminal.lock().unwrap();
        let before = guard.frame()?;
        let selected = guard.selected_text()?;
        assert!(!search.try_set_query("missing")?);
        assert!(search.try_refresh()?.is_none());
        assert!(search.try_step()?.is_none());
        drop(guard);
        assert!(search.try_set_query("needle")?);
        let mut complete = false;
        for _ in 0..1000 {
            if let Some(progress) = search.try_step()? {
                if !progress.pending { assert_eq!(progress.matches, 2); complete = true; break; }
            }
        }
        assert!(complete);
        let mut guard = terminal.lock().unwrap();
        let after = guard.frame()?;
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.scrollback_offset, before.scrollback_offset);
        assert_eq!(guard.selected_text()?, selected);
        drop(guard);
        Ok(())
    }

    #[test]
    fn no_match_navigation_preserves_revision_selection_and_viewport() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(20, 3)?));
        terminal.lock().unwrap().write(b"keep selected text");
        terminal.lock().unwrap().select_all()?;
        let mut search = Search::new(terminal.clone())?;
        for query in ["absent", ""] {
            search.set_query(query)?;
            assert_eq!(finish(&mut search)?.matches, 0);
            let before = terminal.lock().unwrap().frame()?;
            let selection = terminal.lock().unwrap().selected_text()?;
            for direction in [Direction::Older, Direction::Newer] {
                assert_eq!(search.select(direction)?.matches, 0);
            }
            let after = terminal.lock().unwrap().frame()?;
            assert_eq!(after.revision, before.revision);
            assert_eq!(after.scrollback_offset, before.scrollback_offset);
            assert_eq!(terminal.lock().unwrap().selected_text()?, selection);
        }
        Ok(())
    }

    #[test]
    fn busy_navigation_waits_without_consuming_or_applying_stale_selection() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(20, 3)?));
        terminal.lock().unwrap().write(b"needle one\r\nneedle two");
        let mut search = Search::new(terminal.clone())?;
        search.set_query("needle")?;
        assert_eq!(finish(&mut search)?.matches, 2);
        let mut guard = terminal.lock().unwrap();
        let before = guard.frame()?;
        assert!(search.try_select_if(Direction::Older, || true)?.is_none());
        assert!(guard.selected_text()?.is_none());
        assert_eq!(guard.frame()?.revision, before.revision);
        drop(guard);
        let stale = search.try_select_if(Direction::Older, || false)?.unwrap();
        assert_eq!(stale.selected, None);
        let selected = search.try_select_if(Direction::Older, || true)?.unwrap();
        assert_eq!(selected.selected, Some(0));
        assert_eq!(terminal.lock().unwrap().selected_text()?.as_deref(), Some("needle"));
        Ok(())
    }

    #[test]
    fn searches_history_navigates_and_refreshes_after_output() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(20, 3)?));
        {
            let mut terminal = terminal.lock().unwrap();
            for line in 0..30 {
                terminal.write(format!("row-{line} Needle\r\n").as_bytes());
            }
        }
        let mut search = Search::new(terminal.clone())?;
        search.set_query("needle")?;
        assert_eq!(finish(&mut search)?.matches, 30);
        let first = search.select(Direction::Older)?;
        assert!(first.selected.is_some());
        assert_eq!(
            terminal.lock().unwrap().selected_text()?.as_deref(),
            Some("Needle")
        );
        let second = search.select(Direction::Older)?;
        assert_ne!(first.selected, second.selected);
        assert_eq!(search.select(Direction::Newer)?.selected, first.selected);
        terminal.lock().unwrap().write(b"another needle\r\n");
        search.refresh()?;
        assert_eq!(finish(&mut search)?.matches, 31);
        search.set_query("")?;
        assert_eq!(
            finish(&mut search)?,
            Progress {
                matches: 0,
                selected: None,
                pending: false
            }
        );
        Ok(())
    }

    #[test]
    fn unicode_soft_wrap_and_alternate_screen_search() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(8, 3)?));
        terminal
            .lock()
            .unwrap()
            .write("prefix 世界 suffix".as_bytes());
        let mut search = Search::new(terminal.clone())?;
        search.set_query("世界 suffix")?;
        assert_eq!(finish(&mut search)?.matches, 1);
        search.select(Direction::Older)?;
        assert_eq!(
            terminal.lock().unwrap().selected_text()?.as_deref(),
            Some("世界 suffix")
        );
        terminal.lock().unwrap().write(b"\x1b[?1049hother screen");
        search.refresh()?;
        assert_eq!(finish(&mut search)?.matches, 0);
        terminal.lock().unwrap().write(b"\x1b[?1049l");
        search.refresh()?;
        assert_eq!(finish(&mut search)?.matches, 1);
        Ok(())
    }
}
