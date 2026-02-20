//! Configuration for wireless protocols
//!
//! This module provides configuration types for various wireless protocols.
//! Each protocol has its own configuration type (e.g., `GazellConfig` for
//! Nordic Gazell on nRF52840).

/// Common trait for wireless protocol configurations
///
/// This trait provides a protocol-agnostic interface for validating
/// wireless configurations. Each wireless protocol should implement
/// this trait for its configuration type.
///
/// # Example
///
/// ```no_run
/// use rmk::wireless::{WirelessConfig, GazellConfig};
///
/// let config = GazellConfig::default();
/// assert!(config.validate());
/// ```
pub trait WirelessConfig {
    /// Validate configuration parameters
    ///
    /// Returns `true` if all parameters are within valid ranges.
    fn validate(&self) -> bool;

    /// Get a human-readable description of the configuration
    fn description(&self) -> &'static str {
        "Wireless configuration"
    }
}

/// Nordic Gazell configuration for nRF52840
///
/// This struct contains all configuration parameters for the Nordic Gazell
/// protocol on nRF52 series MCUs (nRF52832, nRF52840, etc.).
///
/// **Note**: This configuration is specific to Nordic nRF52 hardware.
/// For other MCUs, implement your own configuration type and `WirelessTransport`.
///
/// This struct contains all configuration parameters for the Gazell protocol.
///
/// # Example
///
/// ```no_run
/// use rmk::wireless::GazellConfig;
///
/// let config = GazellConfig {
///     channel: 4,
///     data_rate: DataRate::_1Mbps,
///     tx_power: TxPower::Pos0dBm,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct GazellConfig {
    /// RF channel (0-100)
    ///
    /// Each channel is 1MHz wide, starting at 2400MHz:
    /// - Channel 0 = 2400 MHz
    /// - Channel 4 = 2404 MHz
    /// - Channel 100 = 2500 MHz
    ///
    /// **Recommendation**: Use channels 4, 25, 42, 63, or 79 to avoid
    /// WiFi interference (WiFi channels 1, 6, 11).
    pub channel: u8,

    /// Data rate
    ///
    /// Higher data rates provide better throughput but reduced range.
    pub data_rate: DataRate,

    /// Transmit power
    ///
    /// Higher power increases range but consumes more battery.
    pub tx_power: TxPower,

    /// Maximum number of automatic retransmissions
    ///
    /// Range: 0-15
    /// - 0 = No retransmission
    /// - 3 = Recommended for reliability
    /// - 15 = Maximum reliability, higher latency
    pub max_retries: u8,

    /// ACK timeout in microseconds
    ///
    /// Time to wait for acknowledgment before retrying.
    /// Range: 250-4000 μs
    ///
    /// **Recommendation**: 250-500 μs for keyboard applications
    pub ack_timeout_us: u16,

    /// Base address (4 bytes)
    ///
    /// Common base address for all pipes. Should be unique
    /// to avoid interference with other Gazell networks.
    pub base_address: [u8; 4],

    /// Address prefix (pipe 0)
    ///
    /// Each pipe has its own prefix byte combined with base address.
    pub address_prefix: u8,
}

impl Default for GazellConfig {
    fn default() -> Self {
        Self {
            channel: 4,                           // 2404 MHz (safe from WiFi)
            data_rate: DataRate::_1Mbps,         // Good balance
            tx_power: TxPower::Pos0dBm,          // 0dBm (1mW)
            max_retries: 3,                       // Reliable but low latency
            ack_timeout_us: 250,                  // Fast ACK
            base_address: [0xE7, 0xE7, 0xE7, 0xE7], // Default Gazell address
            address_prefix: 0xAA,                 // Default prefix
        }
    }
}

impl WirelessConfig for GazellConfig {
    fn validate(&self) -> bool {
        self.channel <= 100
            && self.max_retries <= 15
            && self.ack_timeout_us >= 250
            && self.ack_timeout_us <= 4000
    }

    fn description(&self) -> &'static str {
        "Nordic Gazell 2.4GHz protocol (nRF52840)"
    }
}

impl GazellConfig {
    /// Create a low-latency configuration
    ///
    /// Optimized for keyboard/mouse with minimal latency:
    /// - Fast data rate (2Mbps)
    /// - Low retries
    /// - Short ACK timeout
    pub fn low_latency() -> Self {
        Self {
            data_rate: DataRate::_2Mbps,
            max_retries: 2,
            ack_timeout_us: 250,
            ..Default::default()
        }
    }

    /// Create a long-range configuration
    ///
    /// Optimized for maximum range:
    /// - Lower data rate (250kbps)
    /// - Maximum TX power
    /// - More retries
    pub fn long_range() -> Self {
        Self {
            data_rate: DataRate::_250Kbps,
            tx_power: TxPower::Pos8dBm,
            max_retries: 5,
            ack_timeout_us: 500,
            ..Default::default()
        }
    }

    /// Create a low-power configuration
    ///
    /// Optimized for battery life:
    /// - Lower TX power
    /// - Fewer retries
    /// - Fast data rate (less air time)
    pub fn low_power() -> Self {
        Self {
            data_rate: DataRate::_1Mbps,
            tx_power: TxPower::Neg4dBm,
            max_retries: 2,
            ack_timeout_us: 250,
            ..Default::default()
        }
    }
}

/// Gazell data rate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRate {
    /// 250 kbps - Maximum range
    _250Kbps = 0,

    /// 1 Mbps - Good balance (default)
    _1Mbps = 1,

    /// 2 Mbps - Minimum latency
    _2Mbps = 2,
}

/// Transmit power levels for nRF52840
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxPower {
    /// -40 dBm
    Neg40dBm = 0xD8,

    /// -20 dBm
    Neg20dBm = 0xEC,

    /// -16 dBm
    Neg16dBm = 0xF0,

    /// -12 dBm
    Neg12dBm = 0xF4,

    /// -8 dBm
    Neg8dBm = 0xF8,

    /// -4 dBm
    Neg4dBm = 0xFC,

    /// 0 dBm (1 mW) - Default
    Pos0dBm = 0x00,

    /// +2 dBm
    Pos2dBm = 0x02,

    /// +3 dBm
    Pos3dBm = 0x03,

    /// +4 dBm
    Pos4dBm = 0x04,

    /// +5 dBm
    Pos5dBm = 0x05,

    /// +6 dBm
    Pos6dBm = 0x06,

    /// +7 dBm
    Pos7dBm = 0x07,

    /// +8 dBm (6.3 mW) - Maximum
    Pos8dBm = 0x08,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = GazellConfig::default();
        assert!(config.validate());
    }

    #[test]
    fn test_low_latency_config_valid() {
        let config = GazellConfig::low_latency();
        assert!(config.validate());
        assert_eq!(config.data_rate, DataRate::_2Mbps);
    }

    #[test]
    fn test_long_range_config_valid() {
        let config = GazellConfig::long_range();
        assert!(config.validate());
        assert_eq!(config.data_rate, DataRate::_250Kbps);
    }

    #[test]
    fn test_low_power_config_valid() {
        let config = GazellConfig::low_power();
        assert!(config.validate());
    }

    #[test]
    fn test_invalid_channel() {
        let mut config = GazellConfig::default();
        config.channel = 101; // Out of range
        assert!(!config.validate());
    }

    #[test]
    fn test_invalid_retries() {
        let mut config = GazellConfig::default();
        config.max_retries = 16; // Out of range
        assert!(!config.validate());
    }
}
