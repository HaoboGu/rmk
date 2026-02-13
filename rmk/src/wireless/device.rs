//! Multi-device addressing and management
//!
//! This module provides device addressing capabilities for multi-device
//! wireless scenarios (e.g., multiple keyboards connected to one dongle).

use heapless::Vec;

/// Maximum number of devices that can be connected simultaneously
pub const MAX_DEVICES: usize = 8;

/// Device address for multi-device scenarios
///
/// Each device has a unique identifier and is assigned to a specific
/// wireless channel/pipe.
///
/// # Example
///
/// ```no_run
/// use rmk::wireless::DeviceAddress;
///
/// let device = DeviceAddress {
///     device_id: 0x1234,
///     pipe: 0,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceAddress {
    /// Unique device identifier (0x0000-0xFFFF)
    ///
    /// This ID should be persistent across reboots and uniquely
    /// identify the keyboard device.
    pub device_id: u16,

    /// Wireless pipe/channel number
    ///
    /// For Nordic Gazell: pipe 0-7
    /// For other protocols: implementation-defined
    pub pipe: u8,
}

impl DeviceAddress {
    /// Create a new device address
    pub fn new(device_id: u16, pipe: u8) -> Self {
        Self { device_id, pipe }
    }

    /// Check if device address is valid
    pub fn is_valid(&self) -> bool {
        self.device_id != 0x0000 && self.device_id != 0xFFFF
    }

    /// Get broadcast address (targets all devices)
    pub fn broadcast() -> Self {
        Self {
            device_id: 0xFFFF,
            pipe: 0xFF,
        }
    }
}

/// Multi-device frame wrapper
///
/// Wraps an Elink frame with device addressing information.
/// This is used when multiple devices share the same wireless channel.
///
/// # Frame Format
///
/// ```text
/// +----------------+----------------+------------------+
/// | Device ID (2B) | Frame Len (1B) | Frame Data (N B) |
/// +----------------+----------------+------------------+
/// ```
#[derive(Debug, Clone)]
pub struct MultiDeviceFrame {
    /// Source/destination device address
    pub device_addr: DeviceAddress,

    /// Frame payload (typically an Elink StandardFrame)
    pub payload: Vec<u8, 64>,
}

impl MultiDeviceFrame {
    /// Create a new multi-device frame
    pub fn new(device_addr: DeviceAddress, payload: Vec<u8, 64>) -> Self {
        Self {
            device_addr,
            payload,
        }
    }

    /// Serialize frame to bytes
    ///
    /// # Returns
    ///
    /// Serialized frame: [device_id_hi, device_id_lo, len, ...payload]
    pub fn serialize(&self) -> Result<Vec<u8, 67>, ()> {
        let mut buf = Vec::new();

        // Device ID (big-endian)
        buf.push((self.device_addr.device_id >> 8) as u8)
            .map_err(|_| ())?;
        buf.push((self.device_addr.device_id & 0xFF) as u8)
            .map_err(|_| ())?;

        // Payload length
        buf.push(self.payload.len() as u8).map_err(|_| ())?;

        // Payload data
        for byte in &self.payload {
            buf.push(*byte).map_err(|_| ())?;
        }

        Ok(buf)
    }

    /// Deserialize frame from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, ()> {
        if data.len() < 3 {
            return Err(());
        }

        // Parse device ID
        let device_id = ((data[0] as u16) << 8) | (data[1] as u16);
        let len = data[2] as usize;

        if data.len() < 3 + len {
            return Err(());
        }

        // Parse payload
        let mut payload = Vec::new();
        for i in 0..len {
            payload.push(data[3 + i]).map_err(|_| ())?;
        }

        Ok(Self {
            device_addr: DeviceAddress {
                device_id,
                pipe: 0, // Pipe is determined by receiver
            },
            payload,
        })
    }
}

/// Connected device state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    /// Device is disconnected
    Disconnected,

    /// Device is connecting
    Connecting,

    /// Device is connected and active
    Connected,

    /// Device connection is lost (but not explicitly disconnected)
    Lost,
}

