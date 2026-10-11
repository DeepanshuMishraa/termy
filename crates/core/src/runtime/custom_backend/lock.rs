//! Engine-lock hand-off between the PTY reader and every other caller.
//!
//! `std::sync::Mutex` is not fair: a reader thread that parses output in a
//! loop reacquires the lock microseconds after releasing it, so the UI can
//! wait many frames for a snapshot during `cat`-style floods. Other callers
//! announce themselves, and the reader yields to them before relocking.
use super::*;
use std::sync::atomic::AtomicUsize;

/// Output fed under one lock hold; the reader yields between slices.
const FEED_SLICE: usize = 2 * PARSE_BATCH;
/// Upper bound on one yield, so a stuck waiter cannot stall output.
const READER_YIELD_LIMIT: Duration = Duration::from_millis(2);

#[derive(Default)]
pub(super) struct LockWaiters(AtomicUsize);

impl Shared {
    pub(super) fn state(&self) -> MutexGuard<'_, State> {
        self.lock_waiters.0.fetch_add(1, Ordering::AcqRel);
        let state = self.lock_state();
        self.lock_waiters.0.fetch_sub(1, Ordering::AcqRel);
        state
    }

    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
            self.schedule_maintenance(&state);
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
