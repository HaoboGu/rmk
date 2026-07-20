use core::future::Future;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::boxed::Box;

use embassy_time::{Duration, MockDriver};

const STEP: Duration = Duration::from_micros(100);
const MAX_ITERS: usize = 600_000;

pub(super) fn test_block_on<F: Future>(future: F) -> F::Output {
    MockDriver::get().reset();

    // The waker carries no data, and every vtable operation is a no-op.
    let waker = unsafe { Waker::from_raw(RAW_WAKER) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);

    for _ in 0..MAX_ITERS {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        MockDriver::get().advance(STEP);
    }

    panic!(
        "test_block_on: future did not resolve within {} iterations ({} s of virtual time)",
        MAX_ITERS,
        (MAX_ITERS as u64 * STEP.as_micros()) / 1_000_000,
    );
}

const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &WAKER_VTABLE);

const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(|_| RAW_WAKER, |_| {}, |_| {}, |_| {});
