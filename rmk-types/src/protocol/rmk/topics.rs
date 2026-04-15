//! Topic declarations for the RMK protocol.
//!
//! Topics are server-to-client push messages (no request/response). They're
//! used for asynchronous events like layer changes, BLE state transitions, etc.

use postcard_rpc::{TopicDirection, topics};

use crate::connection::ConnectionType;
use crate::led_indicator::LedIndicator;
#[cfg(feature = "_ble")]
use crate::{battery::BatteryStatus, ble::BleStatus};

topics! {
    list = TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy               | MessageTy      | Path               |
    | -------               | ---------      | ----               |
    | LayerChangeTopic      | u8             | "event/layer"      |
    | WpmUpdateTopic        | u16            | "event/wpm"        |
    | ConnectionChangeTopic | ConnectionType | "event/connection" |
    | SleepStateTopic       | bool           | "event/sleep"      |
    | LedIndicatorTopic     | LedIndicator   | "event/led"        |
}

#[cfg(feature = "_ble")]
topics! {
    list = BLE_TOPICS_OUT_LIST;
    direction = TopicDirection::ToClient;
    | TopicTy              | MessageTy     | Path               |
    | -------              | ---------     | ----               |
    | BatteryStatusTopic   | BatteryStatus | "event/battery"    |
    | BleStatusChangeTopic | BleStatus     | "event/ble_status" |
}

#[cfg(test)]
mod tests {
    use postcard_rpc::Topic;

    use super::*;
    use crate::protocol::rmk::snapshot;

    /// Lock down topic schema fingerprints. Any change to a topic's message
    /// type changes its TOPIC_KEY — this snapshot fails when that happens.
    /// Update the snapshot intentionally with `UPDATE_SNAPSHOTS=1`.
    #[test]
    fn topic_keys_base_locked() {
        let entries: &[(&str, [u8; 8])] = &[
            (LayerChangeTopic::PATH, LayerChangeTopic::TOPIC_KEY.to_bytes()),
            (WpmUpdateTopic::PATH, WpmUpdateTopic::TOPIC_KEY.to_bytes()),
            (ConnectionChangeTopic::PATH, ConnectionChangeTopic::TOPIC_KEY.to_bytes()),
            (SleepStateTopic::PATH, SleepStateTopic::TOPIC_KEY.to_bytes()),
            (LedIndicatorTopic::PATH, LedIndicatorTopic::TOPIC_KEY.to_bytes()),
        ];
        let actual = snapshot::format_topic_keys("snapshots/topic_keys_base.snap", entries);
        snapshot::assert_snapshot("snapshots/topic_keys_base.snap", actual);
    }

    #[cfg(feature = "_ble")]
    #[test]
    fn topic_keys_ble_locked() {
        let entries: &[(&str, [u8; 8])] = &[
            (BatteryStatusTopic::PATH, BatteryStatusTopic::TOPIC_KEY.to_bytes()),
            (BleStatusChangeTopic::PATH, BleStatusChangeTopic::TOPIC_KEY.to_bytes()),
        ];
        let actual = snapshot::format_topic_keys("snapshots/topic_keys_ble.snap", entries);
        snapshot::assert_snapshot("snapshots/topic_keys_ble.snap", actual);
    }
}
