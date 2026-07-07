mod behavior;
#[cfg(feature = "_ble")]
mod ble_battery;
mod device;
#[cfg(feature = "rynk")]
mod lock;
mod positional;
mod storage;
mod vial;

pub use behavior::{
    AutoMouseLayerConfig, BehaviorConfig, CombosConfig, ForksConfig, KeyboardMacrosConfig, MorsesConfig,
    MouseKeyConfig, OneShotConfig, OneShotModifiersConfig, TapConfig,
};
#[cfg(feature = "_ble")]
pub use ble_battery::BleBatteryConfig;
pub use device::DeviceConfig;
#[cfg(feature = "rynk")]
pub use lock::LockConfig;
pub use positional::{Hand, PositionalConfig};
pub use storage::StorageConfig;
pub use vial::VialConfig;

/// Internal configurations for RMK keyboard.
#[derive(Default)]
pub struct RmkConfig<'a> {
    pub device_config: DeviceConfig<'a>,
    #[cfg(feature = "vial")]
    pub vial_config: VialConfig<'a>,
    #[cfg(feature = "rynk")]
    pub lock_config: LockConfig,
    #[cfg(feature = "storage")]
    pub storage_config: StorageConfig,
    #[cfg(feature = "_ble")]
    pub ble_battery_config: BleBatteryConfig<'a>,
}
