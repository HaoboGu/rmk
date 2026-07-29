//! Host protocol changes observed through a running keyboard.
//!
//! `rynk_loopback.rs` and `rynk_hid_loopback.rs` prove the protocol itself —
//! frames, error codes, framing — with no keyboard behind it. These cases
//! instead run the keyboard task and a host session together and interleave
//! them: a host writes config over the wire, then the matrix is driven and the
//! HID output must reflect the write. That is what proves the write path and
//! the keyboard's read path share one `KeyMap`, live, with no restart.

#![cfg(all(any(not(feature = "_no_usb"), feature = "_ble"), feature = "host"))]

pub mod common;

#[cfg(feature = "rynk")]
use rmk::config::RmkConfig;
#[cfg(feature = "vial")]
use rmk::encoder;
#[cfg(any(feature = "vial", feature = "rynk"))]
use rmk::types::action::EncoderAction;
#[cfg(feature = "rynk")]
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::HidKeyCode;
#[cfg(feature = "vial")]
use rmk::types::protocol::vial::SettingKey;
use rmk::{k, layer};
#[cfg(feature = "rynk")]
use rmk_types::combo::Combo;
#[cfg(feature = "rynk")]
use rmk_types::fork::{Fork, StateBits};
#[cfg(feature = "rynk")]
use rmk_types::modifier::ModifierCombination;
#[cfg(feature = "rynk")]
use rmk_types::morse::{Morse, MorseMode, MorseProfile};
#[cfg(feature = "rynk")]
use rmk_types::protocol::rynk::{
    BehaviorConfig as RynkBehaviorConfig, GetKeymapBulkRequest, GetKeymapBulkResponse, LayoutChunk, MacroData,
    SetComboRequest, SetForkRequest, SetKeymapBulkRequest, SetMacroRequest, SetMorseRequest, command,
};
#[cfg(feature = "vial")]
use rmk_types::protocol::vial::{VIA_PROTOCOL_VERSION, ViaCommand};

use crate::common::simulator::{SimHost, SimKeyboard};

#[cfg(feature = "vial")]
#[test]
fn via_protocol_version_round_trips() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;
        let host = SimHost::new();
        let mut expected = [0u8; 32];
        expected[0] = ViaCommand::GetProtocolVersion as u8;
        expected[1..3].copy_from_slice(&VIA_PROTOCOL_VERSION.to_be_bytes());

        host.vial(&mut keyboard).get_protocol_version().expect(expected);

        keyboard.run().await;
    });
}

#[cfg(feature = "vial")]
#[test]
fn via_keymap_write_changes_what_the_key_reports() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;
        let host = SimHost::new();

        host.vial(&mut keyboard).get_key(0, 0, 0).expect(k!(A));
        host.vial(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
        host.vial(&mut keyboard).get_key(0, 0, 0).expect(k!(B));

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::B])
            .delay(10)
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "vial")]
#[test]
fn vial_encoder_write_changes_what_the_knob_reports() {
    common::test_block_on(async {
        let encoder_action = encoder!(k!(C), k!(D));
        let mut keyboard = SimKeyboard::builder([[[k!(A)]]])
            .encoders([[encoder!(k!(A), k!(B))]])
            .build()
            .await;
        let host = SimHost::new();

        host.vial(&mut keyboard)
            .get_encoder(0, 0)
            .expect(encoder!(k!(A), k!(B)));
        host.vial(&mut keyboard).set_encoder(0, 0, encoder_action).expect_ok();
        host.vial(&mut keyboard).get_encoder(0, 0).expect(encoder_action);

        keyboard
            .rotary_cw(0)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .rotary_ccw(0)
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "vial")]
#[test]
fn vial_rejects_out_of_range_and_unknown_requests() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A)]]])
            .encoders([[encoder!(k!(A), k!(B))]])
            .build()
            .await;
        let host = SimHost::new();

        host.vial(&mut keyboard)
            .get_encoder(0, 99)
            .expect(EncoderAction::default());
        host.vial(&mut keyboard).unsupported_dynamic_entry().expect([0u8; 32]);

        keyboard.run().await;
    });
}

