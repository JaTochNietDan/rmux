//! Linux child-exit and shutdown notification without periodic polling.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
};

#[derive(Clone)]
pub(crate) struct ProcessWakeup {
    process: Arc<OwnedFd>,
    close_event: Arc<OwnedFd>,
}
impl ProcessWakeup {
    pub fn new(pid: u32) -> io::Result<Self> {
        // The child remains ours to reap, so its PID cannot be reused here.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return Err(io::Error::new(
                error.kind(),
                format!(
                    "pidfd_open failed; Linux requires a kernel and sandbox policy allowing process descriptors: {error}"
                ),
            ));
        }
        let process = Arc::new(unsafe { OwnedFd::from_raw_fd(fd as i32) });
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let close_event = Arc::new(unsafe { OwnedFd::from_raw_fd(fd) });
        Ok(Self {
            process,
            close_event,
        })
    }
    pub fn close(&self) -> io::Result<()> {
        let value = 1u64;
        loop {
            let result = unsafe {
                libc::write(
                    self.close_event.as_raw_fd(),
                    (&value as *const u64).cast(),
                    size_of::<u64>(),
                )
            };
            if result == size_of::<u64>() as isize {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => continue,
                // A saturated counter already has a pending wakeup.
                io::ErrorKind::WouldBlock => return Ok(()),
                _ => return Err(error),
            }
        }
    }
    /// Sleeps until process exit or an explicit close request, with no timeout.
    pub fn wait(&self) -> io::Result<bool> {
        let mut fds = [
            libc::pollfd {
                fd: self.process.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.close_event.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        loop {
            let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) };
            if result < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                return Err(io::Error::other("process wakeup descriptor failed"));
            }
            if fds[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                return Ok(false);
            }
            if fds[1].revents & libc::POLLIN != 0 {
                return Ok(true);
            }
        }
    }
}
