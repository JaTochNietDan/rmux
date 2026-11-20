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
                    let result = (|| -> Result<Progress> {
                        if search.is_none() {
                            search = Some(Search::new_cancellable(terminal.clone(), &canceled)?);
                        }
                        let search = search.as_mut().expect("initialized search");
                        if let Some(query) = query {
                            navigation = 0;
                            search.set_query(&query)?;
                        }
                        navigation = (navigation + movement).clamp(-64, 64);
                        if refresh {
                            search.refresh()?;
                        }
                        let mut progress = search.step()?;
                        // Resolve pending navigation once the current snapshot has
                        // been scanned, rather than dropping Enter during a search.
                        if !progress.pending && navigation != 0 {
                            let direction = if navigation > 0 {
                                Direction::Older
                            } else {
                                Direction::Newer
                            };
                            navigation -= navigation.signum();
                            progress = search.select(direction)?;
                        }
                        Ok(progress)
                    })();
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

    pub fn set_query(&self, query: String) -> Result<u64> {
        ensure!(query.len() <= 4096, "search query exceeds 4096 bytes");
        let revision = {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.revision = pending.revision.wrapping_add(1);
            pending.query = Some(query);
            pending.navigation = 0;
            pending.refresh = false;
            pending.revision
        };
        let _ = self.wake.try_send(());
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
}