/// Information about a connected device
#[derive(Debug, Clone)]
pub struct ConnectedDevice {
    /// Device address
    pub address: DeviceAddress,

    /// Current connection state
    pub state: DeviceState,

    /// Last received packet timestamp (milliseconds)
    pub last_seen_ms: u64,

    /// Signal strength (RSSI in dBm, if available)
    pub rssi: Option<i8>,
}

impl ConnectedDevice {
    /// Create a new connected device entry
    pub fn new(address: DeviceAddress) -> Self {
        Self {
            address,
            state: DeviceState::Connecting,
            last_seen_ms: 0,
            rssi: None,
        }
    }

    /// Check if device is active (connected or connecting)
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            DeviceState::Connected | DeviceState::Connecting
        )
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self, timestamp_ms: u64) {
        self.last_seen_ms = timestamp_ms;
        if self.state == DeviceState::Connecting || self.state == DeviceState::Lost {
            self.state = DeviceState::Connected;
        }
    }

    /// Check if device timed out
    pub fn is_timed_out(&self, current_time_ms: u64, timeout_ms: u64) -> bool {
        current_time_ms.saturating_sub(self.last_seen_ms) > timeout_ms
    }
}

/// Multi-device manager
///
/// Manages multiple connected devices on the dongle side.
///
/// # Example
///
/// ```no_run
/// use rmk::wireless::{DeviceManager, DeviceAddress};
///
/// let mut manager = DeviceManager::new();
/// manager.register_device(DeviceAddress::new(0x1234, 0));
/// ```
pub struct DeviceManager {
    devices: [Option<ConnectedDevice>; MAX_DEVICES],
    device_timeout_ms: u64,
}

impl DeviceManager {
    /// Create a new device manager
    pub fn new() -> Self {
        Self {
            devices: [None, None, None, None, None, None, None, None],
            device_timeout_ms: 5000, // 5 seconds default timeout
        }
    }

    /// Set device timeout (milliseconds)
    pub fn set_timeout(&mut self, timeout_ms: u64) {
        self.device_timeout_ms = timeout_ms;
    }

    /// Register a new device
    pub fn register_device(&mut self, address: DeviceAddress) -> Result<(), ()> {
        // Check if already registered
        for device in &self.devices {
            if let Some(dev) = device {
                if dev.address.device_id == address.device_id {
                    return Ok(()); // Already registered
                }
            }
        }

        // Find empty slot
        for slot in &mut self.devices {
            if slot.is_none() {
                *slot = Some(ConnectedDevice::new(address));
                return Ok(());
            }
        }

        Err(()) // No slots available
    }

    /// Unregister a device
    pub fn unregister_device(&mut self, device_id: u16) {
        for slot in &mut self.devices {
            if let Some(dev) = slot {
                if dev.address.device_id == device_id {
                    *slot = None;
                    return;
                }
            }
        }
    }

    /// Get device by ID
    pub fn get_device(&self, device_id: u16) -> Option<&ConnectedDevice> {
        for device in &self.devices {
            if let Some(dev) = device {
                if dev.address.device_id == device_id {
                    return Some(dev);
                }
            }
        }
        None
    }

    /// Get mutable device by ID
    pub fn get_device_mut(&mut self, device_id: u16) -> Option<&mut ConnectedDevice> {
        for device in &mut self.devices {
            if let Some(dev) = device {
                if dev.address.device_id == device_id {
                    return Some(dev);
                }
            }
        }
        None
    }

    /// Update device activity
    pub fn update_device(&mut self, device_id: u16, timestamp_ms: u64, rssi: Option<i8>) {
        if let Some(device) = self.get_device_mut(device_id) {
            device.update_last_seen(timestamp_ms);
            device.rssi = rssi;
        }
    }

    /// Check for timed out devices
    pub fn check_timeouts(&mut self, current_time_ms: u64) {
        for slot in &mut self.devices {
            if let Some(device) = slot {
                if device.is_active() && device.is_timed_out(current_time_ms, self.device_timeout_ms)
                {
                    device.state = DeviceState::Lost;
                }
            }
        }
    }

