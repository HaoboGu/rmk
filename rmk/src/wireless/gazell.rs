//! Nordic Gazell protocol implementation
//!
//! This module provides a `WirelessTransport` implementation using
//! Nordic's Gazell 2.4GHz protocol.
//!
//! # Features
//!
//! - Automatic retransmission and acknowledgment
//! - Multiple channels (0-100)
//! - Configurable data rate (250kbps / 1Mbps / 2Mbps)
//! - Configurable TX power (-40dBm to +8dBm)
//!
//! # Hardware Requirements
//!
//! - nRF52840, nRF52833, or nRF52832 MCU
//! - Nordic nRF5 SDK v17.1.0+
//! - Enable feature: `wireless_gazell_nrf52840` (or nrf52833/nrf52832)
//!
//! # Example
//!
//! ```no_run
//! use rmk::wireless::{GazellTransport, GazellConfig, WirelessTransport};
//!
//! let config = GazellConfig::low_latency();
//! let mut transport = GazellTransport::new(config);
//!
//! transport.init()?;
//! transport.set_device_mode()?;
//!
//! // Send an Elink frame
//! let frame = [0xAA, 0xBB, 0xCC];
//! transport.send_frame(&frame)?;
//! ```

use super::config::{GazellConfig, WirelessConfig};
use super::transport::{Result, WirelessError, WirelessTransport};
use heapless::Vec;

#[cfg(feature = "wireless_gazell")]
use rmk_gazell_sys as sys;

/// Convert C error code to Rust Result
#[cfg(feature = "wireless_gazell")]
fn convert_gz_error(code: sys::gz_error_t) -> Result<()> {
    match code {
        sys::GZ_OK => Ok(()),
        sys::GZ_ERR_SEND_FAILED => Err(WirelessError::SendFailed),
        sys::GZ_ERR_RECEIVE_FAILED => Err(WirelessError::ReceiveFailed),
        sys::GZ_ERR_FRAME_TOO_LARGE => Err(WirelessError::FrameTooLarge),
        sys::GZ_ERR_NOT_INITIALIZED => Err(WirelessError::NotInitialized),
        sys::GZ_ERR_BUSY => Err(WirelessError::Busy),
        sys::GZ_ERR_INVALID_CONFIG => Err(WirelessError::InvalidConfig),
        sys::GZ_ERR_HARDWARE => Err(WirelessError::HardwareError),
        _ => Err(WirelessError::HardwareError),
    }
}

/// Nordic Gazell transport implementation
///
/// Provides a safe wrapper around Nordic's Gazell protocol stack via FFI.
///
/// # Implementation Notes
///
/// When `wireless_gazell` feature is enabled:
/// - Uses Nordic nRF5 SDK via rmk-gazell-sys FFI bindings
/// - Requires NRF5_SDK_PATH environment variable during build
/// - Supports device mode (transmitter) and host mode (receiver)
///
/// When feature is disabled:
/// - Uses mock implementation for testing and development
/// - All operations succeed but no actual transmission occurs
///
/// # Usage
///
/// Device mode (keyboard):
/// ```no_run
/// # use rmk::wireless::{GazellTransport, GazellConfig, WirelessTransport};
/// let config = GazellConfig::low_latency();
/// let mut transport = GazellTransport::new(config);
/// transport.init()?;
/// transport.set_device_mode()?;
/// transport.send_frame(&[0xAA, 0xBB, 0xCC])?;
/// # Ok::<(), rmk::wireless::WirelessError>(())
/// ```
///
/// Host mode (dongle):
/// ```no_run
/// # use rmk::wireless::{GazellTransport, GazellConfig, WirelessTransport};
/// let config = GazellConfig::low_latency();
/// let mut transport = GazellTransport::new(config);
/// transport.init()?;
/// transport.set_host_mode()?;
/// if let Some(frame) = transport.recv_frame()? {
///     // Process received frame
/// }
/// # Ok::<(), rmk::wireless::WirelessError>(())
/// ```
pub struct GazellTransport {
    config: GazellConfig,
    initialized: bool,
}

