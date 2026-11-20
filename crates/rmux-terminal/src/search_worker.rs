//! One on-demand worker per open find bar, with coalesced commands and results.
use crate::search::{Direction, Progress, Search};
use anyhow::{Result, ensure};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub revision: u64,
    pub result: std::result::Result<Progress, String>,
}

#[derive(Default)]
struct Pending {
    revision: u64,
    query: Option<String>,
    refresh: bool,
    navigation: i32,
}

pub struct SearchWorker {
    pending: Arc<Mutex<Pending>>,
    latest: Arc<Mutex<Option<Update>>>,
    canceled: Arc<AtomicBool>,
    wake: async_channel::Sender<()>,
    pub updates: async_channel::Receiver<()>,
}

impl SearchWorker {
    pub(crate) fn new(terminal: Arc<Mutex<crate::Terminal>>) -> Result<Self> {
        let (wake, commands) = async_channel::bounded(1);
        let (notify, updates) = async_channel::bounded(1);
        let pending = Arc::new(Mutex::new(Pending::default()));
        let latest = Arc::new(Mutex::new(None));
        let canceled = Arc::new(AtomicBool::new(false));
        let this = Self {
            pending: pending.clone(),
            latest: latest.clone(),
            canceled: canceled.clone(),
            wake,
            updates,
        };
        std::thread::Builder::new()
            .name("rmux-search".into())
            .spawn(move || {
                // Registration takes the parser lock, so defer it until work
                // reaches this worker rather than blocking the UI opening Find.
                let mut search = None;
                let mut busy = false;
                let mut navigation = 0i32;
                loop {
                    if !busy && commands.recv_blocking().is_err() {
                        break;
                    }
                    if canceled.load(Ordering::Acquire) {
                        break;
                    }
                    let _ = commands.try_recv();
                    let (revision, query, refresh, movement) = {
                        let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            pending.revision,
                            pending.query.take(),
                            std::mem::take(&mut pending.refresh),
                            std::mem::take(&mut pending.navigation),
                        )
                    };
                    let result = (|| -> Result<Option<Progress>> {
                        if search.is_none() {
                            search = Some(Search::new_cancellable(terminal.clone(), &canceled)?);
                        }
                        let search = search.as_mut().expect("initialized search");
                        // Initialization can wait behind output while the user
                        // edits again. Do not scan or select the obsolete query.
                        if pending.lock().unwrap_or_else(|e| e.into_inner()).revision != revision {
                            return search.progress().map(Some);
                        }
                        if let Some(query) = query {
                            navigation = 0;
                            if !search.try_set_query(&query)? {
                                let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
                                if pending.revision == revision {
                                    pending.query = Some(query);
                                    pending.refresh |= refresh;
                                    pending.navigation = (pending.navigation + movement).clamp(-64, 64);
                                }
                                return Ok(None);
                            }
                        }
                        navigation = (navigation + movement).clamp(-64, 64);
                        if refresh {
                            if search.try_refresh()?.is_none() {
                                pending.lock().unwrap_or_else(|e| e.into_inner()).refresh = true;
                                return Ok(None);
                            }
                        }
                        let Some(mut progress) = search.try_step()? else { return Ok(None); };
                        // Navigation queued while scanning applies to that result
                        // set. Do not carry it into later output if nothing matched.
                        if !progress.pending && progress.matches == 0 { navigation = 0; }
                        // Resolve pending navigation once the current snapshot has
                        // been scanned, rather than dropping Enter during a search.
                        if !progress.pending && navigation != 0
                            && !canceled.load(Ordering::Acquire)
                            && pending.lock().unwrap_or_else(|e| e.into_inner()).revision == revision
                        {
                            let direction = if navigation > 0 {
                                Direction::Older
                            } else {
                                Direction::Newer
                            };
                            let Some(selected) = search.try_select_if(direction, || {
                                !canceled.load(Ordering::Acquire)
                                    && pending.lock().unwrap_or_else(|e| e.into_inner()).revision == revision
                            })? else { return Ok(None); };
                            navigation -= navigation.signum();
                            progress = selected;
                        }
                        Ok(Some(progress))
                    })();
                    let result = match result {
                        Ok(Some(progress)) => Ok(progress),
                        Ok(None) => {
                            if canceled.load(Ordering::Acquire) { break; }
                            busy = true;
                            std::thread::sleep(std::time::Duration::from_millis(1));
                            continue;
                        }
                        Err(error) => Err(error),
                    };
                    busy = result
                        .as_ref()
                        .is_ok_and(|progress| progress.pending || navigation != 0);
                    if canceled.load(Ordering::Acquire) {
                        break;
                    }
                    let update = Update {
                        revision,
                        result: result.map_err(|error| error.to_string()),
                    };
                    // Never publish an older query's progress after a newer query
                    // was submitted. The UI also checks revision when reading.
                    if pending.lock().unwrap_or_else(|e| e.into_inner()).revision == revision {
                        let mut latest = latest.lock().unwrap_or_else(|e| e.into_inner());
                        if latest.as_ref() != Some(&update) {
                            *latest = Some(update);
                            let _ = notify.try_send(());
                        }
                    }
                    if busy {
                        std::thread::yield_now();
                    }
                }
                // Search drops here, releasing tracked references and its terminal Arc.
            })?;
        Ok(this)
    }

    /// Every edit supersedes prior work. Invalid input schedules an empty query
    /// before returning its error, so old results/navigation cannot remain current.
    pub fn set_query(&self, query: String) -> Result<u64> {
        let valid = query.len() <= 4096;
        let revision = {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.revision = pending.revision.wrapping_add(1);
            pending.query = Some(if valid { query } else { String::new() });
            pending.navigation = 0;
            pending.refresh = false;
            pending.revision
        };
        let _ = self.wake.try_send(());
        ensure!(valid, "search query exceeds 4096 bytes");
        Ok(revision)
    }

    pub fn refresh(&self) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .refresh = true;
        let _ = self.wake.try_send(());
    }

    pub fn navigate(&self, direction: Direction) {
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.navigation = (pending.navigation
                + match direction {
                    Direction::Older => 1,
                    Direction::Newer => -1,
                })
            .clamp(-64, 64);
        }
        let _ = self.wake.try_send(());
    }

    pub fn latest(&self) -> Option<Update> {
        let pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        self.latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .filter(|update| update.revision == pending.revision)
            .cloned()
    }
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        self.wake.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Terminal;
    use std::time::{Duration, Instant};

    fn wait_for(worker: &SearchWorker, revision: u64, selected: bool) -> Update {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(update) = worker.latest() {
                if update.revision == revision
                    && update
                        .result
                        .as_ref()
                        .is_ok_and(|p| !p.pending && (!selected || p.selected.is_some()))
                {
                    return update;
                }
                assert!(update.result.is_ok(), "{update:?}");
            }
            assert!(Instant::now() < deadline, "search worker did not finish");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn rejected_query_supersedes_results_and_pending_navigation() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        terminal.lock().unwrap().write(b"needle needle\r\n");
        let worker = SearchWorker::new(terminal.clone())?;
        let initial = worker.set_query("needle".into())?;
        assert_eq!(wait_for(&worker, initial, false).result.unwrap().matches, 2);
        // Hold parsing to make the supersession observable before clear executes.
        let guard = terminal.lock().unwrap();
        worker.navigate(Direction::Older);
        assert!(worker.set_query("x".repeat(4097)).is_err());
        assert!(worker.latest().is_none());
        let revision = worker.pending.lock().unwrap().revision;
        assert_ne!(revision, initial);
        assert_eq!(worker.pending.lock().unwrap().navigation, 0);
        drop(guard);
        let cleared = wait_for(&worker, revision, false).result.unwrap();
        assert_eq!(cleared.matches, 0);
        assert_eq!(cleared.selected, None);
        assert!(terminal.lock().unwrap().selected_text()?.is_none());
        let next = worker.set_query("needle".into())?;
        worker.navigate(Direction::Older);
        assert_eq!(wait_for(&worker, next, true).result.unwrap().matches, 2);
        Ok(())
    }

    #[test]
    fn opening_and_queuing_search_does_not_wait_for_parser() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        let guard = terminal.lock().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        let owned = terminal.clone();
        let creator = std::thread::spawn(move || {
            let worker = SearchWorker::new(owned).unwrap();
            let revision = worker.set_query("needle".into()).unwrap();
            send.send((worker, revision)).unwrap();
        });
        let result = receive.recv_timeout(Duration::from_secs(1));
        // Always release the parser before asserting, including on failure.
        drop(guard);
        creator.join().unwrap();
        let (worker, revision) = result.expect("opening Find waited for the parser lock");
        assert_eq!(wait_for(&worker, revision, false).result.unwrap().matches, 0);
        Ok(())
    }

    #[test]
    fn closing_find_cancels_registration_while_parser_is_locked() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        let guard = terminal.lock().unwrap();
        let worker = SearchWorker::new(terminal.clone())?;
        worker.set_query("needle".into())?;
        // Wait until the worker consumes the query, so cancellation exercises
        // initialization rather than merely closing an idle command channel.
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.pending.lock().unwrap().query.is_some() {
            assert!(Instant::now() < deadline, "worker did not consume query");
            std::thread::sleep(Duration::from_millis(1));
        }
        let updates = worker.updates.clone();
        drop(worker);
        while !updates.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        let stopped_while_locked = updates.is_closed();
        drop(guard);
        assert!(stopped_while_locked, "canceled search waited for the parser");
        assert!(updates.try_recv().is_err(), "canceled query published a result");
        Ok(())
    }

    #[test]
    fn superseded_initial_query_does_not_install_a_selection() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        let mut guard = terminal.lock().unwrap();
        guard.write(b"needle");
        let worker = SearchWorker::new(terminal.clone())?;
        worker.set_query("needle".into())?;
        worker.navigate(Direction::Older);
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.pending.lock().unwrap().query.is_some() {
            assert!(Instant::now() < deadline, "worker did not consume initial query");
            std::thread::sleep(Duration::from_millis(1));
        }
        let revision = worker.set_query("missing".into())?;
        drop(guard);
        assert_eq!(wait_for(&worker, revision, false).result.unwrap().matches, 0);
        assert_eq!(terminal.lock().unwrap().selected_text()?, None,
            "superseded navigation changed the terminal selection");
        Ok(())
    }

    #[test]
    fn empty_results_discard_navigation_before_later_output() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        let guard = terminal.lock().unwrap();
        let worker = SearchWorker::new(terminal.clone())?;
        let revision = worker.set_query("needle".into())?;
        for _ in 0..64 { worker.navigate(Direction::Older); }
        drop(guard);
        assert_eq!(wait_for(&worker, revision, false).result.unwrap().matches, 0);
        terminal.lock().unwrap().write(b"needle");
        worker.refresh();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(update) = worker.latest() {
                let progress = update.result.map_err(anyhow::Error::msg)?;
                if !progress.pending && progress.matches == 1 {
                    assert_eq!(progress.selected, None);
                    break;
                }
            }
            assert!(Instant::now() < deadline, "refreshed search did not complete");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(terminal.lock().unwrap().selected_text()?.is_none());
        worker.navigate(Direction::Older);
        assert_eq!(wait_for(&worker, revision, true).result.unwrap().selected, Some(0));
        Ok(())
    }

    #[test]
    fn latest_query_wins_refresh_and_navigation_are_delivered() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        terminal.lock().unwrap().write(b"alpha beta beta\r\n");
        let worker = SearchWorker::new(terminal.clone())?;
        worker.set_query("alpha".into())?;
        let revision = worker.set_query("beta".into())?;
        worker.navigate(Direction::Older);
        assert_eq!(wait_for(&worker, revision, true).result.unwrap().matches, 2);
        assert_eq!(
            terminal.lock().unwrap().selected_text()?.as_deref(),
            Some("beta")
        );
        terminal.lock().unwrap().write(b"beta\r\n");
        worker.refresh();
        let deadline = Instant::now() + Duration::from_secs(3);
        while worker
            .latest()
            .and_then(|u| u.result.ok())
            .is_none_or(|p| p.matches != 3)
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let updates = worker.updates.clone();
        drop(worker);
        while !updates.is_closed() {
            assert!(Instant::now() < deadline, "worker failed to stop");
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn busy_query_replacement_preserves_latest_navigation() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(30, 3)?));
        terminal.lock().unwrap().write(b"alpha beta needle\r\n");
        let worker = SearchWorker::new(terminal.clone())?;
        let initial = worker.set_query("alpha".into())?;
        assert_eq!(wait_for(&worker, initial, false).result.unwrap().matches, 1);
        let guard = terminal.lock().unwrap();
        worker.set_query("beta".into())?;
        worker.navigate(Direction::Older);
        // Allow repeated busy retries before superseding the blocked query.
        std::thread::sleep(Duration::from_millis(10));
        let revision = worker.set_query("needle".into())?;
        worker.refresh();
        worker.navigate(Direction::Older);
        drop(guard);
        let progress = wait_for(&worker, revision, true).result.unwrap();
        assert_eq!(progress.matches, 1);
        assert_eq!(terminal.lock().unwrap().selected_text()?.as_deref(), Some("needle"));
        Ok(())
    }
}
