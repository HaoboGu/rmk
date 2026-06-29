//! Strict Rynk behavior verifier for the deterministic QEMU fixture.
//!
//! Unlike `hw_test`, this is not a generic hardware smoke test. It assumes the
//! fixture in `examples/use_rust/qemu-riscv-rynk` and asserts exact protocol
//! behavior: expected capabilities, known initial state, mutations, restore
//! paths, and firmware-side rejection behavior.

use std::fmt::Debug;
use std::io::ErrorKind;
use std::time::Duration;

use rynk::io::{Read, Write};
use rynk::rmk_types::action::{Action, EncoderAction, KeyAction};
use rynk::rmk_types::ble::{BleState, BleStatus};
use rynk::rmk_types::combo::Combo;
use rynk::rmk_types::connection::{ConnectionStatus, ConnectionType, UsbState};
use rynk::rmk_types::fork::{Fork, StateBits};
use rynk::rmk_types::keycode::{HidKeyCode, KeyCode};
use rynk::rmk_types::led_indicator::LedIndicator;
use rynk::rmk_types::modifier::ModifierCombination;
use rynk::rmk_types::morse::{Morse, MorseProfile};
use rynk::rmk_types::protocol::rynk::{
    Cmd, GetComboBulkRequest, GetComboBulkResponse, GetKeymapBulkRequest, GetKeymapBulkResponse, GetMorseBulkRequest,
    GetMorseBulkResponse, MacroData, ProtocolVersion, RynkError, SetComboBulkRequest, SetKeymapBulkRequest,
    SetMorseBulkRequest, StorageResetMode,
};
use rynk::{Client, RequestError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_ADDR: &str = "127.0.0.1:9000";

struct TcpTransport {
    stream: tokio::net::TcpStream,
}

impl rynk::io::ErrorType for TcpTransport {
    type Error = std::io::Error;
}

impl Read for TcpTransport {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            self.stream.readable().await?;
            match self.stream.try_read(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
    }
}

impl Write for TcpTransport {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            self.stream.writable().await?;
            match self.stream.try_write(buf) {
                Ok(n) => return Ok(n),
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn key(code: HidKeyCode) -> KeyAction {
    KeyAction::Single(Action::Key(KeyCode::Hid(code)))
}

fn encoder(clockwise: HidKeyCode, counter_clockwise: HidKeyCode) -> EncoderAction {
    EncoderAction::new(key(clockwise), key(counter_clockwise))
}

fn fixture_fork(positive_output: KeyAction) -> Fork {
    Fork::new(
        key(HidKeyCode::A),
        key(HidKeyCode::B),
        positive_output,
        StateBits::default(),
        StateBits::default(),
        ModifierCombination::default(),
        true,
    )
}

fn empty_morse(profile: MorseProfile) -> Morse {
    Morse {
        profile,
        actions: heapless::LinearMap::new(),
    }
}

fn expect_rejected<T: Debug>(label: &str, res: Result<T, RequestError>, expected: RynkError) {
    match res {
        Err(RequestError::Rejected(actual)) if actual == expected => {}
        other => panic!("{label}: expected Rejected({expected:?}), got {other:?}"),
    }
}

fn expect_unsupported<T: Debug>(label: &str, res: Result<T, RequestError>) {
    match res {
        Err(RequestError::Unsupported(_, _)) => {}
        other => panic!("{label}: expected Unsupported, got {other:?}"),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_ADDR.into());
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("connect timed out to QEMU TCP serial at {addr}"))??;
    stream.set_nodelay(true)?;
    let mut client = tokio::time::timeout(CONNECT_TIMEOUT, Client::connect(TcpTransport { stream }))
        .await
        .map_err(|_| format!("Rynk handshake timed out over QEMU TCP serial at {addr}"))??;

    assert_eq!(client.protocol_version(), ProtocolVersion::CURRENT);
    let caps = *client.capabilities();
    assert_eq!((caps.num_layers, caps.num_rows, caps.num_cols), (2, 3, 3));
    assert_eq!(caps.num_encoders, 1);
    assert_eq!(caps.max_combos, 8);
    assert_eq!(caps.max_combo_keys, 4);
    assert_eq!(caps.max_macros, 16);
    assert_eq!(caps.macro_space_size, 256);
    assert_eq!(caps.macro_chunk_size, 64);
    assert_eq!(caps.max_morse, 8);
    assert_eq!(caps.max_patterns_per_key, 8);
    assert_eq!(caps.max_forks, 8);
    assert!(!caps.storage_enabled);
    assert!(!caps.ble_enabled);
    assert!(!caps.is_split);
    assert!(!caps.bulk_transfer_supported);

    assert_eq!(client.get_version().await?, ProtocolVersion::CURRENT);
    assert_eq!(client.get_capabilities().await?, caps);

    assert_eq!(client.get_default_layer().await?, 0);
    client.set_default_layer(1).await?;
    assert_eq!(client.get_default_layer().await?, 1);
    expect_rejected(
        "set_default_layer out of range",
        client.set_default_layer(2).await,
        RynkError::Invalid,
    );
    client.set_default_layer(0).await?;
    assert_eq!(client.get_default_layer().await?, 0);

    let expected_layers = [
        [
            [key(HidKeyCode::Kp1), key(HidKeyCode::Kp2), key(HidKeyCode::Kp3)],
            [key(HidKeyCode::Kp4), key(HidKeyCode::Kp5), key(HidKeyCode::Kp6)],
            [key(HidKeyCode::Kp7), key(HidKeyCode::Kp8), key(HidKeyCode::Kp9)],
        ],
        [
            [key(HidKeyCode::A), key(HidKeyCode::B), key(HidKeyCode::C)],
            [key(HidKeyCode::D), key(HidKeyCode::E), key(HidKeyCode::F)],
            [key(HidKeyCode::G), key(HidKeyCode::H), key(HidKeyCode::I)],
        ],
    ];
    for (layer, rows) in expected_layers.iter().enumerate() {
        for (row, cols) in rows.iter().enumerate() {
            for (col, action) in cols.iter().enumerate() {
                assert_eq!(client.get_key(layer as u8, row as u8, col as u8).await?, *action);
            }
        }
    }

    expect_rejected(
        "get_key row out of range",
        client.get_key(0, 3, 0).await,
        RynkError::Invalid,
    );
    expect_rejected(
        "set_key layer out of range",
        client.set_key(2, 0, 0, KeyAction::No).await,
        RynkError::Invalid,
    );
    client.set_key(0, 0, 0, key(HidKeyCode::Kp2)).await?;
    assert_eq!(client.get_key(0, 0, 0).await?, key(HidKeyCode::Kp2));
    client.set_key(0, 0, 0, key(HidKeyCode::Kp1)).await?;
    assert_eq!(client.get_key(0, 0, 0).await?, key(HidKeyCode::Kp1));

    assert_eq!(
        client.get_encoder(0, 0).await?,
        encoder(HidKeyCode::KpPlus, HidKeyCode::KpMinus)
    );
    expect_rejected(
        "get_encoder id out of range",
        client.get_encoder(1, 0).await,
        RynkError::Invalid,
    );
    let swapped_encoder = encoder(HidKeyCode::KpMinus, HidKeyCode::KpPlus);
    client.set_encoder(0, 0, swapped_encoder).await?;
    assert_eq!(client.get_encoder(0, 0).await?, swapped_encoder);
    client
        .set_encoder(0, 0, encoder(HidKeyCode::KpPlus, HidKeyCode::KpMinus))
        .await?;

    let mut macro_bytes = heapless::Vec::new();
    macro_bytes.extend_from_slice(&[1, 2, 3, 4]).unwrap();
    client.set_macro(0, 0, MacroData { data: macro_bytes }).await?;
    let got_macro = client.get_macro(0, 0).await?;
    assert_eq!(got_macro.data.len(), caps.macro_chunk_size as usize);
    assert_eq!(&got_macro.data[..4], &[1, 2, 3, 4]);
    assert!(got_macro.data[4..].iter().all(|&b| b == 0));

    assert_eq!(client.get_combo(0).await?, Combo::empty());
    let changed_combo = Combo::new(
        [key(HidKeyCode::Kp1), key(HidKeyCode::Kp4)],
        key(HidKeyCode::Kp1),
        Some(1),
    );
    client.set_combo(0, changed_combo.clone()).await?;
    assert_eq!(client.get_combo(0).await?, changed_combo);
    client.set_combo(0, Combo::empty()).await?;
    assert_eq!(client.get_combo(0).await?, Combo::empty());
    expect_rejected(
        "get_combo out of range",
        client.get_combo(250).await,
        RynkError::Invalid,
    );

    assert_eq!(client.get_fork(0).await?, fixture_fork(key(HidKeyCode::C)));
    let changed_fork = fixture_fork(key(HidKeyCode::Kp2));
    client.set_fork(0, changed_fork).await?;
    assert_eq!(client.get_fork(0).await?, changed_fork);
    client.set_fork(0, fixture_fork(key(HidKeyCode::C))).await?;
    expect_rejected("get_fork out of range", client.get_fork(250).await, RynkError::Invalid);

    assert_eq!(client.get_morse(0).await?, empty_morse(MorseProfile::const_default()));
    let changed_morse = empty_morse(MorseProfile::const_default().with_hold_timeout_ms(Some(180)));
    client.set_morse(0, changed_morse.clone()).await?;
    assert_eq!(client.get_morse(0).await?, changed_morse);
    client.set_morse(0, empty_morse(MorseProfile::const_default())).await?;
    expect_rejected(
        "get_morse out of range",
        client.get_morse(250).await,
        RynkError::Invalid,
    );

    let mut behavior = client.get_behavior().await?;
    let original_behavior = behavior;
    behavior.combo_timeout_ms += 5;
    client.set_behavior(behavior).await?;
    assert_eq!(client.get_behavior().await?, behavior);
    client.set_behavior(original_behavior).await?;
    assert_eq!(client.get_behavior().await?, original_behavior);

    assert_eq!(client.get_current_layer().await?, 0);
    assert!(
        client
            .get_matrix_state()
            .await?
            .pressed_bitmap
            .iter()
            .all(|&byte| byte == 0)
    );
    assert_eq!(client.get_wpm().await?, 0);
    assert!(!client.get_sleep_state().await?);
    assert_eq!(client.get_led_indicator().await?, LedIndicator::default());
    assert_eq!(client.get_connection_type().await?, ConnectionType::Usb);
    assert_eq!(
        client.get_connection_status().await?,
        ConnectionStatus {
            usb: UsbState::Disabled,
            ble: BleStatus {
                profile: 0,
                state: BleState::Inactive,
            },
            preferred: ConnectionType::Usb,
        }
    );

    expect_unsupported("get_ble_status", client.get_ble_status().await);
    expect_unsupported("get_battery_status", client.get_battery_status().await);
    expect_unsupported("get_peripheral_status", client.get_peripheral_status(0).await);
    expect_unsupported(
        "storage_reset",
        client.storage_reset(StorageResetMode::LayoutOnly).await,
    );
    expect_unsupported("typed get_keymap_bulk", client.get_keymap_bulk(0, 0, 0, 1).await);

    expect_rejected(
        "raw get_keymap_bulk",
        client
            .request_raw::<_, GetKeymapBulkResponse>(
                Cmd::GetKeymapBulk,
                &GetKeymapBulkRequest {
                    layer: 0,
                    start_row: 0,
                    start_col: 0,
                    count: 1,
                },
            )
            .await,
        RynkError::Unimplemented,
    );
    let mut bulk_actions = heapless::Vec::new();
    bulk_actions.push(key(HidKeyCode::A)).unwrap();
    expect_rejected(
        "raw set_keymap_bulk",
        client
            .request_raw::<_, ()>(
                Cmd::SetKeymapBulk,
                &SetKeymapBulkRequest {
                    layer: 0,
                    start_row: 0,
                    start_col: 0,
                    actions: bulk_actions,
                },
            )
            .await,
        RynkError::Unimplemented,
    );
    expect_rejected(
        "raw get_combo_bulk",
        client
            .request_raw::<_, GetComboBulkResponse>(
                Cmd::GetComboBulk,
                &GetComboBulkRequest {
                    start_index: 0,
                    count: 1,
                },
            )
            .await,
        RynkError::Unimplemented,
    );
    let mut bulk_combos = heapless::Vec::new();
    bulk_combos.push(Combo::empty()).unwrap();
    expect_rejected(
        "raw set_combo_bulk",
        client
            .request_raw::<_, ()>(
                Cmd::SetComboBulk,
                &SetComboBulkRequest {
                    start_index: 0,
                    configs: bulk_combos,
                },
            )
            .await,
        RynkError::Unimplemented,
    );
    expect_rejected(
        "raw get_morse_bulk",
        client
            .request_raw::<_, GetMorseBulkResponse>(
                Cmd::GetMorseBulk,
                &GetMorseBulkRequest {
                    start_index: 0,
                    count: 1,
                },
            )
            .await,
        RynkError::Unimplemented,
    );
    let mut bulk_morses = heapless::Vec::new();
    bulk_morses.push(empty_morse(MorseProfile::const_default())).unwrap();
    expect_rejected(
        "raw set_morse_bulk",
        client
            .request_raw::<_, ()>(
                Cmd::SetMorseBulk,
                &SetMorseBulkRequest {
                    start_index: 0,
                    configs: bulk_morses,
                },
            )
            .await,
        RynkError::Unimplemented,
    );

    println!("QEMU Rynk behavior verification passed.");
    Ok(())
}
