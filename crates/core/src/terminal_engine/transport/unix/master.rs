// PTY master readiness and window-size helpers for the writer thread.
// Included into `unix.rs`, so it shares that module's imports and items.

fn wait_for_pty_writable(master: &File, wake: &UnixStream) -> io::Result<()> {
    let mut descriptors = [
        PollFd {
            fd: master.as_raw_fd(),
            events: POLLOUT,
            revents: 0,
        },
        PollFd {
            fd: wake.as_raw_fd(),
            events: POLLIN,
            revents: 0,
        },
    ];
    // std's channel and poll cannot be waited on together, so Resize, Close
    // and replies also write to `wake`. Readiness still wakes immediately when
    // the child drains input, avoiding a fixed delay between partial writes.
    // SAFETY: both descriptors are live and exactly two entries are passed.
    let ready = unsafe {
        poll(
            descriptors.as_mut_ptr(),
            2 as PollCount,
            WRITER_POLL_TIMEOUT_MS,
        )
    };
    if ready < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error);
        }
    }
    if descriptors[1].revents & POLLIN != 0 {
        let mut drained = [0; 64];
        while matches!((&*wake).read(&mut drained), Ok(read) if read == drained.len()) {}
    }
    let descriptor = &descriptors[0];
    if descriptor.revents & POLLNVAL != 0 {
        return Err(io::Error::new(
            ErrorKind::BrokenPipe,
            "Termy native PTY writer descriptor is invalid",
        ));
    }
    Ok(())
}

fn resize_pty_master(master: &File, window_size: WinSize) -> io::Result<()> {
    loop {
        // SAFETY: `master` owns a live PTY descriptor and `window_size` has the
        // platform winsize layout for the duration of the ioctl call.
        if unsafe { ioctl(master.as_raw_fd(), TIOCSWINSZ, &window_size) } >= 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
