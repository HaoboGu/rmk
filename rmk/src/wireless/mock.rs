//! Mock wireless transport for testing
//!
//! This module provides a mock implementation of `WirelessTransport`
//! that can be used for unit testing and integration testing without
//! real hardware.

use super::transport::{Result, WirelessError, WirelessTransport};
use heapless::Vec;

/// Mock wireless transport for testing
///
/// This transport simulates wireless communication by maintaining
/// send/receive queues. It can be configured to simulate packet loss,
/// latency, and other network conditions.
///
/// # Example
///
/// ```no_run
/// use rmk::wireless::{MockTransport, WirelessTransport};
///
/// let mut transport = MockTransport::new();
/// transport.set_packet_loss_rate(0.1); // 10% packet loss
///
/// // Simulate sending a frame
/// let frame = [0xAA, 0xBB, 0xCC];
/// transport.send_frame(&frame).unwrap();
///
/// // Simulate receiving it (if not lost)
/// if let Some(received) = transport.recv_frame().unwrap() {
///     assert_eq!(&received[..], &frame);
/// }
/// ```
pub struct MockTransport {
    /// Queue of sent frames (visible for testing)
    pub send_queue: Vec<Vec<u8, 64>, 16>,

    /// Queue of frames to be received
    pub recv_queue: Vec<Vec<u8, 64>, 16>,

    /// Packet loss rate (0.0-1.0)
    packet_loss_rate: f32,

    /// Total frames sent
    pub frames_sent: usize,

    /// Total frames received
    pub frames_received: usize,

    /// Total frames dropped due to packet loss
    pub frames_dropped: usize,

    /// Whether transport is initialized
    initialized: bool,

    /// Maximum frame size
    max_size: usize,
}

impl MockTransport {
    /// Create a new mock transport
    pub fn new() -> Self {
        Self {
            send_queue: Vec::new(),
            recv_queue: Vec::new(),
            packet_loss_rate: 0.0,
            frames_sent: 0,
            frames_received: 0,
            frames_dropped: 0,
            initialized: true,
            max_size: 64,
        }
    }

    /// Set packet loss rate (0.0 = no loss, 1.0 = all packets lost)
    pub fn set_packet_loss_rate(&mut self, rate: f32) {
        self.packet_loss_rate = rate.clamp(0.0, 1.0);
    }

    /// Set maximum frame size
    pub fn set_max_size(&mut self, size: usize) {
        self.max_size = size;
    }

    /// Simulate receiving a frame from remote
    ///
    /// This adds a frame to the receive queue, simulating an incoming packet.
    pub fn simulate_receive(&mut self, frame: &[u8]) -> Result<()> {
        let mut vec = Vec::new();
        for byte in frame {
            vec.push(*byte).map_err(|_| WirelessError::FrameTooLarge)?;
        }

        self.recv_queue
            .push(vec)
            .map_err(|_| WirelessError::Busy)?;

        Ok(())
    }

    /// Get sent frame at index (for testing)
    pub fn get_sent_frame(&self, index: usize) -> Option<&Vec<u8, 64>> {
        self.send_queue.get(index)
    }

    /// Clear all queues
    pub fn clear(&mut self) {
        self.send_queue.clear();
        self.recv_queue.clear();
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.frames_sent = 0;
        self.frames_received = 0;
        self.frames_dropped = 0;
    }

