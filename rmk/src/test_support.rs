//! Test-only `block_on` that drives `embassy-time`'s mock clock.
//!
//! Mirrors `tests/common/test_block_on.rs`. Lives under `src/` because
//! `#[cfg(test)] mod tests` blocks inside library files cannot import from
//! the `tests/` directory (that's a separate compilation target).
//!
//! Use as a drop-in replacement for `embassy_futures::block_on`:
//!
//! ```ignore
//! use crate::test_support::test_block_on as block_on;
//! ```

use core::future::Future;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use embassy_time::{Duration, MockDriver};

const STEP: Duration = Duration::from_micros(100);
const MAX_ITERS: usize = 60_000_000; // 60 s of virtual time

pub(crate) fn test_block_on<F: Future>(fut: F) -> F::Output {
    MockDriver::get().reset();

    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    let mut fut = Box::pin(fut);
    for _ in 0..MAX_ITERS {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return out;
        }
        MockDriver::get().advance(STEP);
    }
    panic!(
        "test_block_on: future did not resolve within {} iterations ({} s of virtual time)",
        MAX_ITERS,
        (MAX_ITERS as u64 * STEP.as_micros()) / 1_000_000,
    );
}

fn noop_waker() -> Waker {
    // Safety: every vtable function is a true no-op; no state is ever
    // dereferenced through the data pointer.
    unsafe { Waker::from_raw(RAW) }
}

const RAW: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);

const VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RAW, // clone
    |_| {},  // wake
    |_| {},  // wake_by_ref
    |_| {},  // drop
);