    /// Get list of active devices
    pub fn active_devices(&self) -> Vec<DeviceAddress, MAX_DEVICES> {
        let mut list = Vec::new();
        for device in &self.devices {
            if let Some(dev) = device {
                if dev.is_active() {
                    let _ = list.push(dev.address);
                }
            }
        }
        list
    }

    /// Get number of connected devices
    pub fn connected_count(&self) -> usize {
        self.devices
            .iter()
            .filter(|d| d.as_ref().map_or(false, |dev| dev.state == DeviceState::Connected))
            .count()
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_address_valid() {
        assert!(DeviceAddress::new(0x1234, 0).is_valid());
        assert!(!DeviceAddress::new(0x0000, 0).is_valid());
        assert!(!DeviceAddress::new(0xFFFF, 0).is_valid());
    }

    #[test]
    fn test_broadcast_address() {
        let broadcast = DeviceAddress::broadcast();
        assert_eq!(broadcast.device_id, 0xFFFF);
        assert_eq!(broadcast.pipe, 0xFF);
    }

    #[test]
    fn test_multi_device_frame_serialize() {
        let addr = DeviceAddress::new(0x1234, 0);
        let mut payload = Vec::new();
        payload.push(0xAA).unwrap();
        payload.push(0xBB).unwrap();

        let frame = MultiDeviceFrame::new(addr, payload);
        let serialized = frame.serialize().unwrap();

        assert_eq!(serialized[0], 0x12); // Device ID high
        assert_eq!(serialized[1], 0x34); // Device ID low
        assert_eq!(serialized[2], 2); // Length
        assert_eq!(serialized[3], 0xAA); // Payload
        assert_eq!(serialized[4], 0xBB);
    }

    #[test]
    fn test_multi_device_frame_deserialize() {
        let data = [0x12, 0x34, 0x02, 0xAA, 0xBB];
        let frame = MultiDeviceFrame::deserialize(&data).unwrap();

        assert_eq!(frame.device_addr.device_id, 0x1234);
        assert_eq!(frame.payload.len(), 2);
        assert_eq!(frame.payload[0], 0xAA);
        assert_eq!(frame.payload[1], 0xBB);
    }

    #[test]
    fn test_device_manager_register() {
        let mut manager = DeviceManager::new();
        let addr = DeviceAddress::new(0x1234, 0);

        assert!(manager.register_device(addr).is_ok());
        assert_eq!(manager.connected_count(), 0); // Still connecting
        assert!(manager.get_device(0x1234).is_some());
    }

    #[test]
    fn test_device_manager_update() {
        let mut manager = DeviceManager::new();
        let addr = DeviceAddress::new(0x1234, 0);

        manager.register_device(addr).unwrap();
        manager.update_device(0x1234, 1000, Some(-50));

        let device = manager.get_device(0x1234).unwrap();
        assert_eq!(device.state, DeviceState::Connected);
        assert_eq!(device.last_seen_ms, 1000);
        assert_eq!(device.rssi, Some(-50));
    }

    #[test]
    fn test_device_manager_timeout() {
        let mut manager = DeviceManager::new();
        manager.set_timeout(1000);

        let addr = DeviceAddress::new(0x1234, 0);
        manager.register_device(addr).unwrap();
        manager.update_device(0x1234, 1000, None);

        // Not timed out yet
        manager.check_timeouts(1500);
        assert_eq!(
            manager.get_device(0x1234).unwrap().state,
            DeviceState::Connected
        );

        // Timed out
        manager.check_timeouts(2500);
        assert_eq!(
            manager.get_device(0x1234).unwrap().state,
            DeviceState::Lost
        );
    }

    #[test]
    fn test_device_manager_max_devices() {
        let mut manager = DeviceManager::new();

        // Register MAX_DEVICES devices
        for i in 0..MAX_DEVICES {
            let addr = DeviceAddress::new(0x1000 + i as u16, i as u8);
            assert!(manager.register_device(addr).is_ok());
        }

        // Next registration should fail
        let addr = DeviceAddress::new(0x2000, 0);
        assert!(manager.register_device(addr).is_err());
    }
}
