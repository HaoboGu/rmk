use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::resolved::Hardware;
#[cfg(feature = "watchdog")]
use rmk_config::resolved::hardware::ChipSeries;

/// Returns (init_tokens, run task) for the chip's hardware watchdog.
///
/// When a chip has watchdog codegen the init tokens declare and start
/// `watchdog_runner`, and the task is `watchdog_runner.run()` which
/// must be joined with the other tasks.
///
/// Chips without codegen return empty tokens and `None`.
#[cfg(feature = "watchdog")]
pub(crate) fn expand_watchdog_init(hardware: &Hardware) -> (TokenStream2, Option<TokenStream2>) {
    let init = match hardware.chip.series {
        ChipSeries::Stm32 | ChipSeries::Rp2040 | ChipSeries::Nrf52 | ChipSeries::Esp32 => quote! {},
    };

    if init.is_empty() {
        (init, None)
    } else {
        (init, Some(quote! { watchdog_runner.run() }))
    }
}

/// No-op when the `watchdog` feature is disabled; codegen emits nothing.
#[cfg(not(feature = "watchdog"))]
pub(crate) fn expand_watchdog_init(_hardware: &Hardware) -> (TokenStream2, Option<TokenStream2>) {
    (quote! {}, None)
}
