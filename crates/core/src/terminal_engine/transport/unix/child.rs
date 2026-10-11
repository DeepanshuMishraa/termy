// PTY allocation and descriptor hygiene for the Unix PTY child.
// Included into `unix.rs`, so it shares that module's imports and items.

/// Open a PTY pair whose descriptors are close-on-exec from creation, so no
/// concurrent fork/exec elsewhere in the process inherits either end.
fn open_pty_pair(size: WinSize) -> io::Result<(File, std::os::fd::OwnedFd)> {
    let window = rustix_openpty::rustix::termios::Winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: size.pixel_width,
        ws_ypixel: size.pixel_height,
    };
    let pty = rustix_openpty::openpty(None, Some(&window)).map_err(io::Error::from)?;
    Ok((File::from(pty.controller), pty.user))
}

/// Descriptor-table snapshot storage for the forked child. It is allocated
/// before fork because the child may neither allocate nor lock.
struct InheritedDescriptors {
    #[cfg(target_os = "macos")]
    listing: Vec<libc::proc_fdinfo>,
    limit: c_int,
}

impl InheritedDescriptors {
    const MAX_SCAN: c_int = 1 << 16;

    fn new() -> Self {
        // SAFETY: sysconf has no preconditions.
        let limit = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
        let limit = if limit <= 0 {
            1024
        } else {
            limit.min(Self::MAX_SCAN.into()) as c_int
        };
        Self {
            #[cfg(target_os = "macos")]
            listing: vec![
                libc::proc_fdinfo {
                    proc_fd: 0,
                    proc_fdtype: 0,
                };
                4096
            ],
            limit,
        }
    }

    /// Close every descriptor above stderr except `keep`. Runs between fork
    /// and exec, so it only issues raw syscalls into preallocated storage.
    unsafe fn close_all_except(&mut self, keep: c_int) {
        #[cfg(target_os = "linux")]
        // SAFETY: close_range only closes descriptors this child owns.
        unsafe {
            let close_range = |low: c_int, high: c_int| {
                low > high
                    || libc::syscall(libc::SYS_close_range, low as u32, high as u32, 0) == 0
            };
            if close_range(3, keep - 1) && close_range(keep + 1, c_int::MAX) {
                return;
            }
        }
        #[cfg(target_os = "macos")]
        // SAFETY: proc_pidinfo writes at most `capacity` bytes into the
        // preallocated listing and does not allocate.
        unsafe {
            let entry = std::mem::size_of::<libc::proc_fdinfo>();
            let capacity = (self.listing.len() * entry) as c_int;
            let written = libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                self.listing.as_mut_ptr().cast(),
                capacity,
            );
            let count = usize::try_from(written).unwrap_or(0) / entry;
            // A full buffer may be truncated, so fall back to the scan.
            if written > 0 && count < self.listing.len() {
                for info in &self.listing[..count] {
                    if info.proc_fd > 2 && info.proc_fd != keep {
                        let _ = close(info.proc_fd);
                    }
                }
                return;
            }
        }
        for fd in 3..self.limit {
            if fd != keep {
                // SAFETY: closing an unowned or invalid descriptor in the
                // child only affects the child's table before exec.
                unsafe {
                    let _ = close(fd);
                }
            }
        }
    }
}

/// Make `user` the session's controlling terminal and the standard streams,
/// as `login_tty` does, then drop every other inherited descriptor. Returns
/// the status endpoint, which moves above stderr if it started below it.
unsafe fn attach_child_terminal(
    user: c_int,
    mut status: c_int,
    descriptors: &mut InheritedDescriptors,
) -> Result<c_int, c_int> {
    // SAFETY: only async-signal-safe syscalls on descriptors owned by this
    // freshly forked child.
    unsafe {
        if status <= 2 {
            status = libc::fcntl(status, libc::F_DUPFD_CLOEXEC, 3);
            if status < 0 {
                return Err(-1);
            }
        }
        if libc::setsid() < 0 || libc::ioctl(user, libc::TIOCSCTTY as _, 0) < 0 {
            return Err(status);
        }
        for target in 0..=2 {
            if libc::dup2(user, target) < 0 {
                return Err(status);
            }
        }
        descriptors.close_all_except(status);
    }
    Ok(status)
}