#[cfg(feature = "vial")]
#[test]
fn vial_combo_timeout_write_changes_when_the_combo_fires() {
    common::test_block_on(async {
        let keymap = [layer!([[k!(A), k!(B)]])];
        let mut keyboard = SimKeyboard::builder(keymap).build().await;
        let host = SimHost::new();

        host.vial(&mut keyboard)
            .get_behavior_setting(SettingKey::ComboTimeout)
            .expect_u16(50);
        host.vial(&mut keyboard)
            .set_behavior_setting_u16(SettingKey::ComboTimeout, 80)
            .expect_ok();
        host.vial(&mut keyboard)
            .get_behavior_setting(SettingKey::ComboTimeout)
            .expect_u16(80);
        host.vial(&mut keyboard).set_combo(0, [k!(A), k!(B)], k!(C)).expect_ok();

        keyboard
            .press(0, 0)
            .expect_no_report(60)
            .expect_keys([HidKeyCode::A])
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "vial")]
#[test]
fn vial_combo_write_makes_the_chord_fire() {
    common::test_block_on(async {
        let keymap = [layer!([[k!(A), k!(B)]])];
        let mut keyboard = SimKeyboard::builder(keymap).build().await;
        let host = SimHost::new();

        host.vial(&mut keyboard).set_combo(0, [k!(A), k!(B)], k!(C)).expect_ok();
        host.vial(&mut keyboard).get_combo(0).expect([k!(A), k!(B)], k!(C));

        keyboard
            .press(0, 0)
            .expect_no_report(5)
            .press(0, 1)
            .expect_keys([HidKeyCode::C])
            .release(0, 0)
            .release(0, 1)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "vial")]
#[test]
fn vial_morse_write_changes_what_the_tap_reports() {
    common::test_block_on(async {
        let keymap = [layer!([[rmk::td!(0)]])];
        let mut keyboard = SimKeyboard::builder(keymap).build().await;
        let host = SimHost::new();

        host.vial(&mut keyboard)
            .set_morse(0, k!(A), k!(B), k!(C), k!(D), 80)
            .expect_ok();
        host.vial(&mut keyboard)
            .get_morse(0)
            .expect(k!(A), k!(B), k!(C), k!(D), 80);

        keyboard
            .delay(150)
            .press(0, 0)
            .delay(20)
            .release(0, 0)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(all(feature = "storage", feature = "vial", feature = "host"))]
#[test]
fn vial_keymap_write_survives_a_restart() {
    common::test_block_on(async {
        let flash = crate::common::simulator::flash::InMemoryFlash::<4096, 256, 4>::new();
        let host = SimHost::new();

        {
            let mut keyboard = SimKeyboard::builder([[[k!(A)]]])
                .storage_flash(flash.clone())
                .build()
                .await;

            host.vial(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
            keyboard.wait_storage().run().await;
        }

        let mut keyboard = SimKeyboard::builder([[[k!(A)]]])
            .storage_flash(flash.clone())
            .build()
            .await;

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::B])
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(all(feature = "storage", feature = "vial", feature = "host"))]
#[test]
fn vial_behavior_write_survives_a_restart() {
    common::test_block_on(async {
        let flash = crate::common::simulator::flash::InMemoryFlash::<4096, 256, 4>::new();
        let host = SimHost::new();

        {
            let mut keyboard = SimKeyboard::builder([[[k!(A), k!(B)]]])
                .storage_flash(flash.clone())
                .build()
                .await;

            host.vial(&mut keyboard)
                .set_behavior_setting_u16(SettingKey::ComboTimeout, 80)
                .expect_ok();
            host.vial(&mut keyboard).set_combo(0, [k!(A), k!(B)], k!(C)).expect_ok();
            keyboard.wait_storage().run().await;
        }

        let mut keyboard = SimKeyboard::builder([[[k!(A), k!(B)]]])
            .storage_flash(flash)
            .build()
            .await;

        keyboard
            .press(0, 0)
            .expect_no_report(60)
            .expect_keys([HidKeyCode::A])
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_protocol_version_round_trips() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;
        let host = SimHost::new();

        host.rynk(&mut keyboard)
            .get_version()
            .expect(rmk_types::protocol::rynk::ProtocolVersion::CURRENT);

        keyboard.run().await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_keymap_write_changes_what_the_key_reports() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;
        let host = SimHost::new();

        host.rynk(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::B])
            .delay(10)
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_encoder_write_changes_what_the_knob_reports() {
    common::test_block_on(async {
        let encoder_action = EncoderAction::new(k!(C), k!(D));
        let mut keyboard = SimKeyboard::builder([[[k!(A)]]])
            .encoders([[EncoderAction::new(k!(A), k!(B))]])
            .build()
            .await;
        let host = SimHost::new();

        host.rynk(&mut keyboard)
            .get_encoder(0, 0)
            .expect(EncoderAction::new(k!(A), k!(B)));
        host.rynk(&mut keyboard).set_encoder(0, 0, encoder_action).expect_ok();
        host.rynk(&mut keyboard).get_encoder(0, 0).expect(encoder_action);

        keyboard
            .rotary_cw(0)
            .expect_keys([HidKeyCode::C])
            .expect_all_up()
            .rotary_ccw(0)
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_default_layer_write_changes_the_active_layer() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A)]], [[k!(B)]]]).build().await;
        let host = SimHost::new();

        host.rynk(&mut keyboard)
            .request::<command::SetDefaultLayer>(1)
            .expect_ok();
        host.rynk(&mut keyboard)
            .request::<command::GetDefaultLayer>(())
            .expect(1);

        keyboard
            .tap(0, 0, 10)
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

/// `SetKeyAction` checks the position but not the action, so a host can write a
/// `PDF` naming a layer the keymap does not have — `rmk-config` rejects that at
/// build time, the wire cannot. The keyboard must ignore the switch instead of
/// selecting a layer that isn't there.
#[cfg(feature = "rynk")]
#[test]
fn rynk_out_of_range_default_layer_write_is_ignored() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(No), k!(A)]], [[k!(No), k!(B)]]])
            .build()
            .await;
        let host = SimHost::new();

        host.rynk(&mut keyboard)
            .set_key(0, 0, 0, KeyAction::Single(Action::PersistentDefaultLayer(5)))
            .expect_ok();

        keyboard
            .tap(0, 1, 10)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .tap(0, 0, 10) // the out-of-range PDF
            .tap(0, 1, 10)
            .expect_keys([HidKeyCode::A]) // still layer 0, not B
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_macro_write_makes_the_macro_key_type_it() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[KeyAction::Single(Action::TriggerMacro(0))]]])
            .build()
            .await;
        let host = SimHost::new();
        let data = heapless::Vec::from_slice(&[1, 1, HidKeyCode::A as u8, 0]).unwrap();

        host.rynk(&mut keyboard)
            .request::<command::SetMacro>(SetMacroRequest {
                offset: 0,
                data: MacroData { data },
            })
            .expect_ok();

        keyboard
            .tap(0, 0, 10)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_combo_and_behavior_writes_change_when_the_chord_fires() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A), k!(B)]]]).build().await;
        let host = SimHost::new();
        let combo = Combo::new([k!(A), k!(B)], k!(C), None);
        let behavior = RynkBehaviorConfig {
            combo_timeout_ms: 80,
            oneshot_timeout_ms: 1000,
            tap_interval_ms: 200,
            tap_capslock_interval_ms: 250,
        };

        host.rynk(&mut keyboard)
            .request::<command::SetCombo>(SetComboRequest {
                index: 0,
                config: combo,
            })
            .expect_ok();
        host.rynk(&mut keyboard)
            .request::<command::SetBehaviorConfig>(behavior)
            .expect_ok();
        host.rynk(&mut keyboard)
            .request::<command::GetBehaviorConfig>(())
            .expect(behavior);

        keyboard
            .delay(10)
            .press(0, 0)
            .expect_no_report(60)
            .expect_keys([HidKeyCode::A])
            .release(0, 0)
            .expect_all_up()
            .delay(20)
            .press(0, 0)
            .delay(10)
            .press(0, 1)
            .expect_keys([HidKeyCode::C])
            .release(0, 0)
            .release(0, 1)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_bulk_keymap_write_changes_what_the_key_reports() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A), k!(B)]]]).build().await;
        let host = SimHost::new();
        let actions = heapless::Vec::from_slice(&[k!(C), k!(D)]).unwrap();

        host.rynk(&mut keyboard)
            .request::<command::SetKeymapBulk>(SetKeymapBulkRequest {
                layer: 0,
                start_row: 0,
                start_col: 0,
                actions: actions.clone(),
            })
            .expect_ok();
        host.rynk(&mut keyboard)
            .request::<command::GetKeymapBulk>(GetKeymapBulkRequest {
                layer: 0,
                start_row: 0,
                start_col: 0,
            })
            .expect(GetKeymapBulkResponse { actions });

        keyboard
            .tap(0, 1, 10)
            .expect_keys([HidKeyCode::D])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_morse_write_changes_what_the_tap_reports() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[KeyAction::Morse(0)]]]).await;
        let host = SimHost::new();
        let morse = Morse::new_from_vial(
            Action::Key(rmk::types::keycode::KeyCode::Hid(HidKeyCode::A)),
            Action::Key(rmk::types::keycode::KeyCode::Hid(HidKeyCode::B)),
            Action::Key(rmk::types::keycode::KeyCode::Hid(HidKeyCode::C)),
            Action::Key(rmk::types::keycode::KeyCode::Hid(HidKeyCode::D)),
            MorseProfile::new(Some(false), Some(MorseMode::Normal), Some(80), Some(80)),
        );

        host.rynk(&mut keyboard)
            .request::<command::SetMorse>(SetMorseRequest {
                index: 0,
                config: morse.clone(),
            })
            .expect_ok();
        host.rynk(&mut keyboard).request::<command::GetMorse>(0).expect(morse);

        keyboard
            .delay(100)
            .tap(0, 0, 20)
            .expect_keys([HidKeyCode::A])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_fork_write_changes_what_the_key_reports() {
    common::test_block_on(async {
        let mut keyboard = SimKeyboard::create([[[k!(A)]]]).await;
        let host = SimHost::new();
        let fork = Fork::new(
            k!(A),
            k!(B),
            k!(C),
            StateBits {
                modifiers: ModifierCombination::LSHIFT,
                ..Default::default()
            },
            StateBits::default(),
            ModifierCombination::default(),
            true,
        );

        host.rynk(&mut keyboard)
            .request::<command::SetFork>(SetForkRequest { index: 0, config: fork })
            .expect_ok();
        host.rynk(&mut keyboard).request::<command::GetFork>(0).expect(fork);

        keyboard
            .tap(0, 0, 10)
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(feature = "rynk")]
#[test]
fn rynk_serves_the_layout_blob_from_config() {
    common::test_block_on(async {
        static LAYOUT: &[u8] = &[1, 2, 3, 4, 5];
        let config = RmkConfig {
            layout_blob: LAYOUT,
            ..Default::default()
        };
        let mut keyboard = SimKeyboard::builder([[[k!(A)]]]).host_config(config).build().await;
        let host = SimHost::new();

        host.rynk(&mut keyboard)
            .request::<command::GetLayout>(0)
            .expect(LayoutChunk {
                total_len: LAYOUT.len() as u32,
                bytes: heapless::Vec::from_slice(LAYOUT).unwrap(),
            });

        keyboard.run().await;
    });
}

#[cfg(all(feature = "rynk", feature = "storage"))]
#[test]
fn rynk_keymap_write_survives_a_restart() {
    common::test_block_on(async {
        let flash = crate::common::simulator::flash::InMemoryFlash::<4096, 256, 4>::new();
        let host = SimHost::new();

        {
            let mut keyboard = SimKeyboard::builder([[[k!(A)]]])
                .storage_flash(flash.clone())
                .build()
                .await;

            host.rynk(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
            keyboard.wait_storage().run().await;
        }

        let mut keyboard = SimKeyboard::builder([[[k!(A)]]]).storage_flash(flash).build().await;
        keyboard
            .tap(0, 0, 10)
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}
