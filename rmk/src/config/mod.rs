pub mod behavior;
#[cfg(feature = "_ble")]
mod ble;
mod device;
mod positional;
mod storage;
mod vial;

pub use behavior::{
    BehaviorConfig, CombosConfig, ForksConfig, KeyboardMacrosConfig, MorsesConfig, MouseKeyConfig, OneShotConfig,
    TapConfig,
};
#[cfg(feature = "_ble")]
pub use ble::BleBatteryConfig;
pub use device::DeviceConfig;
pub use positional::{Hand, PositionalConfig};
pub use storage::StorageConfig;
pub use vial::VialConfig;

/// Internal configurations for RMK keyboard.
#[derive(Default)]
pub struct RmkConfig<'a> {
    pub device_config: DeviceConfig<'a>,
    #[cfg(feature = "vial")]
    pub vial_config: VialConfig<'a>,
    #[cfg(feature = "storage")]
    pub storage_config: StorageConfig,
    #[cfg(feature = "_ble")]
    pub ble_battery_config: BleBatteryConfig<'a>,
}
