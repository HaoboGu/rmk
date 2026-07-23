//! STM32 ROM bootloader (DFU) support.
//!
//! Emits the boot-time bootloader check and registers the request function for the
//! `Bootloader` keycode. See `rmk::boot::stm32` for the mechanism.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rmk_config::resolved::hardware::{ChipModel, ChipSeries};

/// System memory (ROM bootloader) base address for an STM32 chip, from AN2606.
///
/// Cross-checked against QMK's `platforms/chibios/mcu_selection.mk` defaults for
/// `bootloader = stm32-dfu`. `None` for chips not covered yet.
fn stm32_system_memory_address(chip: &str) -> Option<u32> {
    // keyboard.toml isn't case-normalized
    let chip = chip.to_ascii_lowercase();

    // H7R/H7S have their own address, don't let them fall into the generic H7 entry
    if chip.starts_with("stm32h7r") || chip.starts_with("stm32h7s") {
        return None;
    }

    // More specific prefixes first
    let table: &[(&str, u32)] = &[
        // F030xC is the outlier within F03x (AN2606 table 27 vs 26)
        ("stm32f030cc", 0x1FFF_D800),
        ("stm32f030rc", 0x1FFF_D800),
        ("stm32f04", 0x1FFF_C400),
        ("stm32f07", 0x1FFF_C800),
        ("stm32f09", 0x1FFF_D800),
        ("stm32f0", 0x1FFF_EC00),
        // F1 connectivity line
        ("stm32f105", 0x1FFF_B000),
        ("stm32f107", 0x1FFF_B000),
        ("stm32f2", 0x1FFF_0000),
        ("stm32f3", 0x1FFF_D800),
        ("stm32f4", 0x1FFF_0000),
        ("stm32f7", 0x1FF0_0000),
        ("stm32g0", 0x1FFF_0000),
        ("stm32g4", 0x1FFF_0000),
        ("stm32h503", 0x0BF8_7000),
        ("stm32h5", 0x0BF9_7000),
        ("stm32h7a", 0x1FF0_A800),
        ("stm32h7b", 0x1FF0_A000),
        ("stm32h7", 0x1FF0_9800),
        ("stm32l0", 0x1FF0_0000),
        ("stm32l1", 0x1FF0_0000),
        ("stm32l4", 0x1FFF_0000),
        ("stm32l5", 0x0BF9_0000),
        ("stm32u5", 0x0BF9_0000),
        // WBA differs from WB, match first
        ("stm32wba", 0x0BF8_8000),
        ("stm32wb", 0x1FFF_0000),
        ("stm32wl", 0x1FFF_0000),
        ("stm32c0", 0x1FFF_0000),
    ];
    if let Some((_, addr)) = table.iter().find(|(prefix, _)| chip.starts_with(prefix)) {
        return Some(*addr);
    }
    // F1 XL-density (flash size code F or G) has its own address
    if chip.starts_with("stm32f1") {
        return match chip.as_bytes().last() {
            Some(b'f' | b'g') => Some(0x1FFF_E000),
            _ => Some(0x1FFF_F000),
        };
    }
    None
}

/// Bootloader check and `Bootloader` keycode registration for STM32 chip init. Empty
/// for other chips and for STM32 chips without a known address.
///
/// Must come before any clock or peripheral configuration.
pub(crate) fn stm32_bootloader_prelude(chip: &ChipModel) -> TokenStream2 {
    if chip.series != ChipSeries::Stm32 {
        return quote! {};
    }
    match stm32_system_memory_address(&chip.chip) {
        Some(addr) => quote! {
            ::rmk::boot::stm32::enter_bootloader_if_requested(#addr);
            ::rmk::boot::register_bootloader_jump(::rmk::boot::stm32::request_bootloader);
        },
        None => quote! {},
    }
}
