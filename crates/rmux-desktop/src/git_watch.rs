//! One native watcher per application, shared by all terminals in a repository.
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock, mpsc}};
use notify::{Event, RecursiveMode, Watcher};

type Subscribers = HashMap<PathBuf, HashMap<u64, async_channel::Sender<()>>>;
enum Command {
    Add(PathBuf, async_channel::Sender<()>, mpsc::Sender<Option<u64>>),
    Remove(PathBuf, u64),
}
pub struct Subscription {
    receiver: async_channel::Receiver<()>,
    path: PathBuf,
    id: u64,
    control: mpsc::Sender<Command>,
}
impl Subscription {
    pub async fn changed(&self) -> Result<(), async_channel::RecvError> {
        self.receiver.recv().await
    }
}
impl Drop for Subscription {
    fn drop(&mut self) { let _ = self.control.send(Command::Remove(self.path.clone(), self.id)); }
}
fn relevant(event: &Event, directory: &Path) -> bool {
    !matches!(event.kind, notify::EventKind::Access(_)) && event.paths.iter().any(|path| {
        path == directory || (path.parent() == Some(directory) && path.file_name().is_some_and(|name| name == "HEAD"))
    })
}
fn service() -> Option<&'static mpsc::Sender<Command>> {
    static SERVICE: OnceLock<Option<mpsc::Sender<Command>>> = OnceLock::new();
    SERVICE.get_or_init(|| {
        let (send, receive) = mpsc::channel();
        std::thread::Builder::new().name("rmux-git-watch".into()).spawn(move || {
            let subscribers = Arc::new(Mutex::new(Subscribers::new()));
            let events = subscribers.clone();
            let Ok(mut watcher) = notify::recommended_watcher(move |event: notify::Result<Event>| {
                let Ok(event) = event else { return; };
                let Ok(subscribers) = events.lock() else { return; };
                for (directory, listeners) in subscribers.iter() {
                    if relevant(&event, directory) {
                        for listener in listeners.values() { let _ = listener.try_send(()); }
                    }
                }
            }) else { return; };
            let mut next_id = 0;
            while let Ok(command) = receive.recv() {
                match command {
                    Command::Add(path, listener, reply) => {
                        let first = !subscribers.lock().unwrap().contains_key(&path);
                        if first && watcher.watch(&path, RecursiveMode::NonRecursive).is_err() {
                            let _ = reply.send(None); continue;
                        }
                        next_id += 1;
                        subscribers.lock().unwrap().entry(path.clone()).or_default().insert(next_id, listener);
                        if reply.send(Some(next_id)).is_err() {
                            let mut entries = subscribers.lock().unwrap();
                            let listeners = entries.get_mut(&path).unwrap();
                            listeners.remove(&next_id);
                            if listeners.is_empty() { entries.remove(&path); drop(entries); let _ = watcher.unwatch(&path); }
                        }
                    }
                    Command::Remove(path, id) => {
                        let mut entries = subscribers.lock().unwrap();
                        if let Some(listeners) = entries.get_mut(&path) {
                            listeners.remove(&id);
                            if listeners.is_empty() { entries.remove(&path); drop(entries); let _ = watcher.unwatch(&path); }
                        }
                    }
                }
            }
        }).ok()?;
        Some(send)
    }).as_ref()
}
/// Called on a background executor. Drop only queues removal; never blocks the UI.
pub fn subscribe(path: &Path) -> Option<Subscription> {
    let control = service()?.clone();
    let (sender, receiver) = async_channel::bounded(1);
    let (reply, result) = mpsc::channel();
    control.send(Command::Add(path.to_owned(), sender, reply)).ok()?;
    let id = result.recv().ok()??;
    Some(Subscription { receiver, path: path.to_owned(), id, control })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_head_changes_reach_shared_subscribers() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap();
        std::fs::write(directory.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let first = subscribe(&directory).expect("native watcher");
        let second = subscribe(&directory).expect("shared native watcher");
        let replace = |branch: &str| {
            std::fs::write(directory.join("HEAD.lock"), format!("ref: refs/heads/{branch}\n")).unwrap();
            std::fs::rename(directory.join("HEAD.lock"), directory.join("HEAD")).unwrap();
        };
        let receive = |subscription: &Subscription| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if subscription.receiver.try_recv().is_ok() { break; }
                assert!(std::time::Instant::now() < deadline, "missing HEAD notification");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        replace("first");
        receive(&first);
        receive(&second);
        drop(first);
        // Synchronize with the control queue so removal has completed.
        let third = subscribe(&directory).unwrap();
        while second.receiver.try_recv().is_ok() {}
        replace("second");
        receive(&second);
        receive(&third);
    }
    #[test]
    fn head_events_include_atomic_replacement_but_ignore_reads_and_other_files() {
        let directory = Path::new("/repo/.git");
        let event = Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::Both)))
            .add_path(directory.join("HEAD.lock")).add_path(directory.join("HEAD"));
        assert!(relevant(&event, directory));
        assert!(!relevant(&event, Path::new("/other/.git")));
        assert!(!relevant(&Event::new(notify::EventKind::Access(notify::event::AccessKind::Read)).add_path(directory.join("HEAD")), directory));
        assert!(!relevant(&Event::new(notify::EventKind::Any).add_path(directory.join("index")), directory));
    }
}
