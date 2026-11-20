//! Background-safe compression probe. Never waits behind active terminal work.
use crate::{Compression, Terminal};
use anyhow::Result;
use std::sync::{
    Arc, Mutex, TryLockError, Weak,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct Compressor {
    terminal: Weak<Mutex<Terminal>>,
    canceled: Arc<AtomicBool>,
}
pub struct Cancellation(Arc<AtomicBool>);
impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Gone,
    Busy,
    Activity(u64),
    Pending,
    Complete,
}
impl Compressor {
    pub(crate) fn new(terminal: Weak<Mutex<Terminal>>) -> Self {
        Self {
            terminal,
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }
    /// Keep this guard in the owning task, not in dispatched worker clones.
    pub fn cancel_on_drop(&self) -> Cancellation {
        Cancellation(self.canceled.clone())
    }
    /// A changed token requires a fresh idle delay before calling again.
    pub fn step(&self, idle_token: Option<u64>) -> Result<Step> {
        if self.canceled.load(Ordering::Acquire) {
            return Ok(Step::Gone);
        }
        let Some(terminal) = self.terminal.upgrade() else {
            return Ok(Step::Gone);
        };
        let mut terminal = match terminal.try_lock() {
            Ok(terminal) => terminal,
            Err(TryLockError::WouldBlock) => return Ok(Step::Busy),
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
        };
        if self.canceled.load(Ordering::Acquire) {
            return Ok(Step::Gone);
        }
        let token = terminal.compression_activity()?;
        if idle_token != Some(token) {
            return Ok(Step::Activity(token));
        }
        Ok(match terminal.compress_scrollback_step()? {
            Compression::Pending => Step::Pending,
            Compression::Complete | Compression::Unsupported => Step::Complete,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn activity_changes_busy_lock_and_teardown_stop_compression() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)?));
        let compressor = Compressor::new(Arc::downgrade(&terminal));
        let Step::Activity(token) = compressor.step(None)? else {
            panic!("initial probe");
        };
        {
            let mut locked = terminal.lock().unwrap();
            assert_eq!(compressor.step(Some(token))?, Step::Busy);
            for _ in 0..10000 {
                locked.write(b"new historical output\r\n");
            }
        }
        assert!(matches!(compressor.step(Some(token))?, Step::Activity(_)));
        drop(terminal);
        assert_eq!(compressor.step(Some(token))?, Step::Gone);
        Ok(())
    }
    #[test]
    fn canceling_owner_skips_queued_clones_without_retaining_terminal() -> Result<()> {
        let terminal = Arc::new(Mutex::new(Terminal::new(80, 24)?));
        let compressor = Compressor::new(Arc::downgrade(&terminal));
        let cancellation = compressor.cancel_on_drop();
        let queued = compressor.clone();
        drop(cancellation);
        let _locked = terminal.lock().unwrap();
        assert_eq!(queued.step(None)?, Step::Gone);
        assert_eq!(Arc::strong_count(&terminal), 1);
        Ok(())
    }
}
