pub mod common;

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
use rmk::config::RmkConfig;
#[cfg(feature = "vial")]
use rmk::encoder;
#[cfg(not(feature = "_no_usb"))]
use rmk::hid::Report;
#[cfg(all(feature = "steno", not(feature = "_no_usb")))]
use rmk::hid::StenoReport;
#[cfg(any(feature = "vial", feature = "rynk"))]
use rmk::sim::SimHost;
use rmk::sim::SimKeyboard;
#[cfg(any(
    all(feature = "vial", not(feature = "_no_usb")),
    all(feature = "rynk", not(feature = "_no_usb"))
))]
use rmk::types::action::EncoderAction;
#[cfg(all(any(feature = "steno", feature = "rynk"), not(feature = "_no_usb")))]
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::HidKeyCode;
#[cfg(not(feature = "_no_usb"))]
use rmk::types::keycode::{ConsumerKey, SystemControlKey};
#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
use rmk::types::protocol::vial::SettingKey;
#[cfg(all(feature = "steno", not(feature = "_no_usb")))]
use rmk::types::steno::StenoKey;
use rmk::{k, layer};
#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
use rmk_types::combo::Combo;
#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
use rmk_types::fork::{Fork, StateBits};
#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
use rmk_types::modifier::ModifierCombination;
#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
use rmk_types::morse::{Morse, MorseMode, MorseProfile};
#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
use rmk_types::protocol::rynk::{
    BehaviorConfig as RynkBehaviorConfig, LayoutChunk, MacroData, SetComboRequest, SetForkRequest, SetMacroRequest,
    SetMorseRequest, command,
};
#[cfg(all(feature = "rynk", feature = "bulk", not(feature = "_no_usb")))]
use rmk_types::protocol::rynk::{GetKeymapBulkRequest, GetKeymapBulkResponse, SetKeymapBulkRequest};
#[cfg(feature = "vial")]
use rmk_types::protocol::vial::{VIA_PROTOCOL_VERSION, ViaCommand};
#[cfg(not(feature = "_no_usb"))]
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};

#[cfg(feature = "storage")]
#[test]
fn simulator_in_memory_flash_persists_across_clones() {
    let mut flash = rmk::sim::flash::InMemoryFlash::<1024, 256, 4>::new();
    let mut clone = flash.clone();

    embedded_storage::nor_flash::NorFlash::erase(&mut flash, 0, 256).unwrap();
    embedded_storage::nor_flash::NorFlash::write(&mut flash, 0, &[0xAA, 0xBB, 0xCC, 0xDD]).unwrap();

    let mut read = [0u8; 4];
    embedded_storage::nor_flash::ReadNorFlash::read(&mut clone, 0, &mut read).unwrap();

    assert_eq!(read, [0xAA, 0xBB, 0xCC, 0xDD]);
}

#[cfg(not(feature = "_no_usb"))]
#[test]
fn simulator_runs_keyboard_sequence() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::A])
            .delay(10)
            .release(0, 0)
            .expect_all_up()
            .run()
            .await;
    });
}

#[cfg(not(feature = "_no_usb"))]
#[test]
#[should_panic(expected = "unexpected trailing HID report")]
fn simulator_rejects_unasserted_trailing_report() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;

        keyboard.press(0, 0).run().await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_runs_via_host_transaction() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::usb();
        let mut expected = [0u8; 32];
        expected[0] = ViaCommand::GetProtocolVersion as u8;
        expected[1..3].copy_from_slice(&VIA_PROTOCOL_VERSION.to_be_bytes());

        host.vial(&mut keyboard).get_protocol_version().expect(expected);

        keyboard.run().await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_via_keymap_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_encoder_update_and_rotary_reports() {
    common::test_block_on::test_block_on(async {
        let encoder_action = encoder!(k!(C), k!(D));
        let mut keyboard = SimKeyboard::single_key(k!(A))
            .encoders([[encoder!(k!(A), k!(B))]])
            .build()
            .await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_vial_negative_paths_are_timeline_steps() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A))
            .encoders([[encoder!(k!(A), k!(B))]])
            .build()
            .await;
        let host = SimHost::usb();

        host.vial(&mut keyboard)
            .get_encoder(0, 99)
            .expect(EncoderAction::default());
        host.vial(&mut keyboard).unsupported_dynamic_entry().expect([0u8; 32]);

        keyboard.run().await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_behavior_settings_and_key_output() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A), k!(B)]])];
        let mut keyboard = SimKeyboard::builder(keymap).build().await;
        let host = SimHost::usb();

        host.vial(&mut keyboard)
            .get_behavior_setting(SettingKey::ComboTimeout)
            .expect_u16(50);
        host.vial(&mut keyboard)
            .set_behavior_setting_u16(SettingKey::ComboTimeout, 80)
            .expect_echo();
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

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_dynamic_combo_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A), k!(B)]])];
        let mut keyboard = SimKeyboard::builder(keymap).build().await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_dynamic_morse_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[rmk::td!(0)]])];
        let mut keyboard = SimKeyboard::builder(keymap).build().await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "vial", feature = "_ble", not(feature = "_no_usb")))]
#[test]
fn simulator_accepts_vial_transaction_from_ble_host() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::ble();
        let mut expected = [0u8; 32];
        expected[0] = ViaCommand::GetProtocolVersion as u8;
        expected[1..3].copy_from_slice(&VIA_PROTOCOL_VERSION.to_be_bytes());

        host.vial(&mut keyboard).get_protocol_version().expect(expected);

        keyboard.run().await;
    });
}

