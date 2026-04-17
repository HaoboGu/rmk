//! Single-threaded executor that drives `embassy-time`'s mock clock.
//!
//! The integration tests run against `embassy-time/mock-driver` (activated by
//! the dev-only `std` feature in `rmk/Cargo.toml`) instead of the host
//! wall-clock. The mock clock only advances when somebody calls
//! `MockDriver::get().advance(...)`, so a vanilla `embassy_futures::block_on`
//! would just spin forever the first time a test (or the keyboard runtime)
//! awaits a `Timer`.
//!
//! `test_block_on` polls the future and, whenever it returns `Pending`,
//! advances the mock clock by a fixed step. This decouples test timing from
//! host CPU load and makes the previously flaky morse / tap-hold suites
//! deterministic.

use core::future::Future;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use embassy_time::{Duration, MockDriver};

/// Granularity of virtual-time advancement per poll.
///
/// 100 µs is fine enough that timer-driven decisions never miss a deadline
/// (a 50 ms inter-event delay resolves in 500 polls; the 5 s outer kill
/// timer in `run_key_sequence_test` resolves in 50 000 polls — both are
/// CPU-trivial). `MockDriver` doesn't expose the queue's next-expiration
/// time, so a fixed step is the simplest correct choice.
const STEP: Duration = Duration::from_micros(100);

/// Maximum number of poll iterations before we declare a deadlock.
///
/// At `STEP = 100 µs`, this caps at 60 s of *virtual* time — far past any
/// legitimate test scenario. Hitting this almost certainly means the future
/// is awaiting something that will never resolve (e.g. a channel that nobody
/// publishes to), not a slow CPU.
const MAX_ITERS: usize = 60_000_000;

/// Drive `fut` to completion against the mock clock.
///
/// Resets the global `MockDriver` to virtual time 0 and an empty wake queue
/// before the first poll, so each `test_block_on` invocation starts from a
/// known state. Nextest already isolates tests by process; this is an extra
/// guarantee for repeated calls within the same process (e.g. the macro's
/// inner `block_on` plus `wrap_keymap`'s).
pub fn test_block_on<F: Future>(fut: F) -> F::Output {
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
