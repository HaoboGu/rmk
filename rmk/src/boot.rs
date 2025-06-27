pub fn jump_to_bootloader() {
    #[cfg(feature = "adafruit_bl")]
    // Reference: https://github.com/adafruit/Adafruit_nRF52_Bootloader/blob/d6b28e66053eea467166f44875e3c7ec741cb471/src/main.c#L107
    embassy_nrf::pac::POWER
        .gpregret()
        .write_value(embassy_nrf::pac::power::regs::Gpregret(0x57));

    #[cfg(feature = "rp2040_bl")]
    // Jump to RP2040 bootloader
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);

    #[cfg(not(any(feature = "adafruit_bl", feature = "rp2040_bl")))]
    warn!("Please specified a bootloader to jump to!");

    reboot_keyboard();
}

pub(crate) fn reboot_keyboard() {
    warn!("Rebooting keyboard!");
    // For cortex-m:
    #[cfg(all(
        target_arch = "arm",
        target_os = "none",
        any(target_abi = "eabi", target_abi = "eabihf")
    ))]
    cortex_m::peripheral::SCB::sys_reset();

    #[cfg(feature = "_esp_ble")]
    esp_hal::system::software_reset();
}
