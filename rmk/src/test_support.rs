use core::future::Future;

// `embassy-time`'s MockDriver is a process-global singleton, so running the
// suite under plain `cargo test` lets tests race on it and hang at the 60 s
// virtual-time kill switch. Abort at test-binary startup with a pointer
// to the right runner instead of making the user wait for that timeout.
#[ctor::ctor(unsafe)]
fn require_nextest() {
    if std::env::var_os("NEXTEST").is_none() {
        eprintln!(
            "\nrmk tests must run under cargo-nextest (embassy-time's MockDriver \
             is a process-global singleton and needs per-test process isolation).\n\
             \n  cargo install cargo-nextest --locked\n\n\
             Then from rmk/:\n\n  \
             cargo nextest run --no-default-features \
             --features=split,vial,storage,async_matrix,_ble\n\n\
             Or for the full feature matrix: `sh scripts/test_all.sh` from the repo root.\n"
        );
        std::process::exit(1);
    }
}

pub(crate) fn test_block_on<F: Future>(future: F) -> F::Output {
    crate::sim::test_block_on(future)
}