#[cfg(all(feature = "vial", feature = "storage", not(feature = "_no_usb")))]
#[test]
fn simulator_vial_persistence_messages_are_observable() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::usb();

        host.vial(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
        keyboard.expect_flash_key(0, 0, 0, k!(B));

        host.vial(&mut keyboard)
            .set_behavior_setting_u16(SettingKey::ComboTimeout, 77)
            .expect_echo();
        keyboard.expect_flash_combo_timeout(77);

        keyboard.run().await;
    });
}

#[cfg(not(feature = "_no_usb"))]
#[test]
fn simulator_reports_consumer_system_and_mouse_hid_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(AudioVolUp), k!(SystemSleep), k!(MouseRight)]])];
        let mut keyboard = SimKeyboard::create(keymap).await;

        keyboard
            .press(0, 0)
            .expect_report(Report::MediaKeyboardReport(MediaKeyboardReport {
                usage_id: ConsumerKey::VolumeIncrement as u16,
            }))
            .release(0, 0)
            .expect_report(Report::MediaKeyboardReport(MediaKeyboardReport { usage_id: 0 }))
            .press(0, 1)
            .expect_report(Report::SystemControlReport(SystemControlReport {
                usage_id: SystemControlKey::Sleep as u8,
            }))
            .release(0, 1)
            .expect_report(Report::SystemControlReport(SystemControlReport { usage_id: 0 }))
            .press(0, 2)
            .expect_report(Report::MouseReport(MouseReport {
                buttons: 0,
                x: 5,
                y: 0,
                wheel: 0,
                pan: 0,
            }))
            .release(0, 2)
            .expect_report(Report::MouseReport(MouseReport {
                buttons: 0,
                x: 0,
                y: 0,
                wheel: 0,
                pan: 0,
            }))
            .run()
            .await;
    });
}

#[cfg(all(feature = "steno", not(feature = "_no_usb")))]
#[test]
fn simulator_reports_steno_hid_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[KeyAction::Single(Action::Steno(StenoKey::S1))]])];
        let mut keyboard = SimKeyboard::create(keymap).await;

        keyboard
            .press(0, 0)
            .expect_report(Report::StenoReport(StenoReport {
                keys: [0x80, 0, 0, 0, 0, 0, 0, 0],
            }))
            .release(0, 0)
            .expect_report(Report::StenoReport(StenoReport { keys: [0; 8] }))
            .run()
            .await;
    });
}

#[cfg(all(feature = "storage", feature = "vial", feature = "host", not(feature = "_no_usb")))]
#[test]
fn simulator_storage_loaded_keymap_survives_restart() {
    common::test_block_on::test_block_on(async {
        let flash = rmk::sim::flash::InMemoryFlash::<4096, 256, 4>::new();
        let host = SimHost::usb();

        {
            let mut keyboard = SimKeyboard::single_key(k!(A))
                .storage_flash(flash.clone())
                .build()
                .await;

            host.vial(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
            keyboard.wait_storage().run().await;
        }

        let mut keyboard = SimKeyboard::single_key(k!(A))
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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_runs_rynk_host_transaction() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::usb();

        host.rynk(&mut keyboard)
            .get_version()
            .expect(rmk_types::protocol::rynk::ProtocolVersion::CURRENT);

        keyboard.run().await;
    });
}

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_keymap_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_encoder_update_and_rotary_reports() {
    common::test_block_on::test_block_on(async {
        let encoder_action = EncoderAction::new(k!(C), k!(D));
        let mut keyboard = SimKeyboard::single_key(k!(A))
            .encoders([[EncoderAction::new(k!(A), k!(B))]])
            .build()
            .await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_default_layer_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A)]], [[k!(B)]]]).build().await;
        let host = SimHost::usb();

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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_macro_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(KeyAction::Single(Action::TriggerMacro(0)))
            .build()
            .await;
        let host = SimHost::usb();
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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_combo_and_behavior_updates_with_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A), k!(B)]]]).build().await;
        let host = SimHost::usb();
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

#[cfg(all(feature = "rynk", feature = "bulk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_bulk_keymap_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::builder([[[k!(A), k!(B)]]]).build().await;
        let host = SimHost::usb();
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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_morse_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(KeyAction::Morse(0)).build().await;
        let host = SimHost::usb();
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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_fork_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let mut keyboard = SimKeyboard::single_key(k!(A)).build().await;
        let host = SimHost::usb();
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

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_reads_rynk_layout_from_keyboard_config() {
    common::test_block_on::test_block_on(async {
        static LAYOUT: &[u8] = &[1, 2, 3, 4, 5];
        let config = RmkConfig {
            layout_blob: LAYOUT,
            ..Default::default()
        };
        let mut keyboard = SimKeyboard::single_key(k!(A)).host_config(config).build().await;
        let host = SimHost::usb();

        host.rynk(&mut keyboard)
            .request::<command::GetLayout>(0)
            .expect(LayoutChunk {
                total_len: LAYOUT.len() as u32,
                bytes: heapless::Vec::from_slice(LAYOUT).unwrap(),
            });

        keyboard.run().await;
    });
}

#[cfg(all(feature = "rynk", feature = "storage", not(feature = "_no_usb")))]
#[test]
fn simulator_rynk_keymap_update_survives_restart() {
    common::test_block_on::test_block_on(async {
        let flash = rmk::sim::flash::InMemoryFlash::<4096, 256, 4>::new();
        let host = SimHost::usb();

        {
            let mut keyboard = SimKeyboard::single_key(k!(A))
                .storage_flash(flash.clone())
                .build()
                .await;

            host.rynk(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
            keyboard.wait_storage().run().await;
        }

        let mut keyboard = SimKeyboard::single_key(k!(A)).storage_flash(flash).build().await;
        keyboard
            .tap(0, 0, 10)
            .expect_keys([HidKeyCode::B])
            .expect_all_up()
            .run()
            .await;
    });
}
