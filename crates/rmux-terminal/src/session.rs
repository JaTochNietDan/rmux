//! PTY ownership and bounded, event-driven redraw notification.
use crate::{Frame, Terminal};
use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

pub struct Session {
    initial_directory: Option<std::path::PathBuf>,
    child_pid: Option<u32>,
    direct_command: bool,
    _shell_integration: Option<tempfile::TempDir>,
    terminal: Arc<Mutex<Terminal>>,
    master: Box<dyn MasterPty + Send>,
    writer: mpsc::SyncSender<InputMessage>,
    input_errors: async_channel::Receiver<String>,
    paste_pending: Arc<AtomicBool>,
    follow_output_pending: AtomicBool,
    shutdown: ShutdownHandle,
    pub updates: async_channel::Receiver<()>,
    size: (u16, u16),
    exit: Arc<Mutex<Option<String>>>,
    output_bytes: Arc<AtomicU64>,
}

/// A one-shot background read; creating it does not lock or retain the session.
pub type HistoryExport = Box<dyn FnOnce() -> Result<Option<(String, bool)>> + Send>;
pub type MemorySample = Box<dyn FnOnce() -> Result<Option<crate::MemoryUsage>> + Send>;

struct PastePermit(Arc<AtomicBool>);
impl PastePermit {
    fn acquire(pending: &Arc<AtomicBool>) -> Result<Self> {
        anyhow::ensure!(
            pending.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok(),
            "a paste is already pending for this terminal"
        );
        Ok(Self(pending.clone()))
    }
}
impl Drop for PastePermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

enum InputPayload {
    Bytes(Vec<u8>),
    Paste { text: String, bracketed: bool },
}
impl InputPayload {
    fn encode(self) -> Result<Vec<u8>> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            Self::Paste { text, bracketed } => crate::keyboard::encode_paste(&text, bracketed),
        }
    }
}
struct InputMessage {
    payload: InputPayload,
    // Held through write_all, and released on encoding, enqueue or write failure.
    _paste: Option<PastePermit>,
}
impl From<Vec<u8>> for InputMessage {
    fn from(bytes: Vec<u8>) -> Self {
        Self { payload: InputPayload::Bytes(bytes), _paste: None }
    }
}

/// Coherent metadata read under one engine lock. Unchanged titles allocate nothing;
/// a title retrieval error does not discard an independently delivered bell.
pub struct MetadataUpdate {
    pub render_delay: Option<std::time::Duration>,
    pub program_status_records: Option<Vec<crate::program_status::Record>>,
    pub program_status: Option<(u64, Option<crate::program_status::Summary>)>,
    pub progress: Option<crate::Progress>,
    pub bell_revision: u64,
    pub notifications: Vec<Arc<crate::DesktopNotification>>,
    pub title: Result<Option<(u64, String)>>,
    pub directory: Result<Option<(u64, Option<std::path::PathBuf>)>>,
}

