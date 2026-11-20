//! A macOS reaper wakeup that retains child ownership until termination/reaping.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
};

#[derive(Clone)]
pub(crate) struct ProcessWakeup {
    queue: Arc<OwnedFd>,
    already_exited: bool,
}
impl ProcessWakeup {
    pub fn new(pid: u32) -> io::Result<Self> {
        // SAFETY: the returned descriptor is immediately transferred to OwnedFd.
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let queue = Arc::new(unsafe { OwnedFd::from_raw_fd(fd) });
        if unsafe { libc::fcntl(queue.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut this = Self {
            queue,
            already_exited: false,
        };
        this.change(1, libc::EVFILT_USER, libc::EV_ADD | libc::EV_CLEAR, 0)?;
        if let Err(error) = this.change(
            pid as usize,
            libc::EVFILT_PROC,
            libc::EV_ADD | libc::EV_ONESHOT,
            libc::NOTE_EXIT,
        ) {
            if error.raw_os_error() == Some(libc::ESRCH) {
                // An exited child remains ours to reap, even if registration lost
                // the race with its exit. No other thread has waited on it.
                this.already_exited = true;
            } else {
                return Err(error);
            }
        }
        Ok(this)
    }
    fn change(&self, ident: usize, filter: i16, flags: u16, fflags: u32) -> io::Result<()> {
        let event = libc::kevent {
            ident,
            filter,
            flags,
            fflags,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let result = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                &event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    pub fn close(&self) -> io::Result<()> {
        self.change(1, libc::EVFILT_USER, 0, libc::NOTE_TRIGGER)
    }
    /// True requests termination; false means the child already exited.
    pub fn wait(&self) -> io::Result<bool> {
        if self.already_exited {
            return Ok(false);
        }
        loop {
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::kevent(
                    self.queue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &mut event,
                    1,
                    std::ptr::null(),
                )
            };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if result > 0 {
                return Ok(event.filter == libc::EVFILT_USER);
            }
        }
    }
}