    /// Simulate packet loss (using simple deterministic approach)
    fn should_drop_packet(&self) -> bool {
        if self.packet_loss_rate == 0.0 {
            return false;
        }

        // Simple deterministic loss: drop every Nth packet
        // where N = 1 / packet_loss_rate
        if self.packet_loss_rate >= 1.0 {
            return true;
        }

        let n = (1.0 / self.packet_loss_rate) as usize;
        self.frames_sent % n == 0
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl WirelessTransport for MockTransport {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        if frame.len() > self.max_size {
            return Err(WirelessError::FrameTooLarge);
        }

        // Simulate packet loss
        if self.should_drop_packet() {
            self.frames_dropped += 1;
            return Err(WirelessError::SendFailed);
        }

        // Convert to Vec
        let mut vec = Vec::new();
        for byte in frame {
            vec.push(*byte).map_err(|_| WirelessError::FrameTooLarge)?;
        }

        // Add to send queue
        self.send_queue
            .push(vec)
            .map_err(|_| WirelessError::Busy)?;

        self.frames_sent += 1;
        Ok(())
    }

    fn recv_frame(&mut self) -> Result<Option<Vec<u8, 64>>> {
        if !self.initialized {
            return Err(WirelessError::NotInitialized);
        }

        // Pop from receive queue
        if let Some(frame) = self.recv_queue.pop() {
            self.frames_received += 1;
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    fn is_ready(&self) -> bool {
        self.initialized
    }

    fn max_frame_size(&self) -> usize {
        self.max_size
    }

    fn flush(&mut self) -> Result<()> {
        self.send_queue.clear();
        Ok(())
    }
}

/// Pair of mock transports for testing bidirectional communication
///
/// This creates two connected mock transports where sending on one
/// automatically makes the frame available for receiving on the other.
///
/// # Example
///
/// ```no_run
/// use rmk::wireless::{MockTransportPair, WirelessTransport};
///
/// let mut pair = MockTransportPair::new();
///
/// // Send from keyboard
/// pair.keyboard.send_frame(&[0xAA, 0xBB]).unwrap();
///
/// // Receive on dongle
/// let frame = pair.dongle.recv_frame().unwrap().unwrap();
/// assert_eq!(&frame[..], &[0xAA, 0xBB]);
/// ```
pub struct MockTransportPair {
    pub keyboard: MockTransport,
    pub dongle: MockTransport,
}

impl MockTransportPair {
    /// Create a new pair of connected transports
    pub fn new() -> Self {
        Self {
            keyboard: MockTransport::new(),
            dongle: MockTransport::new(),
        }
    }

    /// Set packet loss rate for both transports
    pub fn set_packet_loss_rate(&mut self, rate: f32) {
        self.keyboard.set_packet_loss_rate(rate);
        self.dongle.set_packet_loss_rate(rate);
    }

    /// Transfer pending frames from keyboard to dongle
    ///
    /// This simulates the wireless link by moving frames from
    /// keyboard's send queue to dongle's receive queue.
    pub fn transfer_keyboard_to_dongle(&mut self) -> Result<usize> {
        let mut count = 0;
        while let Some(frame) = self.keyboard.send_queue.pop() {
            // Convert Vec<u8, 64> to &[u8]
            let slice: &[u8] = &frame;
            self.dongle.simulate_receive(slice)?;
            count += 1;
        }
        Ok(count)
    }

    /// Transfer pending frames from dongle to keyboard
    pub fn transfer_dongle_to_keyboard(&mut self) -> Result<usize> {
        let mut count = 0;
        while let Some(frame) = self.dongle.send_queue.pop() {
            let slice: &[u8] = &frame;
            self.keyboard.simulate_receive(slice)?;
            count += 1;
        }
        Ok(count)
    }

    /// Transfer frames in both directions
    pub fn transfer_both(&mut self) -> Result<(usize, usize)> {
        let k_to_d = self.transfer_keyboard_to_dongle()?;
        let d_to_k = self.transfer_dongle_to_keyboard()?;
        Ok((k_to_d, d_to_k))
    }
}

impl Default for MockTransportPair {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_transport_send_recv() {
        let mut transport = MockTransport::new();

        // Send a frame
        let frame = [0xAA, 0xBB, 0xCC];
        assert!(transport.send_frame(&frame).is_ok());
        assert_eq!(transport.frames_sent, 1);

        // Check it's in send queue
        assert_eq!(transport.send_queue.len(), 1);
        assert_eq!(&transport.send_queue[0][..], &frame);

        // Simulate receiving
        transport.simulate_receive(&[0xDD, 0xEE]).unwrap();
        let received = transport.recv_frame().unwrap().unwrap();
        assert_eq!(&received[..], &[0xDD, 0xEE]);
        assert_eq!(transport.frames_received, 1);
    }

    #[test]
    fn test_mock_transport_packet_loss() {
        let mut transport = MockTransport::new();
        transport.set_packet_loss_rate(1.0); // Drop all packets

        let frame = [0xAA, 0xBB];
        assert!(transport.send_frame(&frame).is_err());
        assert_eq!(transport.frames_dropped, 1);
        assert_eq!(transport.send_queue.len(), 0); // Not queued
    }

    #[test]
    fn test_mock_transport_max_size() {
        let mut transport = MockTransport::new();
        transport.set_max_size(4);

        let small_frame = [0xAA, 0xBB];
        assert!(transport.send_frame(&small_frame).is_ok());

        let large_frame = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
        assert_eq!(
            transport.send_frame(&large_frame),
            Err(WirelessError::FrameTooLarge)
        );
    }

    #[test]
    fn test_mock_transport_pair() {
        let mut pair = MockTransportPair::new();

        // Keyboard sends to dongle
        pair.keyboard.send_frame(&[0xAA, 0xBB]).unwrap();
        assert_eq!(pair.keyboard.send_queue.len(), 1);

        // Transfer
        let count = pair.transfer_keyboard_to_dongle().unwrap();
        assert_eq!(count, 1);
        assert_eq!(pair.keyboard.send_queue.len(), 0);
        assert_eq!(pair.dongle.recv_queue.len(), 1);

        // Dongle receives
        let frame = pair.dongle.recv_frame().unwrap().unwrap();
        assert_eq!(&frame[..], &[0xAA, 0xBB]);
    }

    #[test]
    fn test_mock_transport_pair_bidirectional() {
        let mut pair = MockTransportPair::new();

        // Keyboard to dongle
        pair.keyboard.send_frame(&[0x01, 0x02]).unwrap();

        // Dongle to keyboard
        pair.dongle.send_frame(&[0x03, 0x04]).unwrap();

        // Transfer both
        let (k_to_d, d_to_k) = pair.transfer_both().unwrap();
        assert_eq!(k_to_d, 1);
        assert_eq!(d_to_k, 1);

        // Verify
        let frame1 = pair.dongle.recv_frame().unwrap().unwrap();
        assert_eq!(&frame1[..], &[0x01, 0x02]);

        let frame2 = pair.keyboard.recv_frame().unwrap().unwrap();
        assert_eq!(&frame2[..], &[0x03, 0x04]);
    }

    #[test]
    fn test_mock_transport_clear() {
        let mut transport = MockTransport::new();
        transport.send_frame(&[0xAA]).unwrap();
        transport.simulate_receive(&[0xBB]).unwrap();

        assert_eq!(transport.send_queue.len(), 1);
        assert_eq!(transport.recv_queue.len(), 1);

        transport.clear();

        assert_eq!(transport.send_queue.len(), 0);
        assert_eq!(transport.recv_queue.len(), 0);
    }

    #[test]
    fn test_mock_transport_stats() {
        let mut transport = MockTransport::new();

        transport.send_frame(&[0xAA]).unwrap();
        transport.send_frame(&[0xBB]).unwrap();
        assert_eq!(transport.frames_sent, 2);

        transport.simulate_receive(&[0xCC]).unwrap();
        transport.recv_frame().unwrap();
        assert_eq!(transport.frames_received, 1);

        transport.reset_stats();
        assert_eq!(transport.frames_sent, 0);
        assert_eq!(transport.frames_received, 0);
    }
}
