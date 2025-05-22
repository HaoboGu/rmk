pub mod esp32;
pub mod nrf52810;
pub mod nrf52832;
pub mod nrf52833;
pub mod nrf52840;
pub mod rp2040;
pub mod stm32;

pub use esp32::default_esp32;
pub use nrf52810::default_nrf52810;
pub use nrf52832::default_nrf52832;
pub use nrf52833::default_nrf52833;
pub use nrf52840::default_nrf52840;
pub use rp2040::default_rp2040;
pub use stm32::default_stm32;

use crate::{KeyboardConfig, KeyboardTomlConfig};

impl KeyboardTomlConfig {
    pub fn get_default_config(&self) -> Result<KeyboardConfig, String> {
        let chip = self.get_chip_model()?;
        if let Some(board) = chip.board.clone() {
            match board.as_str() {
                "nice!nano" | "nice!nano_v2" | "XIAO BLE" => {
                    return Ok(default_nrf52840(chip));
                }
                _ => (),
            }
        }

        let config = match chip.chip.as_str() {
            "nrf52840" => default_nrf52840(chip),
            "nrf52833" => default_nrf52833(chip),
            "nrf52832" => default_nrf52832(chip),
            "nrf52810" | "nrf52811" => default_nrf52810(chip),
            "rp2040" => default_rp2040(chip),
            s if s.starts_with("stm32") => default_stm32(chip),
            s if s.starts_with("esp32") => default_esp32(chip),
            _ => {
                return Err(format!(
                    "No default chip config for {}, please report at https://github.com/HaoboGu/rmk/issues",
                    chip.chip
                ));
            }
        };

        Ok(config)
    }
}