impl GazellTransport {
    /// Create a new Gazell transport with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Gazell configuration parameters
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rmk::wireless::{GazellTransport, GazellConfig};
    ///
    /// let transport = GazellTransport::new(GazellConfig::default());
    /// ```
    pub fn new(config: GazellConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }

    /// Initialize the Gazell protocol
    ///
    /// This must be called before using `send_frame` or `recv_frame`.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Initialization successful
    /// * `Err(WirelessError)` - Initialization failed
    ///
    /// # Errors
    ///
    /// - `InvalidConfig` - Configuration validation failed
    /// - `HardwareError` - Failed to initialize Gazell hardware
    pub fn init(&mut self) -> Result<()> {
        // Validate configuration
        if !self.config.validate() {
            return Err(WirelessError::InvalidConfig);
        }

        #[cfg(feature = "wireless_gazell")]
        {
            // Convert Rust config to C struct
            let c_config = sys::gz_config_t {
                channel: self.config.channel,
                data_rate: self.config.data_rate as u8,
                tx_power: self.config.tx_power as i8,
                max_retries: self.config.max_retries,
                ack_timeout_us: self.config.ack_timeout_us,
                base_address: self.config.base_address,
                address_prefix: self.config.address_prefix,
            };

            // Call FFI to initialize Gazell
            let result = unsafe { sys::gz_init(&c_config) };
            convert_gz_error(result)?;

            #[cfg(feature = "defmt")]
            defmt::info!("Gazell: Initialized (channel={}, rate={}Mbps, power={}dBm)",
                         self.config.channel,
                         match self.config.data_rate {
                             0 => "0.25",
                             1 => "1",
                             2 => "2",
                             _ => "?",
                         },
                         self.config.tx_power);
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            // Mock implementation for testing
            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Initialized (MOCK)");
        }

        self.initialized = true;
        Ok(())
    }

    /// Set device mode (transmitter/device)
    ///
    /// In device mode, the keyboard acts as a transmitter sending
    /// data to the dongle (host/receiver).
    ///
    /// # Errors
    ///
    /// - `NotInitialized` - Must call `init()` first
    /// - `HardwareError` - Failed to set device mode
    pub fn set_device_mode(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        #[cfg(feature = "wireless_gazell")]
        {
            let result = unsafe { sys::gz_set_mode(sys::GZ_MODE_DEVICE) };
            convert_gz_error(result)?;

            #[cfg(feature = "defmt")]
            defmt::info!("Gazell: Set to device mode (transmitter)");
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Set to device mode (MOCK)");
        }

        Ok(())
    }

    /// Set host mode (receiver/host)
    ///
    /// In host mode, the dongle acts as a receiver listening for
    /// data from keyboards (devices/transmitters).
    ///
    /// # Errors
    ///
    /// - `NotInitialized` - Must call `init()` first
    /// - `HardwareError` - Failed to set host mode
    pub fn set_host_mode(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        #[cfg(feature = "wireless_gazell")]
        {
            let result = unsafe { sys::gz_set_mode(sys::GZ_MODE_HOST) };
            convert_gz_error(result)?;

            #[cfg(feature = "defmt")]
            defmt::info!("Gazell: Set to host mode (receiver)");
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Set to host mode (MOCK)");
        }

        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &GazellConfig {
        &self.config
    }

    /// Update configuration (requires re-initialization)
    pub fn set_config(&mut self, config: GazellConfig) -> Result<()> {
        if !config.validate() {
            return Err(WirelessError::InvalidConfig);
        }

        self.config = config;
        self.initialized = false;
        self.init()
    }
}

impl WirelessTransport for GazellTransport {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        if frame.len() > self.max_frame_size() {
            return Err(WirelessError::FrameTooLarge);
        }