/// Does not retain the PTY or terminal; safe to retain while waiting for cleanup.
#[derive(Clone)]
pub struct ShutdownHandle {
    exit: Arc<Mutex<Option<String>>>,
    completed: async_channel::Receiver<()>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    process_wakeup: crate::process_wakeup::ProcessWakeup,
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    killer: Arc<Mutex<Box<dyn portable_pty::ChildKiller + Send + Sync>>>,
}
impl ShutdownHandle {
    pub fn request(&self) -> Result<()> {
        if self
            .exit
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return Ok(());
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        self.process_wakeup.close()?;
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        self.killer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .kill()?;
        Ok(())
    }
    pub fn is_finished(&self) -> bool {
        self.completed.is_closed()
    }
    pub async fn wait(&self) {
        let _ = self.completed.recv().await;
    }
}
// If creating the reaper thread fails, its closure is dropped. Keep ownership
// armed until that thread starts so a partially constructed session cannot
// abandon a live child. This cleanup runs only on startup failure.
struct UnstartedChild(Option<Box<dyn portable_pty::Child + Send + Sync>>);
impl UnstartedChild {
    fn take(&mut self) -> Box<dyn portable_pty::Child + Send + Sync> {
        self.0.take().expect("owned child")
    }
}
impl Drop for UnstartedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Session {
    pub fn compressor(&self) -> crate::compression::Compressor {
        crate::compression::Compressor::new(Arc::downgrade(&self.terminal))
    }
    /// Create on demand; a search owns no PTY and keeps its terminal data alive
    /// only until the search worker is canceled/dropped.
    pub fn search(&self) -> Result<crate::search::Search> {
        crate::search::Search::new(self.terminal.clone())
    }
    pub fn search_worker(&self) -> Result<crate::search_worker::SearchWorker> {
        crate::search_worker::SearchWorker::new(self.terminal.clone())
    }
    pub fn spawn(command: CommandBuilder, columns: u16, rows: u16) -> Result<Self> {
        Self::spawn_with_scrollback(command, columns, rows, 4 * 1024 * 1024)
    }
    pub fn spawn_with_scrollback(
        command: CommandBuilder,
        columns: u16,
        rows: u16,
        scrollback_bytes: usize,
    ) -> Result<Self> {
        // portable-pty falls back to HOME for a missing Unix cwd. Reject an
        // invalid explicit target before allocating an engine or starting work.
        if let Some(directory) = command.get_cwd() {
            let path = std::path::Path::new(directory);
            let metadata = std::fs::metadata(path)
                .with_context(|| format!("working directory {}", path.display()))?;
            anyhow::ensure!(
                metadata.is_dir(),
                "working directory {} is not a directory",
                path.display()
            );
        }
        let initial_directory = command.get_cwd().map(std::path::absolute).transpose()?;
        let mut engine = Terminal::new(columns, rows)?;
        engine.set_scrollback_limit(scrollback_bytes)?;
        let terminal = Arc::new(Mutex::new(engine));
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut reader = pair.master.try_clone_reader()?;
        let mut pty_writer = pair.master.take_writer()?;
        let child = pair.slave.spawn_command(command).context("start process")?;
        let child_pid = child.process_id();
        let mut unstarted = UnstartedChild(Some(child));
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let killer = unstarted.0.as_ref().unwrap().clone_killer();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let process_wakeup = crate::process_wakeup::ProcessWakeup::new(
            unstarted
                .0
                .as_ref()
                .unwrap()
                .process_id()
                .context("missing child PID")?,
        )?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let reaper_wakeup = process_wakeup.clone();
        drop(pair.slave);
        let (notify, updates) = async_channel::bounded(1);
        let exit = Arc::new(Mutex::new(None));
        let child_exit = exit.clone();
        let exit_terminal = terminal.clone();
        let exit_notify = notify.clone();
        let read_terminal = terminal.clone();
        let output_bytes = Arc::new(AtomicU64::new(0));
        let read_bytes = output_bytes.clone();
        let (writer, writes) = mpsc::sync_channel::<InputMessage>(32);
        let (input_error, input_errors) = async_channel::bounded(1);
        let input_notify = notify.clone();
        let (completion, completed) = async_channel::bounded(1);
        let reply_writer = writer.clone();
        let session = Self {
            initial_directory,
            child_pid,
            direct_command: false,
            _shell_integration: None,
            terminal,
            master: pair.master,
            writer,
            input_errors,
            paste_pending: Arc::new(AtomicBool::new(false)),
            follow_output_pending: AtomicBool::new(false),
            shutdown: ShutdownHandle {
                exit: exit.clone(),
                completed,
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                killer: Arc::new(Mutex::new(killer)),
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                process_wakeup,
            },
            updates,
            size: (columns, rows),
            exit,
            output_bytes,
        };
        let reaper = thread::Builder::new()
            .name("rmux-pty-reap".into())
            .spawn(move || {
                let _completion = completion;
                let mut child = unstarted.take();
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if reaper_wakeup.wait().unwrap_or(true) {
                    // This owner alone may reap or terminate the child, avoiding
                    // delayed signals targeting a PID that has been reused.
                    if !matches!(child.try_wait(), Ok(Some(_))) {
                        let _ = child.kill();
                    }
                }
                let message = match child.wait() {
                    Ok(status) => match status.signal() {
                        Some(signal) => format!("Process exited: {signal}"),
                        None => format!("Process exited with status {}", status.exit_code()),
                    },
                    Err(error) => format!("Unable to wait for process: {error}"),
                };
                exit_terminal
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_program_status();
                *child_exit.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
                let _ = exit_notify.try_send(());
            });
        if let Err(error) = reaper {
            // The dropped closure's guard already terminated/reaped the child.
            // Avoid signaling that child's former PID again from Session::drop.
            *session.exit.lock().unwrap_or_else(|e| e.into_inner()) =
                Some("Shell startup failed".into());
            return Err(error.into());
        }
        thread::Builder::new()
            .name("rmux-pty-write".into())
            .spawn(move || {
                while let Ok(message) = writes.recv() {
                    let bytes = match message.payload.encode() {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            let _ = input_error.try_send(format!("Paste encoding failed: {error}"));
                            let _ = input_notify.try_send(());
                            continue;
                        }
                    };
                    if let Err(error) = pty_writer.write_all(&bytes) {
                        let _ = input_error.try_send(format!("Terminal input failed: {error}"));
                        let _ = input_notify.try_send(());
                        break;
                    }
                }
            })?;
        thread::Builder::new()
            .name("rmux-pty-read".into())
            .spawn(move || {
                let mut bytes = [0; 16 * 1024];
                loop {
                    match reader.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(count) => {
                            let replies = {
                                let mut terminal =
                                    read_terminal.lock().unwrap_or_else(|e| e.into_inner());
                                terminal.write(&bytes[..count]);
                                terminal.take_replies()
                            };
                            read_bytes.fetch_add(count as u64, Ordering::Relaxed);
                            if !replies.is_empty() {
                                if reply_writer.send(replies.into()).is_err() {
                                    break;
                                }
                            }
                            // Coalesce notifications, never terminal bytes. No idle timer.
                            let _ = notify.try_send(());
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                read_terminal
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_program_status();
                let _ = notify.try_send(());
            })?;
        Ok(session)
    }
    pub fn shell(columns: u16, rows: u16) -> Result<Self> {
        Self::shell_in(columns, rows, None)
    }
    pub fn shell_in(columns: u16, rows: u16, directory: Option<&std::path::Path>) -> Result<Self> {
        Self::shell_in_with_scrollback(columns, rows, directory, 4 * 1024 * 1024)
    }
    pub fn shell_in_with_scrollback(
        columns: u16,
        rows: u16,
        directory: Option<&std::path::Path>,
        scrollback_bytes: usize,
    ) -> Result<Self> {
        let shell = select_shell(
            std::env::var_os("RMUX_SHELL"),
            std::env::var_os(if cfg!(windows) { "COMSPEC" } else { "SHELL" }),
        )?;
        let mut command = CommandBuilder::new(&shell);
        if let Some(directory) = directory {
            command.cwd(directory);
        }
        let login = std::env::var_os("RMUX_SHELL_LOGIN").as_deref() != Some(std::ffi::OsStr::new("0"));
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "rmux");
        #[cfg(unix)]
        let integration = if std::path::Path::new(&shell).file_name()
            == Some(std::ffi::OsStr::new("zsh"))
            && std::env::var_os("RMUX_SHELL_INTEGRATION").as_deref()
                != Some(std::ffi::OsStr::new("0"))
        {
            Some(crate::shell_integration::prepare_zsh(
                &mut command,
                std::env::var_os("ZDOTDIR").as_deref(),
            )?)
        } else if !login && std::path::Path::new(&shell).file_name() == Some(std::ffi::OsStr::new("bash"))
            && std::env::var_os("RMUX_SHELL_INTEGRATION").as_deref() != Some(std::ffi::OsStr::new("0")) {
            Some(crate::shell_integration::prepare_bash(&mut command)?)
        } else {
            None
        };
        #[cfg(not(unix))]
        let integration = None;
        if !cfg!(windows) {
            command.arg("-i");
            if login { command.arg("-l"); }
        }
        let mut session = Self::spawn_with_scrollback(command, columns, rows, scrollback_bytes)?;
        session._shell_integration = integration;
        Ok(session)
    }
    /// Execute an argv vector directly; no shell quoting or command evaluation.
    pub fn command_in(
        argv: &[std::ffi::OsString],
        columns: u16,
        rows: u16,
        directory: Option<&std::path::Path>,
    ) -> Result<Self> {
        Self::command_in_with_scrollback(argv, columns, rows, directory, 4 * 1024 * 1024)
    }
    pub fn command_in_with_scrollback(
        argv: &[std::ffi::OsString],
        columns: u16,
        rows: u16,
        directory: Option<&std::path::Path>,
        scrollback_bytes: usize,
    ) -> Result<Self> {
        Self::command_in_with_environment(argv, columns, rows, directory, scrollback_bytes, &[])
    }
    pub fn command_in_with_environment(
        argv: &[std::ffi::OsString], columns: u16, rows: u16,
        directory: Option<&std::path::Path>, scrollback_bytes: usize,
        environment: &[(std::ffi::OsString, std::ffi::OsString)],
    ) -> Result<Self> {
        let program = argv
            .first()
            .filter(|program| !program.is_empty())
            .ok_or_else(|| anyhow::anyhow!("command requires a nonempty program name"))?;
        let mut command = CommandBuilder::new(program);
        command.args(&argv[1..]);
        if let Some(directory) = directory {
            command.cwd(directory);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "rmux");
        for (name, value) in environment { command.env(name, value); }
        let mut session = Self::spawn_with_scrollback(command, columns, rows, scrollback_bytes)?;
        session.direct_command = true;
        Ok(session)
    }
    pub fn current_directory(&self) -> Result<Option<std::path::PathBuf>> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .current_directory()
    }
    /// Explicit spawn directory, without consulting the terminal parser.
    pub fn initial_directory(&self) -> Option<&std::path::Path> {
        self.initial_directory.as_deref()
    }
    /// A predictable idle workload, used only by the opt-in performance harness.
    pub fn benchmark_shell(columns: u16, rows: u16) -> Result<Self> {
        #[cfg(unix)]
        let mut command = CommandBuilder::new("/bin/sh");
        #[cfg(windows)]
        let mut command = CommandBuilder::new("cmd.exe");
        #[cfg(unix)]
        {
            command.arg("-i");
            command.env("ENV", "/dev/null");
            command.env("PS1", "$ ");
        }
        #[cfg(windows)]
        command.arg("/d");
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        Self::spawn(command, columns, rows)
    }
    pub fn input(&self, bytes: &[u8]) -> Result<()> {
        self.write_input(bytes, true)
    }
    /// Targeted automation input preserves the viewport and never locks the engine.
    pub fn input_without_scroll(&self, bytes: &[u8]) -> Result<()> {
        self.write_input(bytes, false)
    }
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown.clone()
    }
    fn write_input(&self, bytes: &[u8], follow_output: bool) -> Result<()> {
        anyhow::ensure!(self.exit_status().is_none(), "process has exited");
        // Backpressure is explicit rather than blocking keyboard dispatch. Each
        // accepted message is delivered in order by the sole writer thread.
        anyhow::ensure!(
            bytes.len() <= 64 * 1024,
            "input exceeds the prototype's 64 KiB limit"
        );
        self.writer
            .try_send(bytes.to_vec().into())
            .map_err(|_| anyhow::anyhow!("terminal input queue is full or closed"))?;
        if follow_output && !bytes.is_empty() {
            self.follow_output()?;
        }
        Ok(())
    }
    pub fn mouse(&self, event: crate::mouse::Event) -> Result<bool> {
        let bytes = self
            .terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .encode_mouse(event)?;
        if let Some(bytes) = bytes {
            if !bytes.is_empty() {
                self.write_input(&bytes, false)?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// Input delivery must not wait behind output parsing just to move the viewport.
    /// A busy parser defers this coalesced request until the next frame acquisition.
    fn follow_output(&self) -> Result<()> {
        self.follow_output_pending.store(true, Ordering::Release);
        let mut terminal = match self.terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(()),
        };
        self.apply_pending_follow(&mut terminal)
    }
    fn apply_pending_follow(&self, terminal: &mut Terminal) -> Result<()> {
        if self.follow_output_pending.swap(false, Ordering::AcqRel) {
            terminal.scroll(crate::Scroll::Bottom)?;
        }
        Ok(())
    }
    pub fn scroll(&self, scroll: crate::Scroll) -> Result<()> {
        let mut terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
        // Explicit navigation after input supersedes a deferred follow request.
        self.follow_output_pending.store(false, Ordering::Release);
        terminal.scroll(scroll)
    }
    pub fn focus(&self, focused: bool) -> Result<()> {
        let bytes = {
            let mut terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
            if focused {
                terminal.acknowledge_program_status();
            }
            terminal.encode_focus(focused)?
        };
        if let Some(bytes) = bytes {
            self.write_input(&bytes, false)?;
        }
        Ok(())
    }
    pub fn key(&self, key: &str, mods: crate::keyboard::Modifiers, repeat: bool) -> Result<bool> {
        self.key_with_text(key, mods, repeat, None)
    }
    pub fn key_with_text(
        &self,
        key: &str,
        mods: crate::keyboard::Modifiers,
        repeat: bool,
        text: Option<&str>,
    ) -> Result<bool> {
        let bytes = self
            .terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .encode_key_action_with_text(
                key,
                mods,
                if repeat {
                    crate::keyboard::Action::Repeat
                } else {
                    crate::keyboard::Action::Press
                },
                text,
            )?;
        if let Some(bytes) = bytes {
            self.input(&bytes)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn paste(&self, text: &str) -> Result<()> {
        anyhow::ensure!(text.len() <= 1024 * 1024, "paste exceeds 1 MiB input limit");
        self.paste_owned(text.to_owned())
    }
    pub fn take_input_error(&self) -> Option<String> {
        self.input_errors.try_recv().ok()
    }
    /// Reserve queue order before encoding on the sole PTY writer thread.
    pub fn paste_owned(&self, text: String) -> Result<()> {
        anyhow::ensure!(text.len() <= 1024 * 1024, "paste exceeds 1 MiB input limit");
        anyhow::ensure!(self.exit_status().is_none(), "process has exited");
        let permit = PastePermit::acquire(&self.paste_pending)?;
        let bracketed = {
            let terminal = match self.terminal.try_lock() {
                Ok(terminal) => terminal,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => anyhow::bail!("Terminal is busy; paste again to retry. No paste input was queued."),
            };
            terminal.bracketed_paste()?
        };
        self.writer
            .try_send(InputMessage { payload: InputPayload::Paste { text, bracketed }, _paste: Some(permit) })
            .map_err(|_| anyhow::anyhow!("terminal input queue is full or closed"))?;
        self.follow_output()
    }
    /// Bounded automation paste. Never waits for parser access or changes viewport.
    /// False means the parser is busy and no input was queued.
    pub fn try_paste_without_scroll(&self, text: &str) -> Result<bool> {
        anyhow::ensure!(text.len() <= 16 * 1024, "automation paste exceeds 16 KiB");
        anyhow::ensure!(self.exit_status().is_none(), "process has exited");
        let permit = PastePermit::acquire(&self.paste_pending)?;
        let bracketed = {
            let terminal = match self.terminal.try_lock() {
                Ok(terminal) => terminal,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Ok(false),
            };
            terminal.bracketed_paste()?
        };
        let bytes = crate::keyboard::encode_paste(text, bracketed)?;
        self.writer.try_send(InputMessage { payload: InputPayload::Bytes(bytes), _paste: Some(permit) })
            .map_err(|_| anyhow::anyhow!("terminal input queue is full or closed"))?;
        Ok(true)
    }

    /// Atomically enqueue a key press/release pair without parser waits or scrolling.
    pub fn try_key_tap_without_scroll(&self, key: &str, mods: crate::keyboard::Modifiers) -> Result<Option<usize>> {
        anyhow::ensure!(self.exit_status().is_none(), "process has exited");
        let bytes = {
            let mut terminal = match self.terminal.try_lock() {
                Ok(terminal) => terminal,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            };
            terminal.encode_key_tap(key, mods)?
        };
        let count = bytes.len();
        self.input_without_scroll(&bytes)?;
        Ok(Some(count))
    }

    pub fn key_release(&self, key: &str, mods: crate::keyboard::Modifiers) -> Result<()> {
        let bytes = self
            .terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .encode_key_action(key, mods, crate::keyboard::Action::Release)?;
        if let Some(bytes) = bytes.filter(|bytes| !bytes.is_empty()) {
            // Releasing a key must not pull a user out of scrollback.
            self.write_input(&bytes, false)?;
        }
        Ok(())
    }
    pub fn frame(&self) -> Result<Frame> {
        let mut terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
        self.apply_pending_follow(&mut terminal)?;
        terminal.frame()
    }
    pub fn try_frame(&self) -> Result<Option<Frame>> {
        let mut terminal = match self.terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
        };
        self.apply_pending_follow(&mut terminal)?;
        terminal.frame().map(Some)
    }
    pub fn history_export(&self, max_bytes: usize) -> HistoryExport {
        self.text_export(max_bytes, false, true)
    }
    pub fn selection_export(&self, max_bytes: usize, unwrap: bool) -> HistoryExport {
        self.text_export(max_bytes, true, unwrap)
    }
    /// Bounded clipboard formatting for a background executor. Construction never
    /// acquires the parser lock or keeps a closed terminal alive.
    pub fn clipboard_export(&self, unwrap: bool, current: impl Fn() -> bool + Send + 'static) -> Box<dyn FnOnce() -> Result<Option<String>> + Send> {
        let terminal = Arc::downgrade(&self.terminal);
        Box::new(move || {
            if !current() { return Ok(None); }
            let terminal = terminal.upgrade().context("terminal closed before copying")?;
            let engine = loop {
                match terminal.try_lock() {
                    Ok(engine) => break engine,
                    Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        if !current() { return Ok(None); }
                        // Only pending copies poll; there is no idle timer.
                        thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            };
            // Another clipboard write may have superseded us while waiting.
            if !current() { return Ok(None); }
            engine.selected_text_with_wraps(16 * 1024 * 1024, unwrap)
        })
    }
    /// Deferred directory lookup without retaining the terminal while queued.
    pub fn directory_lookup(&self, current: impl Fn() -> bool + Send + 'static) -> Box<dyn FnOnce() -> Result<Option<std::path::PathBuf>> + Send> {
        let terminal = Arc::downgrade(&self.terminal);
        Box::new(move || {
            if !current() { return Ok(None); }
            let terminal = terminal.upgrade().context("terminal closed before directory lookup")?;
            let engine = loop {
                match terminal.try_lock() {
                    Ok(engine) => break engine,
                    Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        if !current() { return Ok(None); }
                        thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
            };
            if !current() { return Ok(None); }
            engine.current_directory()
        })
    }
    fn text_export(&self, max_bytes: usize, selected: bool, unwrap: bool) -> HistoryExport {
        let terminal = Arc::downgrade(&self.terminal);
        Box::new(move || {
            let terminal = terminal.upgrade().context("terminal closed before history export")?;
            let engine = match terminal.try_lock() {
                Ok(engine) => engine,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            };
            if selected { engine.selection_text_prefix_with_wraps(max_bytes, unwrap).map(|text| Some(text.unwrap_or_default())) }
            else { engine.history_text_limited(max_bytes).map(Some) }
        })
    }
    pub fn selection_event(&self, event: crate::selection::Event) -> Result<bool> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .selection_event(event)
    }
    pub fn select_all(&self) -> Result<()> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .select_all()
    }
    pub fn clear_scrollback(&self) -> Result<()> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear_scrollback()
    }
    pub fn memory_usage(&self) -> Result<crate::MemoryUsage> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .memory_usage()
    }
    /// Create a one-shot background sample without retaining the engine.
    pub fn memory_sample(&self) -> MemorySample {
        let terminal = Arc::downgrade(&self.terminal);
        Box::new(move || {
            let terminal = terminal.upgrade().context("terminal closed before memory sample")?;
            let engine = match terminal.try_lock() {
                Ok(engine) => engine,
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            };
            engine.memory_usage().map(Some)
        })
    }
    /// Opportunistic diagnostics must not wait behind parsing or engine work.
    pub fn try_memory_usage(&self) -> Result<Option<crate::MemoryUsage>> {
        let terminal = match self.terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
        };
        terminal.memory_usage().map(Some)
    }
    pub fn selected_text(&self) -> Result<Option<String>> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .selected_text()
    }
    /// A small single-line selection can seed Find without copying a large selection.
    pub fn selected_search_query(&self) -> Option<String> {
        // Find can fall back to its remembered query while output is being parsed.
        self.terminal
            .try_lock()
            .ok()?
            // Streaming stops at the limit; a size-probing formatter would
            // still walk the entire selection while holding the parser lock.
            .selection_text_prefix(4096)
            .ok()
            .flatten()
            .filter(|(_, truncated)| !truncated)
            .map(|(text, _)| text)
            .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
    }
    pub fn hyperlink_at(
        &self,
        column: u16,
        row: u32,
        displayed_revision: u64,
    ) -> Result<Option<String>> {
        let mut terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
        if terminal.revision != displayed_revision {
            return Ok(None);
        }
        terminal.link_at(column, row)
    }
    pub fn program_status_records(&self) -> Vec<crate::program_status::Record> {
        let terminal = self.terminal.lock().unwrap_or_else(|e| e.into_inner());
        let records = terminal.program_status();
        let mut snapshot: Vec<_> = records
            .iter()
            .map(|record| {
                let mut record = record.clone();
                record.app = records.effective_app(&record.id).unwrap_or("").to_owned();
                record
            })
            .collect();
        snapshot.sort_by(|left, right| left.id.cmp(&right.id));
        snapshot
    }
    pub fn set_default_colors(&self, colors: [Option<crate::Color>; 3]) -> Result<()> {
        self.terminal.lock().unwrap_or_else(|e| e.into_inner()).set_default_colors(colors)
    }
    pub fn set_ansi_palette(&self, colors: Option<[crate::Color; 16]>) -> Result<()> {
        self.terminal.lock().unwrap_or_else(|e| e.into_inner()).set_ansi_palette(colors)
    }
    pub fn set_scrollback_limit(&self, bytes: usize) -> Result<()> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_scrollback_limit(bytes)
    }
    /// Boolean query for close warnings without copying or sorting status text.
    pub fn reports_active_work(&self) -> bool {
        if self
            .exit
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return false;
        }
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .program_status()
            .has_active_work()
    }
    /// On-demand close-warning check. Unix job control identifies a foreground
    /// group distinct from the owning shell; directly launched programs are work
    /// for their entire lifetime. No process-table scans or polling are needed.
    pub fn has_active_work(&self) -> bool {
        if self
            .exit
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return false;
        }
        if self.direct_command {
            return true;
        }
        #[cfg(unix)]
        if let (Some(child), Some(group)) = (self.child_pid, self.master.process_group_leader()) {
            if group > 0 && group as u32 != child {
                return true;
            }
        }
        self.reports_active_work()
    }
    pub fn program_status_since(
        &self,
        revision: u64,
    ) -> Option<(u64, Option<crate::program_status::Summary>)> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .program_status()
            .summary_since(revision)
    }
    pub fn try_metadata_since(&self, title_revision: u64, directory_revision: u64, notification_revision: u64,
        program_status_revision: u64, include_records: bool) -> Option<MetadataUpdate> {
        let terminal = match self.terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return None,
        };
        let program_status = terminal.program_status().summary_since(program_status_revision);
        let program_status_records = if include_records && program_status.is_some() {
            let records = terminal.program_status();
            let mut snapshot: Vec<_> = records.iter().map(|record| {
                let mut record = record.clone();
                record.app = records.effective_app(&record.id).unwrap_or("").to_owned();
                record
            }).collect();
            snapshot.sort_by(|left, right| left.id.cmp(&right.id));
            Some(snapshot)
        } else { None };
        Some(MetadataUpdate {
            program_status,
            program_status_records,
            render_delay: terminal.render_delay(),
            progress: terminal.progress(),
            bell_revision: terminal.bell_revision(),
            notifications: terminal.notifications_since(notification_revision),
            title: terminal.title_since(title_revision),
            directory: terminal.directory_since(directory_revision),
        })
    }
    pub fn try_render_delay(&self) -> Option<Option<std::time::Duration>> {
        let terminal = match self.terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return None,
        };
        Some(terminal.render_delay())
    }
    pub fn render_delay(&self) -> Option<std::time::Duration> {
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .render_delay()
    }
    pub fn exit_status(&self) -> Option<String> {
        self.exit.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// Original PTY child identity, retained after exit; never a liveness check.
    pub fn process_id(&self) -> Option<u32> {
        self.child_pid
    }
    /// Reaper completion observed without acquiring the terminal or exit lock.
    pub fn process_exited(&self) -> bool {
        self.shutdown.is_finished()
    }
    /// PTY bytes parsed by the engine; excludes queued/unread kernel output.
    pub fn output_bytes(&self) -> u64 {
        self.output_bytes.load(Ordering::Relaxed)
    }
    /// Attempt a resize without waiting behind output parsing. None means no
    /// PTY or engine change occurred; callers should retry their latest size.
    /// Some(false) is unchanged; Some(true) applied both PTY and engine sizes.
    pub fn try_resize(&mut self, columns: u16, rows: u16) -> Result<Option<bool>> {
        if self.size == (columns, rows) { return Ok(Some(false)); }
        let mut terminal = match self.terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
        };
        self.master.resize(PtySize { rows, cols: columns, pixel_width: 0, pixel_height: 0 })?;
        terminal.resize(columns, rows)?;
        self.size = (columns, rows);
        Ok(Some(true))
    }

    pub fn resize(&mut self, columns: u16, rows: u16) -> Result<()> {
        if self.size == (columns, rows) {
            return Ok(());
        }
        self.master.resize(PtySize {
            rows,
            cols: columns,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        self.terminal
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .resize(columns, rows)?;
        self.size = (columns, rows);
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.shutdown.request();
    }
}

fn select_shell(
    explicit: Option<std::ffi::OsString>,
    inherited: Option<std::ffi::OsString>,
) -> Result<std::ffi::OsString> {
    if let Some(shell) = explicit {
        anyhow::ensure!(!shell.is_empty(), "RMUX_SHELL must name a shell executable");
        return Ok(shell);
    }
    Ok(inherited
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "cmd.exe".into()
            } else {
                "/bin/sh".into()
            }
        }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn shell_override_preserves_literal_paths_and_rejects_empty_override() -> anyhow::Result<()> {
        use super::select_shell;
        use std::ffi::OsString;
        let custom = OsString::from("/path with spaces/my-shell");
        assert_eq!(
            select_shell(Some(custom.clone()), Some("fallback".into()))?,
            custom
        );
        assert_eq!(
            select_shell(None, Some("inherited-shell".into()))?,
            OsString::from("inherited-shell")
        );
        assert!(select_shell(Some(OsString::new()), Some("fallback".into())).is_err());
        assert_eq!(
            select_shell(None, Some(OsString::new()))?,
            select_shell(None, None)?
        );
        Ok(())
    }
    use super::*;
    #[cfg(unix)]
    #[test]
    fn opportunistic_snapshot_and_targeted_input_do_not_wait_for_parser_lock() -> Result<()> {
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "printf READY; IFS= read -r line; printf '\\nRECEIVED:%s\\n' \"$line\""]);
        let session = Session::spawn(command, 40, 4)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !session.frame()?.text().contains("READY") {
            anyhow::ensure!(std::time::Instant::now() < deadline, "receiver did not start");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        let before = session.frame()?;
        let engine = session.terminal.clone();
        let (ready, started) = mpsc::sync_channel(1);
        let (release, finish) = mpsc::sync_channel(1);
        let holder = thread::spawn(move || {
            let mut engine = engine.lock().unwrap();
            engine.write(b"-pending\x1b]2;metadata-pending\x07");
            ready.send(()).unwrap();
            // Bound failure time if a regression accidentally waits on this lock.
            let _ = finish.recv_timeout(std::time::Duration::from_secs(1));
        });
        started.recv_timeout(std::time::Duration::from_secs(2))?;
        let started_at = std::time::Instant::now();
        let snapshot = session.try_frame()?;
        assert!(session.try_metadata_since(0, 0, 0, 0, true).is_none(), "busy parser must not block metadata");
        assert!(session.try_render_delay().is_none(), "busy parser must not block render delay");
        assert!(session.history_export(4096)()?.is_none(), "busy parser must return no history export");
        assert!(session.selection_export(4096, true)()?.is_none(), "busy parser must return no selection export");
        assert!(session.memory_sample()()?.is_none(), "busy parser must return no memory sample");
        let paste_error = session.paste("must-not-be-queued").unwrap_err();
        assert!(paste_error.to_string().contains("No paste input was queued"));
        assert!(!session.paste_pending.load(Ordering::Acquire), "rejected paste retained its permit");
        session.input_without_scroll("state-世界\n".as_bytes())?;
        let elapsed = started_at.elapsed();
        let _ = release.send(());
        holder.join().unwrap();
        assert!(snapshot.is_none(), "busy parser must return no snapshot");
        assert!(elapsed < std::time::Duration::from_millis(500), "opportunistic work waited behind parser: {elapsed:?}");
        assert!(!before.text().contains("pending"), "previous snapshot mutated");
        while !session.frame()?.text().contains("RECEIVED:state-世界") || session.exit_status().is_none() {
            anyhow::ensure!(std::time::Instant::now() < deadline, "queued input/output did not complete");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        let after = session.try_frame()?.expect("idle snapshot");
        let memory = thread::spawn(session.memory_sample()).join().unwrap()?.expect("idle memory sample");
        assert!(memory.primary_resident_bytes > 0);
        assert!(after.text().contains("pending"), "busy snapshot consumed pending changes");
        let metadata = session.try_metadata_since(0, 0, 0, 0, true).expect("idle metadata");
        assert_eq!(metadata.title?.unwrap().1, "metadata-pending");
        session.select_all()?;
        let expected = session.selected_text()?.unwrap();
        let owners = Arc::strong_count(&session.terminal);
        let export = session.selection_export(4096, true);
        assert_eq!(Arc::strong_count(&session.terminal), owners, "queued export retained the engine");
        let selected = thread::spawn(export).join().unwrap()?.expect("idle selection export");
        assert_eq!(selected, (expected.clone(), false));
        assert_eq!(session.selected_text()?, Some(expected));
        let expected = session.selected_text()?;
        let clipboard = session.clipboard_export(true, || true);
        assert_eq!(Arc::strong_count(&session.terminal), owners, "queued clipboard export retained the engine");
        assert_eq!(thread::spawn(clipboard).join().unwrap()?, expected);
        assert!(session.clipboard_export(true, || false)()?.is_none());
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = calls.clone();
        let superseded = session.clipboard_export(true, move || seen.fetch_add(1, Ordering::Relaxed) == 0);
        assert!(thread::spawn(superseded).join().unwrap()?.is_none());
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        assert_eq!(session.selected_text()?, expected, "cancellation changed selection");
        let engine = session.terminal.lock().unwrap();
        let canceled = Arc::new(AtomicBool::new(false));
        let canceled_copy = canceled.clone();
        let (started, starting) = mpsc::channel();
        let waiting = session.clipboard_export(true, move || {
            let _ = started.send(());
            !canceled_copy.load(Ordering::Relaxed)
        });
        let (finished, completion) = mpsc::channel();
        let worker = thread::spawn(move || { let _ = finished.send(waiting()); });
        starting.recv_timeout(std::time::Duration::from_secs(2))?;
        canceled.store(true, Ordering::Relaxed);
        // The parser remains locked: completion must not depend on releasing it.
        let result = completion.recv_timeout(std::time::Duration::from_secs(2));
        drop(engine);
        worker.join().unwrap();
        assert!(result??.is_none(), "obsolete copy waited for parser access");

        let directory = session.directory_lookup(|| true);
        assert_eq!(Arc::strong_count(&session.terminal), owners, "queued directory lookup retained the engine");
        assert_eq!(thread::spawn(directory).join().unwrap()?, session.current_directory()?);
        assert!(session.directory_lookup(|| false)()?.is_none());
        let engine = session.terminal.lock().unwrap_or_else(|error| error.into_inner());
        let canceled = Arc::new(AtomicBool::new(false));
        let token = canceled.clone();
        let (started, starting) = mpsc::channel();
        let lookup = session.directory_lookup(move || {
            let _ = started.send(());
            !token.load(Ordering::Relaxed)
        });
        let (finished, completion) = mpsc::channel();
        let worker = thread::spawn(move || { let _ = finished.send(lookup()); });
        starting.recv_timeout(std::time::Duration::from_secs(2))?;
        canceled.store(true, Ordering::Relaxed);
        let result = completion.recv_timeout(std::time::Duration::from_secs(2));
        drop(engine);
        worker.join().unwrap();
        assert!(result??.is_none(), "obsolete directory lookup waited for parser access");
        Ok(())
    }
    #[test]
    fn paste_permit_survives_queueing_and_recovers_from_rejection() -> Result<()> {
        let pending = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::sync_channel(1);
        sender.try_send(InputMessage {
            payload: InputPayload::Bytes(vec![1]),
            _paste: Some(PastePermit::acquire(&pending)?),
        }).ok().unwrap();
        assert!(PastePermit::acquire(&pending).is_err());
        let message = receiver.recv()?;
        assert!(PastePermit::acquire(&pending).is_err());
        drop(message);
        sender.try_send(vec![2].into()).ok().unwrap();
        let rejected = sender.try_send(InputMessage {
            payload: InputPayload::Bytes(vec![3]),
            _paste: Some(PastePermit::acquire(&pending)?),
        });
        assert!(rejected.is_err());
        drop(rejected);
        drop(PastePermit::acquire(&pending)?);
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn large_bracketed_paste_arrives_before_following_input() -> Result<()> {
        let root = tempfile::tempdir()?;
        let destination = root.path().join("received");
        let text = "界\nabc".repeat(20_000);
        let expected = format!("\x1b[200~{text}\x1b[201~AFTER");
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "stty raw -echo; printf '\\033[?2004hREADY'; dd bs=1 count=\"$2\" of=\"$1\" 2>/dev/null", "rmux-test"]);
        command.arg(&destination);
        command.arg(expected.len().to_string());
        let session = Session::spawn(command, 40, 4)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !session.frame()?.text().contains("READY") {
            anyhow::ensure!(std::time::Instant::now() < deadline, "paste receiver not ready");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        session.paste(&text)?;
        session.input(b"AFTER")?;
        while session.exit_status().is_none() {
            anyhow::ensure!(std::time::Instant::now() < deadline, "paste delivery timed out");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(std::fs::read(destination)?, expected.as_bytes());
        Ok(())
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn automation_paste_is_nonblocking_bracketed_and_preserves_viewport() -> Result<()> {
        let root = tempfile::tempdir()?;
        let destination = root.path().join("received");
        let text = "世界\nsecond line";
        let expected = format!("\x1b[200~{text}\x1b[201~AFTER");
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "stty raw -echo; printf '1\\n2\\n3\\n4\\n5\\n\\033[?2004hREADY'; dd bs=1 count=\"$2\" of=\"$1\" 2>/dev/null", "rmux-test"]);
        command.arg(&destination);
        command.arg(expected.len().to_string());
        let session = Session::spawn(command, 40, 3)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !session.frame()?.text().contains("READY") {
            anyhow::ensure!(std::time::Instant::now() < deadline, "paste receiver not ready");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        session.scroll(crate::Scroll::Top)?;
        let offset = session.frame()?.scrollback_offset;
        assert!(offset > 0);
        {
            let _parser = session.terminal.lock().unwrap();
            assert!(!session.try_paste_without_scroll("not sent")?);
            assert!(!session.paste_pending.load(Ordering::Acquire));
        }
        assert!(session.try_paste_without_scroll(text)?);
        session.input_without_scroll(b"AFTER")?;
        assert_eq!(session.frame()?.scrollback_offset, offset);
        while session.exit_status().is_none() {
            anyhow::ensure!(std::time::Instant::now() < deadline, "paste delivery timed out");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(std::fs::read(destination)?, expected.as_bytes());
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn input_defers_follow_while_parser_busy_and_navigation_supersedes_it() -> Result<()> {
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "stty -echo; printf '1\\n2\\n3\\n4\\n5\\nREADY'; read -r line"]);
        let session = Session::spawn(command, 40, 3)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !session.frame()?.text().contains("READY") {
            anyhow::ensure!(std::time::Instant::now() < deadline, "shell not ready");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        for explicit_navigation in [false, true] {
            session.scroll(crate::Scroll::Top)?;
            assert!(session.frame()?.scrollback_offset > 0);
            let terminal = session.terminal.clone();
            let (ready, waiting) = mpsc::channel();
            let (release, released) = mpsc::channel();
            let parser = thread::spawn(move || {
                let _guard = terminal.lock().unwrap();
                ready.send(()).unwrap();
                // Bounded even if input regresses to a blocking parser lock.
                let _ = released.recv_timeout(std::time::Duration::from_secs(2));
            });
            waiting.recv()?;
            let start = std::time::Instant::now();
            session.input(b"x")?;
            let elapsed = start.elapsed();
            let pending = session.follow_output_pending.load(Ordering::Acquire);
            let _ = release.send(());
            parser.join().unwrap();
            assert!(elapsed < std::time::Duration::from_millis(500), "input waited for parser: {elapsed:?}");
            assert!(pending);
            if explicit_navigation { session.scroll(crate::Scroll::Top)?; }
            let frame = session.try_frame()?.expect("parser released");
            assert_eq!(frame.scrollback_offset > 0, explicit_navigation);
            assert!(!session.follow_output_pending.load(Ordering::Acquire));
        }
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn closing_session_terminates_a_shell_that_ignores_hangup() -> Result<()> {
        let session = Session::spawn(
            shell_command("trap '' HUP; printf 'ready'; while :; do read line || exit; done"),
            40,
            4,
        )?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !session.frame()?.text().contains("ready") {
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "shell readiness timeout"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
        let exit = session.exit.clone();
        let terminal = Arc::downgrade(&session.terminal);
        let shutdown = session.shutdown_handle();
        let started = std::time::Instant::now();
        drop(session);
        // Termination's grace period must run on the reaper, not this caller.
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
        while exit.lock().unwrap().is_none()
            || terminal.upgrade().is_some()
            || !shutdown.is_finished()
        {
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "stubborn shell retained after close"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }
    #[test]
    fn dropping_sessions_releases_shell_and_terminal() -> Result<()> {
        for _ in 0..8 {
            let session = Session::benchmark_shell(40, 4)?;
            let terminal = Arc::downgrade(&session.terminal);
            let exit = session.exit.clone();
            drop(session);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while terminal.upgrade().is_some() || exit.lock().unwrap().is_none() {
                anyhow::ensure!(
                    std::time::Instant::now() < deadline,
                    "session retained its terminal or child after drop"
                );
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Ok(())
    }
    fn shell_command(script: &str) -> CommandBuilder {
        #[cfg(unix)]
        let mut command = CommandBuilder::new("/bin/sh");
        #[cfg(windows)]
        let mut command = CommandBuilder::new("cmd.exe");
        #[cfg(unix)]
        command.args(["-c", script]);
        #[cfg(windows)]
        command.args(["/d", "/c", script]);
        command
    }
    #[test]
    fn reports_exit_and_rejects_further_input() -> Result<()> {
        let command = shell_command("exit 7");
        let session = Session::spawn(command, 40, 4)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some(status) = session.exit_status() {
                assert_eq!(status, "Process exited with status 7");
                assert!(session.input(b"ignored").is_err());
                return Ok(());
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "shell exit deadline exceeded"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    #[cfg(unix)]
    #[test]
    fn close_detection_tracks_foreground_jobs_and_direct_commands() -> Result<()> {
        let session = Session::benchmark_shell(80, 8)?;
        let wait = |expected| -> Result<()> {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while session.has_active_work() != expected {
                anyhow::ensure!(
                    std::time::Instant::now() < deadline,
                    "foreground state did not become {expected}"
                );
                thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(())
        };
        wait(false)?;
        session.input(b"sleep 30\n")?;
        wait(true)?;
        session.input(b"\x03")?;
        wait(false)?;
        let argv = ["/bin/sleep", "30"].map(std::ffi::OsString::from);
        let direct = Session::command_in(&argv, 80, 8, None)?;
        assert!(direct.has_active_work());
        let shutdown = direct.shutdown_handle();
        drop(direct);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !shutdown.is_finished() {
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "direct process cleanup timed out"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn direct_command_preserves_arguments_directory_and_terminal_environment() -> Result<()> {
        let root = tempfile::tempdir()?;
        let root = root.path().canonicalize()?;
        let argv = [
            "/bin/sh",
            "-c",
            "printf '%s\\n' \"$1\" \"$2\" \"$PWD\" \"$TERM:$COLORTERM:$TERM_PROGRAM\"",
            "fixture",
            "a b 世界",
            "$(touch nope); *",
        ]
        .map(std::ffi::OsString::from);
        let session = Session::command_in(&argv, 160, 8, Some(&root))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let text = session.frame()?.text();
            if text.contains("xterm-256color:truecolor:rmux") {
                assert!(text.contains("a b 世界"));
                assert!(text.contains("$(touch nope); *"));
                assert!(text.contains(root.to_str().unwrap()));
                assert!(!root.join("nope").exists());
                break;
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "direct command output timed out: {text}"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(Session::command_in(&[], 80, 24, None).is_err());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn command_environment_is_literal_and_scoped_to_child() -> Result<()> {
        let before = std::env::var_os("RMUX_TEST_COMMAND_VALUE");
        let argv = ["/bin/sh", "-c", "printf '[%s][%s]done' \"$RMUX_TEST_COMMAND_VALUE\" \"$RMUX_TEST_COMMAND_EMPTY\""].map(std::ffi::OsString::from);
        let environment = [("RMUX_TEST_COMMAND_VALUE".into(), "世界 $(literal)".into()), ("RMUX_TEST_COMMAND_EMPTY".into(), "".into())];
        let session = Session::command_in_with_environment(&argv, 100, 4, None, 0, &environment)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let text = session.frame()?.text();
            if text.contains("done") { assert!(text.contains("[世界 $(literal)][]done")); break; }
            anyhow::ensure!(std::time::Instant::now() < deadline, "environment fixture timed out");
            thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(std::env::var_os("RMUX_TEST_COMMAND_VALUE"), before);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn configured_retention_applies_to_first_process_output() -> Result<()> {
        let argv = ["/bin/sh", "-c", "i=0; while [ $i -lt 100 ]; do printf 'startup output\\n'; i=$((i+1)); done; printf 'finished-marker'"]
            .map(std::ffi::OsString::from);
        for limit in [0, 1024 * 1024] {
            let session = Session::command_in_with_scrollback(&argv, 40, 3, None, limit)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            loop {
                let frame = session.frame()?;
                if frame.text().contains("finished-marker") {
                    assert_eq!(frame.scrollback_lines > 0, limit > 0);
                    break;
                }
                anyhow::ensure!(
                    std::time::Instant::now() < deadline,
                    "startup output timed out"
                );
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Ok(())
    }
    #[test]
    fn invalid_explicit_directory_is_rejected_before_process_start() -> Result<()> {
        let root = tempfile::tempdir()?;
        let file = root.path().join("file");
        std::fs::write(&file, b"not a directory")?;
        for path in [root.path().join("missing"), file] {
            let mut command = CommandBuilder::new("rmux-fixture-must-not-launch");
            command.cwd(&path);
            let error = match Session::spawn(command, 80, 24) {
                Ok(_) => panic!("invalid directory was accepted"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("working directory"), "{error:#}");
        }
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn opportunistic_resize_leaves_busy_pty_unchanged_and_applies_latest_size() -> Result<()> {
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "IFS= read -r line"]);
        let mut session = Session::spawn(command, 40, 4)?;
        let engine = session.terminal.clone();
        {
            let _guard = engine.lock().unwrap();
            assert_eq!(session.try_resize(40, 4)?, Some(false));
            assert_eq!(session.try_resize(117, 31)?, None);
            assert_eq!(session.master.get_size()?.cols, 40);
            assert_eq!(session.master.get_size()?.rows, 4);
        }
        assert_eq!(session.try_resize(120, 33)?, Some(true));
        assert_eq!(session.master.get_size()?.cols, 120);
        assert_eq!(session.master.get_size()?.rows, 33);
        let frame = session.frame()?;
        assert_eq!(frame.columns, 120);
        assert_eq!(frame.rows.len(), 33);
        assert_eq!(session.try_resize(120, 33)?, Some(false));
        session.input_without_scroll(b"done\n")?;
        Ok(())
    }

    #[test]
    fn interactive_shell_accepts_input_after_resize() -> Result<()> {
        let mut session = Session::benchmark_shell(80, 24)?;
        let _ = session.frame()?;
        session.resize(117, 31)?;
        // The expected marker is absent from the command itself, so PTY echo
        // alone cannot satisfy this check.
        #[cfg(unix)]
        session.input(b"printf '%s%s\\n' rmux- interactive-ok\r")?;
        #[cfg(windows)]
        session.input(b"echo rmux-interactive-^ok\r")?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if session.frame()?.text().contains("rmux-interactive-ok") {
                return Ok(());
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "interactive output deadline exceeded"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    #[test]
    fn real_shell_output_reaches_ghostty() -> Result<()> {
        let command = shell_command(if cfg!(windows) {
            "echo rmux-pty-ok"
        } else {
            r"printf '\033[32mrmux-pty-ok\033[0m\n'"
        });
        let session = Session::spawn(command, 40, 4)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if session.frame()?.text().contains("rmux-pty-ok") {
                return Ok(());
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "PTY output deadline exceeded"
            );
            thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
