use crate::KeyboardTomlConfig;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ChipSeries {
    Stm32,
    Nrf52,
    #[default]
    Rp2040,
    Esp32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChipModel {
    pub series: ChipSeries,
    pub chip: String,
    pub board: Option<String>,
}

impl KeyboardTomlConfig {
    pub fn get_chip_model(&self) -> Result<ChipModel, String> {
        // Duplicate the logic in `rmk-macro/src/keyboard_config.rs`
        if self.keyboard.board.is_none() == self.keyboard.chip.is_none() {
            return Err("Either \"board\" or \"chip\" should be set in keyboard.toml, but not both".to_string());
        }

        // Check board type
        if let Some(board) = self.keyboard.board.clone() {
            match board.as_str() {
                "nice!nano" | "nice!nano_v2" | "XIAO BLE" => Ok(ChipModel {
                    series: ChipSeries::Nrf52,
                    chip: "nrf52840".to_string(),
                    board: Some(board),
                }),
                _ => Err(format!("Unsupported board: {}", board)),
            }
        } else if let Some(chip) = self.keyboard.chip.clone() {
            if chip.to_lowercase().starts_with("stm32") {
                Ok(ChipModel {
                    series: ChipSeries::Stm32,
                    chip,
                    board: None,
                })
            } else if chip.to_lowercase().starts_with("nrf52") {
                Ok(ChipModel {
                    series: ChipSeries::Nrf52,
                    chip,
                    board: None,
                })
            } else if chip.to_lowercase().starts_with("rp2040") {
                Ok(ChipModel {
                    series: ChipSeries::Rp2040,
                    chip,
                    board: None,
                })
            } else if chip.to_lowercase().starts_with("esp32") {
                Ok(ChipModel {
                    series: ChipSeries::Esp32,
                    chip,
                    board: None,
                })
            } else {
                Err(format!("Unsupported chip: {}", chip))
            }
        } else {
            Err("Neither board nor chip is specified".to_string())
        }
    }
}