        #[cfg(feature = "wireless_gazell")]
        {
            // Send frame via FFI (blocking call with timeout)
            let result = unsafe {
                sys::gz_send(frame.as_ptr(), frame.len() as u8)
            };

            convert_gz_error(result)?;

            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Sent {} bytes", frame.len());
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Sending {} bytes (MOCK)", frame.len());
        }

        Ok(())
    }

    fn recv_frame(&mut self) -> Result<Option<Vec<u8, 64>>> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        #[cfg(feature = "wireless_gazell")]
        {
            let mut buffer = [0u8; 64];
            let mut length: u8 = 0;

            // Non-blocking receive
            let result = unsafe {
                sys::gz_recv(buffer.as_mut_ptr(), &mut length, buffer.len() as u8)
            };

            convert_gz_error(result)?;

            if length > 0 {
                let mut vec = Vec::new();
                vec.extend_from_slice(&buffer[..length as usize])
                    .map_err(|_| WirelessError::FrameTooLarge)?;

                #[cfg(feature = "defmt")]
                defmt::trace!("Gazell: Received {} bytes", length);

                Ok(Some(vec))
            } else {
                Ok(None)
            }
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Checking for received frames (MOCK)");

            // Mock implementation - no data available
            Ok(None)
        }
    }

    fn is_ready(&self) -> bool {
        if !self.initialized {
            return false;
        }

        #[cfg(feature = "wireless_gazell")]
        {
            unsafe { sys::gz_is_ready() }
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            true
        }
    }

    fn max_frame_size(&self) -> usize {
        // Gazell maximum payload size is 32 bytes
        // But we can use Elink frames up to 64 bytes by fragmenting if needed
        32
    }

    fn flush(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        #[cfg(feature = "wireless_gazell")]
        {
            let result = unsafe { sys::gz_flush() };
            convert_gz_error(result)?;

            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Flushed TX/RX FIFOs");
        }

        #[cfg(not(feature = "wireless_gazell"))]
        {
            #[cfg(feature = "defmt")]
            defmt::trace!("Gazell: Flush (MOCK)");
        }

        Ok(())
    }
}

/// Async version of GazellTransport
///
/// # TODO
///
/// Implement async send/receive using Embassy
#[cfg(feature = "async")]
pub struct GazellTransportAsync {
    inner: GazellTransport,
}

#[cfg(feature = "async")]
impl GazellTransportAsync {
    pub fn new(config: GazellConfig) -> Self {
        Self {
            inner: GazellTransport::new(config),
        }
    }

    pub async fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        // TODO: Implement async send
        self.inner.send_frame(frame)
    }

    pub async fn recv_frame(&mut self) -> Result<Option<Vec<u8, 64>>> {
        // TODO: Implement async receive
        self.inner.recv_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_transport() {
        let config = GazellConfig::default();
        let transport = GazellTransport::new(config);
        assert!(!transport.is_ready());
    }

    #[test]
    fn test_init() {
        let config = GazellConfig::default();
        let mut transport = GazellTransport::new(config);
        assert!(transport.init().is_ok());
        assert!(transport.is_ready());
    }

    #[test]
    fn test_send_before_init_fails() {
        let config = GazellConfig::default();
        let mut transport = GazellTransport::new(config);
        let frame = [0xAA, 0xBB, 0xCC];
        assert_eq!(transport.send_frame(&frame), Err(WirelessError::NotInitialized));
    }

    #[test]
    fn test_frame_too_large() {
        let config = GazellConfig::default();
        let mut transport = GazellTransport::new(config);
        transport.init().unwrap();

        let large_frame = [0u8; 128]; // Exceeds max size
        assert_eq!(transport.send_frame(&large_frame), Err(WirelessError::FrameTooLarge));
    }

    #[test]
    fn test_invalid_config() {
        let mut config = GazellConfig::default();
        config.channel = 101; // Invalid

        let mut transport = GazellTransport::new(config);
        assert_eq!(transport.init(), Err(WirelessError::InvalidConfig));
    }
}
