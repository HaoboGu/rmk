pub mod common;

#[cfg(all(feature = "storage", feature = "host", not(feature = "_no_usb")))]
use rmk::KeymapData;
#[cfg(all(feature = "storage", feature = "host", not(feature = "_no_usb")))]
use rmk::config::{BehaviorConfig, PositionalConfig, StorageConfig};
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
#[cfg(all(feature = "steno", not(feature = "_no_usb")))]
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::HidKeyCode;
#[cfg(not(feature = "_no_usb"))]
use rmk::types::keycode::{ConsumerKey, SystemControlKey};
#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
use rmk::types::protocol::vial::SettingKey;
#[cfg(all(feature = "steno", not(feature = "_no_usb")))]
use rmk::types::steno::StenoKey;
use rmk::{k, layer};
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
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::create(keymap).await;

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::A])
            .delay(10)
            .release(0, 0)
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_runs_via_host_transaction() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
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
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
        let host = SimHost::usb();

        host.vial(&mut keyboard).get_key(0, 0, 0).expect(k!(A));
        host.vial(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();
        host.vial(&mut keyboard).get_key(0, 0, 0).expect(k!(B));

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::B])
            .delay(10)
            .release(0, 0)
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_encoder_update_and_rotary_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A)]])];
        let encoder_action = encoder!(k!(C), k!(D));
        let mut keyboard = SimKeyboard::builder(keymap)
            .encoders([[encoder!(k!(A), k!(B))]])
            .vial()
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
            .expect_empty()
            .rotary_ccw(0)
            .expect_keys([HidKeyCode::D])
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_vial_negative_paths_are_timeline_steps() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap)
            .encoders([[encoder!(k!(A), k!(B))]])
            .vial()
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
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
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
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_dynamic_combo_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A), k!(B)]])];
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
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
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "vial", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_vial_dynamic_morse_update_and_key_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[rmk::td!(0)]])];
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
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
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "vial", feature = "_ble", not(feature = "_no_usb")))]
#[test]
fn simulator_routes_vial_replies_to_ble_host() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
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
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap).vial().build().await;
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

#[cfg(all(feature = "storage", feature = "host", not(feature = "_no_usb")))]
#[test]
fn simulator_storage_loaded_keymap_survives_restart() {
    common::test_block_on::test_block_on(async {
        let flash = rmk::sim::flash::InMemoryFlash::<4096, 256, 4>::new();
        let storage_config = StorageConfig::default();

        {
            let data = Box::leak(Box::new(KeymapData::new([layer!([[k!(B)]])])));
            let behavior_config = Box::leak(Box::new(BehaviorConfig::default()));
            let positional_config = Box::leak(Box::new(PositionalConfig::default()));
            let (_keymap, _storage) = rmk::initialize_keymap_and_storage(
                data,
                flash.clone(),
                &storage_config,
                behavior_config,
                positional_config,
            )
            .await;
        }

        let data = Box::leak(Box::new(KeymapData::new([layer!([[k!(A)]])])));
        let behavior_config = Box::leak(Box::new(BehaviorConfig::default()));
        let positional_config = Box::leak(Box::new(PositionalConfig::default()));
        let (keymap, _storage) = rmk::initialize_keymap_and_storage(
            data,
            flash.clone(),
            &storage_config,
            behavior_config,
            positional_config,
        )
        .await;
        let keymap = Box::leak(Box::new(keymap));
        let mut keyboard = SimKeyboard::from_keymap(keymap);

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::B])
            .release(0, 0)
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_runs_rynk_host_transaction() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap).rynk().build().await;
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
        let keymap = [layer!([[k!(A)]])];
        let mut keyboard = SimKeyboard::builder(keymap).rynk().build().await;
        let host = SimHost::usb();

        host.rynk(&mut keyboard).set_key(0, 0, 0, k!(B)).expect_ok();

        keyboard
            .press(0, 0)
            .expect_keys([HidKeyCode::B])
            .delay(10)
            .release(0, 0)
            .expect_empty()
            .run()
            .await;
    });
}

#[cfg(all(feature = "rynk", not(feature = "_no_usb")))]
#[test]
fn simulator_combines_rynk_encoder_update_and_rotary_reports() {
    common::test_block_on::test_block_on(async {
        let keymap = [layer!([[k!(A)]])];
        let encoder_action = EncoderAction::new(k!(C), k!(D));
        let mut keyboard = SimKeyboard::builder(keymap)
            .encoders([[EncoderAction::new(k!(A), k!(B))]])
            .rynk()
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
            .expect_empty()
            .rotary_ccw(0)
            .expect_keys([HidKeyCode::D])
            .expect_empty()
            .run()
            .await;
    });
}
