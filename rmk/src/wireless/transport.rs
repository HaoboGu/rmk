//! Wireless transport trait and common types

use core::fmt;

/// Errors that can occur during wireless communication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WirelessError {
    /// Failed to send frame
    SendFailed,

    /// Failed to receive frame
    ReceiveFailed,

    /// Frame too large for transport
    FrameTooLarge,

    /// Transport not initialized
    NotInitialized,

    /// Transport is busy
    Busy,

    /// No data available
    NoData,

    /// Invalid configuration
    InvalidConfig,

    /// Hardware error
    HardwareError,
}

impl fmt::Display for WirelessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SendFailed => write!(f, "Failed to send frame"),
            Self::ReceiveFailed => write!(f, "Failed to receive frame"),
            Self::FrameTooLarge => write!(f, "Frame too large"),
            Self::NotInitialized => write!(f, "Transport not initialized"),
            Self::Busy => write!(f, "Transport busy"),
            Self::NoData => write!(f, "No data available"),
            Self::InvalidConfig => write!(f, "Invalid configuration"),
            Self::HardwareError => write!(f, "Hardware error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WirelessError {}

/// Result type for wireless operations
pub type Result<T> = core::result::Result<T, WirelessError>;

/// Trait for wireless transport implementations
///
/// This trait provides a transport-agnostic interface for sending and receiving
/// frames over a wireless link. Implementations handle the low-level protocol
/// details (e.g., Nordic Gazell, proprietary 2.4GHz).
///
/// # Frame Format
///
/// The transport layer is agnostic to frame content. It can carry:
/// - Raw Elink protocol frames
/// - Custom application frames
/// - Any byte payload up to max frame size
///
/// # Example Implementation
///
/// ```ignore
/// struct MyTransport { /* ... */ }
///
/// impl WirelessTransport for MyTransport {
///     fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
///         // Send frame over wireless link
///         // ...
///         Ok(())
///     }
///
///     fn recv_frame(&mut self) -> Result<Option<heapless::Vec<u8, 64>>> {
///         // Check for received frame
///         // ...
///         Ok(Some(received_frame))
///     }
/// }
/// ```
pub trait WirelessTransport {
    /// Send a frame over the wireless link
    ///
    /// # Arguments
    ///
    /// * `frame` - Frame data to send (typically an Elink frame)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Frame sent successfully
    /// * `Err(WirelessError)` - Send failed
    ///
    /// # Notes
    ///
    /// - This may block until the frame is transmitted
    /// - Retry logic should be handled by the implementation
    /// - Frame size must not exceed max frame size (typically 64 bytes)
    fn send_frame(&mut self, frame: &[u8]) -> Result<()>;

    /// Attempt to receive a frame from the wireless link
    ///
    /// # Returns
    ///
    /// * `Ok(Some(frame))` - Frame received
    /// * `Ok(None)` - No frame available
    /// * `Err(WirelessError)` - Receive error
    ///
    /// # Notes
    ///
    /// - This should be non-blocking (poll-based)
    /// - Returns None if no frame is available
    /// - Frame is validated by the transport (CRC, etc.)
    fn recv_frame(&mut self) -> Result<Option<heapless::Vec<u8, 64>>>;

    /// Check if the transport is ready to send
    ///
    /// # Returns
    ///
    /// * `true` - Ready to send
    /// * `false` - Busy, cannot send now
    fn is_ready(&self) -> bool {
        true // Default implementation
    }

    /// Get the maximum frame size supported by this transport
    ///
    /// # Returns
    ///
    /// Maximum frame size in bytes (default: 64 for Nordic Gazell)
    fn max_frame_size(&self) -> usize {
        64
    }

    /// Flush any pending operations
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Flush successful
    /// * `Err(WirelessError)` - Flush failed
    fn flush(&mut self) -> Result<()> {
        Ok(()) // Default no-op
    }
}

/// Async version of WirelessTransport
///
/// For async/await environments (Embassy, Tokio, etc.)
#[cfg(feature = "async")]
pub trait WirelessTransportAsync {
    /// Send a frame asynchronously
    async fn send_frame(&mut self, frame: &[u8]) -> Result<()>;

    /// Receive a frame asynchronously
    async fn recv_frame(&mut self) -> Result<Option<heapless::Vec<u8, 64>>>;
}
