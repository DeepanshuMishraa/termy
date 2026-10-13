//! Engine-lock hand-off between the PTY reader and every other caller.
//!
//! `std::sync::Mutex` is not fair: a reader thread that parses output in a
//! loop reacquires the lock microseconds after releasing it, so the UI can
//! wait many frames for a snapshot during `cat`-style floods. Other callers
//! announce themselves, and the reader yields to them before relocking.
use super::*;
use parking_lot::MutexGuard;
use std::sync::atomic::AtomicUsize;

/// Output fed under one lock hold; the reader yields between slices.
const FEED_SLICE: usize = 2 * PARSE_BATCH;
/// Upper bound on one yield, so a stuck waiter cannot stall output.
const READER_YIELD_LIMIT: Duration = Duration::from_millis(2);

#[derive(Default)]
pub(super) struct LockWaiters(AtomicUsize);

/// Announces a waiting caller for as long as it lives, so a panic while
/// locking cannot leave the feeder yielding to a waiter that is gone.
struct WaiterAnnouncement<'a>(&'a AtomicUsize);

impl<'a> WaiterAnnouncement<'a> {
    fn new(waiters: &'a LockWaiters) -> Self {
        waiters.0.fetch_add(1, Ordering::AcqRel);
        Self(&waiters.0)
    }
}

impl Drop for WaiterAnnouncement<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Shared {
    pub(super) fn state(&self) -> MutexGuard<'_, State> {
        let _announcement = WaiterAnnouncement::new(&self.lock_waiters);
        self.lock_state()
    }

    pub(super) fn live_transport(&self) -> Option<Arc<Transport>> {
        self.transport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade()
    }

    /// Queue replies to the child. Callers hold the engine lock, so replies
    /// from different feeders (reader slices, the synchronized-update
    /// watchdog) reach the child in the order the engine produced them.
    pub(super) fn write_replies_locked(transport: &Transport, replies: Vec<u8>) {
        if replies.is_empty() {
            return;
        }
        if let Err(error) = transport.write_protocol_reply_owned(replies) {
            log::warn!("dropping terminal protocol reply: {error}");
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }

    /// Lock for feeding output, after any announced waiter got its turn.
    fn feeder_state(&self) -> MutexGuard<'_, State> {
        if self.lock_waiters.0.load(Ordering::Acquire) != 0 {
            let deadline = Instant::now() + READER_YIELD_LIMIT;
            while self.lock_waiters.0.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
                std::thread::yield_now();
            }
        }
        self.lock_state()
    }

    /// Feed output in bounded slices so waiting callers can interleave.
    /// Hydration replays stay atomic: they finish synchronized updates.
    pub(super) fn feed(
        self: &Arc<Self>,
        bytes: &[u8],
        hydrate: bool,
        buffer_replies: bool,
    ) -> Vec<u8> {
        let slice = if hydrate {
            bytes.len().max(1)
        } else {
            FEED_SLICE
        };
        let mut replies = Vec::new();
        // PTY output queues each slice's replies before the lock is released.
        // Holding them until the whole read is parsed would let the watchdog
        // send a later reply first while this read waits behind a UI lock.
        let transport = (!buffer_replies && !hydrate)
            .then(|| self.live_transport())
            .flatten();
        let (mut should_notify, mut tab_chrome_changed) = (false, false);
        let mut rest = bytes;
        while !rest.is_empty() {
            let mut state = self.feeder_state();
            let (mut slice_replies, consumed) = state.feed(&rest[..rest.len().min(slice)], hydrate);
            rest = &rest[consumed..];
            if let Some(upload) = state.engine.take_deferred_upload() {
                // Decode the image (base64, zlib, PNG, file reads) unlocked.
                // Output has one feeder at a time (the PTY reader, or the
                // host for display terminals), so the upload still lands
                // between the bytes around it.
                drop(state);
                let prepared = upload.prepare();
                state = self.feeder_state();
                slice_replies.append(&mut state.commit_upload(prepared));
            }
            if buffer_replies {
                state.append_replies(&slice_replies);
            } else if let Some(transport) = &transport {
                Self::write_replies_locked(transport, slice_replies);
            } else {
                replies.append(&mut slice_replies);
            }
            if state.engine.take_history_activity() {
                state.defer_history_compaction(Instant::now());
            }
            should_notify |= !state.engine.modes().synchronized_update || !state.events.is_empty();
            tab_chrome_changed |= state.take_tab_chrome_changed();
            // Publish while the engine lock still protects this deadline: a later
            // feed must not have its timer replaced by an earlier feed's deadline.
            self.schedule_maintenance(&mut state);
        }
        if should_notify {
            self.notify();
        }
        if tab_chrome_changed {
            self.notify_tab_chrome();
        }
        replies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicking_waiter_withdraws_its_announcement() {
        let waiters = LockWaiters::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _announcement = WaiterAnnouncement::new(&waiters);
            assert_eq!(waiters.0.load(Ordering::Acquire), 1);
            panic!("lock panicked");
        }));
        assert!(result.is_err());
        assert_eq!(waiters.0.load(Ordering::Acquire), 0);
    }
}
