use core::cell::RefCell;

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use embassy_time::Duration;
use rmk_types::action::KeyAction;
use rmk_types::protocol::vial::{
    SettingKey, VIAL_COMBO_MAX_LENGTH, VIAL_EP_SIZE, VIAL_PROTOCOL_VERSION, VialCommand, VialDynamic,
};

use crate::combo::Combo;
use crate::config::VialConfig;
use crate::descriptor::ViaReport;
use crate::host::via::keycode_convert::{from_via_keycode, to_via_keycode};
use crate::keymap::KeyMap;
use crate::morse::{DOUBLE_TAP, HOLD, HOLD_AFTER_TAP, TAP};
#[cfg(feature = "storage")]
use crate::{
    COMBO_MAX_LENGTH, channel::FLASH_CHANNEL, host::storage_types::ComboData, host::via::storage::VialData,
    storage::FlashOperationMessage,
};
use crate::{COMBO_MAX_NUM, MORSE_MAX_NUM};

/// Note: vial uses little endian, while via uses big endian
pub(crate) async fn process_vial<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    report: &mut ViaReport,
    vial_config: &VialConfig<'a>,
    #[cfg(feature = "vial_lock")] locker: &mut super::vial_lock::VialLock<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    keymap: &RefCell<KeyMap<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
) {
    // report.output_data[0] == 0xFE -> vial commands
    let vial_command = report.output_data[1].into();
    debug!("Received vial command: {:?}", vial_command);
    match vial_command {
        VialCommand::GetKeyboardId => {
            // Returns vial protocol version + vial keyboard id
            LittleEndian::write_u32(&mut report.input_data[0..4], VIAL_PROTOCOL_VERSION);
            report.input_data[4..12].clone_from_slice(vial_config.vial_keyboard_id);
            debug!("Vial return: {:?}", report.input_data);
        }
        VialCommand::GetSize => {
            LittleEndian::write_u32(&mut report.input_data[0..4], vial_config.vial_keyboard_def.len() as u32);
        }
        VialCommand::GetKeyboardDef => {
            let page = LittleEndian::read_u16(&report.output_data[2..4]) as usize;
            let start = page * VIAL_EP_SIZE;
            let mut end = start + VIAL_EP_SIZE;
            let vial_keyboard_def = &vial_config.vial_keyboard_def;
            if end < start || start >= vial_keyboard_def.len() {
                return;
            }
            if end > vial_keyboard_def.len() {
                end = vial_keyboard_def.len();
            }
            vial_keyboard_def[start..end].iter().enumerate().for_each(|(i, v)| {
                report.input_data[i] = *v;
            });
            debug!(
                "Vial return: page:{} start:{} end: {}, data: {:?}",
                page, start, end, report.input_data
            );
        }
        VialCommand::GetUnlockStatus => {
            // Reset all data to 0xFF(it's required!)
            report.input_data.fill(0xFF);
            #[cfg(feature = "vial_lock")]
            {
                // Unlocked
                report.input_data[0] = locker.is_unlocked() as u8;
                // Unlock in progress
                report.input_data[1] = locker.is_unlocking() as u8;
                // Unlock keys
                for (idx, (row, col)) in vial_config.unlock_keys.iter().enumerate() {
                    report.input_data[2 + idx * 2] = *row;
                    report.input_data[3 + idx * 2] = *col;
                }
            }
            #[cfg(not(feature = "vial_lock"))]
            {
                // Unlocked
                report.input_data[0] = 1;
                // Unlock in progress
                report.input_data[1] = 0;
                warn!("Vial lock feature is not enabled");
            }
        }
        VialCommand::UnlockStart => {
            #[cfg(feature = "vial_lock")]
            locker.unlocking();
            #[cfg(not(feature = "vial_lock"))]
            error!("Vial lock feature is not enabled");
        }
        VialCommand::UnlockPoll => {
            #[cfg(feature = "vial_lock")]
            {
                locker.unlocking();
                report.input_data[0] = locker.is_unlocked() as u8;
                report.input_data[1] = locker.is_unlocking() as u8;
                report.input_data[2] = locker.check_unlock();
            }
            #[cfg(not(feature = "vial_lock"))]
            error!("Vial lock feature is not enabled");
        }
        VialCommand::Lock => {
            #[cfg(feature = "vial_lock")]
            locker.lock();
            #[cfg(not(feature = "vial_lock"))]
            error!("Vial lock feature is not enabled");
        }
        VialCommand::BehaviorSettingQuery => {
            report.input_data.fill(0xFF);
            let value = u16::from_le_bytes([report.output_data[2], report.output_data[3]]);
            if value <= 7 {
                LittleEndian::write_u16(&mut report.input_data[0..2], 0x02);
                LittleEndian::write_u16(&mut report.input_data[2..4], 0x06);
                LittleEndian::write_u16(&mut report.input_data[4..6], 0x07);
                LittleEndian::write_u16(&mut report.input_data[6..8], 0x12);
                LittleEndian::write_u16(&mut report.input_data[8..10], 0x13);
                LittleEndian::write_u16(&mut report.input_data[10..12], 0x1A);
                LittleEndian::write_u16(&mut report.input_data[12..14], 0x1B);
            }
        }
        VialCommand::GetBehaviorSetting => {
            report.input_data.fill(0xFF);
            let value = u16::from_le_bytes([report.output_data[2], report.output_data[3]]);
            match value.into() {
                SettingKey::None => (),
                SettingKey::ComboTimeout => {
                    report.input_data[0] = 0;
                    let combo_timeout = keymap.borrow().behavior.combo.timeout.as_millis() as u16;
                    LittleEndian::write_u16(&mut report.input_data[1..3], combo_timeout);
                }
                SettingKey::MorseTimeout => {
                    report.input_data[0] = 0;
                    let tapping_term = keymap
                        .borrow()
                        .behavior
                        .morse
                        .default_profile
                        .hold_timeout_ms()
                        .unwrap_or(0);
                    LittleEndian::write_u16(&mut report.input_data[1..3], tapping_term);
                }
                SettingKey::OneShotTimeout => {
                    report.input_data[0] = 0;
                    let one_shot_timeout = keymap.borrow().behavior.one_shot.timeout.as_millis() as u16;
                    LittleEndian::write_u16(&mut report.input_data[1..3], one_shot_timeout);
                }
                SettingKey::TapInterval => {
                    report.input_data[0] = 0;
                    let tap_interval = keymap.borrow().behavior.tap.tap_interval;
                    LittleEndian::write_u16(&mut report.input_data[1..3], tap_interval);
                }
                SettingKey::TapCapslockInterval => {
                    report.input_data[0] = 0;
                    let tap_interval = keymap.borrow().behavior.tap.tap_interval;
                    LittleEndian::write_u16(&mut report.input_data[1..3], tap_interval);
                }
                SettingKey::UnilateralTap => {
                    report.input_data[0] = 0;
                    let unilateral_tap = keymap
                        .borrow()
                        .behavior
                        .morse
                        .default_profile
                        .unilateral_tap()
                        .unwrap_or(false);
                    if unilateral_tap {
                        report.input_data[1] = 1;
                    } else {
                        report.input_data[1] = 0;
                    };
                }
                SettingKey::PriorIdleTime => {
                    report.input_data[0] = 0;
                    let prior_idle_time = keymap.borrow().behavior.morse.prior_idle_time.as_millis() as u16;
                    LittleEndian::write_u16(&mut report.input_data[1..3], prior_idle_time);
                }
            }
        }
        VialCommand::SetBehaviorSetting => {
            let key = u16::from_le_bytes([report.output_data[2], report.output_data[3]]);
            match key.into() {
                SettingKey::None => (),
                SettingKey::ComboTimeout => {
                    let combo_timeout = u16::from_le_bytes([report.output_data[4], report.output_data[5]]);
                    keymap.borrow_mut().behavior.combo.timeout = Duration::from_millis(combo_timeout as u64);
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::ComboTimeout(combo_timeout))
                        .await;
                }
                SettingKey::MorseTimeout => {
                    let timeout_time = u16::from_le_bytes([report.output_data[4], report.output_data[5]]);
                    let old = keymap.borrow().behavior.morse.default_profile;
                    keymap.borrow_mut().behavior.morse.default_profile = old.with_hold_timeout_ms(Some(timeout_time));
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::MorseHoldTimeout(timeout_time))
                        .await;
                }
                SettingKey::OneShotTimeout => {
                    let timeout_time = u16::from_le_bytes([report.output_data[4], report.output_data[5]]);
                    keymap.borrow_mut().behavior.one_shot.timeout = Duration::from_millis(timeout_time as u64);
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::OneShotTimeout(timeout_time))
                        .await;
                }
                SettingKey::TapInterval => {
                    let tap_interval = u16::from_le_bytes([report.output_data[4], report.output_data[5]]);
                    keymap.borrow_mut().behavior.tap.tap_interval = tap_interval;
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::TapInterval(tap_interval))
                        .await;
                }
                SettingKey::TapCapslockInterval => {
                    let tap_capslock_interval = u16::from_le_bytes([report.output_data[4], report.output_data[5]]);
                    keymap.borrow_mut().behavior.tap.tap_capslock_interval = tap_capslock_interval;
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::TapCapslockInterval(tap_capslock_interval))
                        .await;
                }
                SettingKey::UnilateralTap => {
                    let old = keymap.borrow().behavior.morse.default_profile;
                    keymap.borrow_mut().behavior.morse.default_profile =
                        old.with_unilateral_tap(Some(report.output_data[4] == 1));
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::UnilateralTap(report.output_data[4] == 1))
                        .await;
                }
                SettingKey::PriorIdleTime => {
                    let prior_idle_time = u16::from_le_bytes([report.output_data[4], report.output_data[5]]);
                    keymap.borrow_mut().behavior.morse.prior_idle_time = Duration::from_millis(prior_idle_time as u64);
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::PriorIdleTime(prior_idle_time))
                        .await;
                }
            }
        }
        VialCommand::DynamicEntryOp => {
            let vial_dynamic = report.output_data[2].into();
            match vial_dynamic {
                VialDynamic::DynamicVialGetNumberOfEntries => {
                    debug!("DynamicEntryOp - DynamicVialGetNumberOfEntries");
                    report.input_data[0] = core::cmp::min(MORSE_MAX_NUM, 255) as u8; // Tap dance entries
                    report.input_data[1] = core::cmp::min(COMBO_MAX_NUM, 255) as u8; // Combo entries
                    // TODO: Support dynamic key override
                    report.input_data[2] = 0; // Key override entries
                    report.input_data[31] = 1 // Enable caps word
                }
                VialDynamic::DynamicVialMorseGet => {
                    debug!("DynamicEntryOp - DynamicVialMorseGet");
                    report.input_data[0] = 0; // Index 0 is the return code, 0 means success

                    let morse_idx = report.output_data[3] as usize;
                    let morses = &keymap.borrow().behavior.morse.morses;
                    if let Some(morse) = morses.get(morse_idx) {
                        // Pack morse data into report
                        LittleEndian::write_u16(
                            &mut report.input_data[1..3],
                            to_via_keycode(morse.get(TAP).map_or(KeyAction::No, |a| KeyAction::Single(a))),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[3..5],
                            to_via_keycode(morse.get(HOLD).map_or(KeyAction::No, |a| KeyAction::Single(a))),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[5..7],
                            to_via_keycode(morse.get(DOUBLE_TAP).map_or(KeyAction::No, |a| KeyAction::Single(a))),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[7..9],
                            to_via_keycode(
                                morse
                                    .get(HOLD_AFTER_TAP)
                                    .map_or(KeyAction::No, |a| KeyAction::Single(a)),
                            ),
                        );
                        LittleEndian::write_u16(&mut report.input_data[9..11], 250); //dummy morse.timeout_ms
                    } else {
                        report.input_data[1..11].fill(0);
                    }
                }
                VialDynamic::DynamicVialMorseSet => {
                    debug!("DynamicEntryOp - DynamicVialMorseSet");
                    report.input_data[0] = 0; // Index 0 is the return code, 0 means success

                    let morse_idx = report.output_data[3] as usize;
                    let morses = &mut keymap.borrow_mut().behavior.morse.morses;

                    if morse_idx < morses.len() {
                        // Update the morse in keymap
                        if let Some(morse) = morses.get_mut(morse_idx) {
                            // Extract morse (also known as "tap dance" in vial)
                            let tap = from_via_keycode(LittleEndian::read_u16(&report.output_data[4..6]));
                            let hold = from_via_keycode(LittleEndian::read_u16(&report.output_data[6..8]));
                            let double_tap = from_via_keycode(LittleEndian::read_u16(&report.output_data[8..10]));
                            let hold_after_tap = from_via_keycode(LittleEndian::read_u16(&report.output_data[10..12]));
                            //let timeout_ms = LittleEndian::read_u16(&report.output_data[12..14]);

                            morse.put(TAP, tap.to_action());
                            morse.put(DOUBLE_TAP, double_tap.to_action());
                            morse.put(HOLD, hold.to_action());
                            morse.put(HOLD_AFTER_TAP, hold_after_tap.to_action());
                            //morse.timeout_ms = timeout_ms;

                            #[cfg(feature = "storage")]
                            {
                                // Save to storage
                                FLASH_CHANNEL
                                    .send(FlashOperationMessage::VialMessage(VialData::Morse(
                                        morse_idx as u8,
                                        morse.clone(),
                                    )))
                                    .await;
                            }
                        }
                    }
                }
                VialDynamic::DynamicVialComboGet => {
                    debug!("DynamicEntryOp - DynamicVialComboGet");
                    report.input_data[0] = 0; // Index 0 is the return code, 0 means success

                    let combo_idx = report.output_data[3] as usize;
                    let combos = &keymap.borrow().behavior.combo.combos;
                    if let Some((_, combo)) = vial_combo(combos, combo_idx) {
                        for i in 0..VIAL_COMBO_MAX_LENGTH {
                            LittleEndian::write_u16(
                                &mut report.input_data[1 + i * 2..3 + i * 2],
                                to_via_keycode(*combo.actions.get(i).unwrap_or(&KeyAction::No)),
                            );
                        }
                        LittleEndian::write_u16(
                            &mut report.input_data[1 + VIAL_COMBO_MAX_LENGTH * 2..3 + VIAL_COMBO_MAX_LENGTH * 2],
                            to_via_keycode(combo.output),
                        );
                    } else {
                        report.input_data[1..3 + VIAL_COMBO_MAX_LENGTH * 2].fill(0);
                    }
                }
                VialDynamic::DynamicVialComboSet => {
                    debug!("DynamicEntryOp - DynamicVialComboSet");
                    report.input_data[0] = 0; // Index 0 is the return code, 0 means success

                    #[cfg(feature = "storage")]
                    let (real_idx, actions, output) = {
                        // Drop combos to release the borrowed keymap, avoid potential run-time panics
                        let combo_idx = report.output_data[3] as usize;
                        let km = &mut keymap.borrow_mut();
                        let combos = &mut km.behavior.combo.combos;
                        let Some((real_idx, combo)) = vial_combo_mut(combos, combo_idx) else {
                            return;
                        };

                        let mut actions = [KeyAction::No; COMBO_MAX_LENGTH];
                        let mut n: usize = 0;
                        for i in 0..VIAL_COMBO_MAX_LENGTH {
                            let action =
                                from_via_keycode(LittleEndian::read_u16(&report.output_data[4 + i * 2..6 + i * 2]));
                            if !action.is_empty() {
                                if n >= COMBO_MAX_LENGTH {
                                    //fail if the combo action buffer is too small
                                    return;
                                }
                                actions[n] = action;
                                n += 1;
                            }
                        }
                        let output = from_via_keycode(LittleEndian::read_u16(
                            &report.output_data[4 + VIAL_COMBO_MAX_LENGTH * 2..6 + VIAL_COMBO_MAX_LENGTH * 2],
                        ));

                        combo.actions.clear();
                        let _ = combo.actions.extend_from_slice(&actions[0..n]);
                        combo.output = output;

                        //reordering combo order
                        km.reorder_combos();

                        (real_idx, actions, output)
                    };
                    #[cfg(feature = "storage")]
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::VialMessage(VialData::Combo(ComboData {
                            idx: real_idx,
                            actions,
                            output,
                        })))
                        .await;
                }
                VialDynamic::DynamicVialKeyOverrideGet => {
                    warn!("DynamicEntryOp - DynamicVialKeyOverrideGet -- to be implemented");
                    report.input_data.fill(0x00);
                }
                VialDynamic::DynamicVialKeyOverrideSet => {
                    warn!("DynamicEntryOp - DynamicVialKeyOverrideSet -- to be implemented");
                    report.input_data.fill(0x00);
                }
                VialDynamic::Unhandled => {
                    warn!("DynamicEntryOp - Unhandled -- subcommand not recognized");
                    report.input_data.fill(0x00);
                }
            }
        }
        VialCommand::GetEncoder => {
            let layer = report.output_data[2];
            let index = report.output_data[3];
            debug!("Received Vial - GetEncoder, encoder idx: {} at layer: {}", index, layer);

            // Get encoder value
            if let Some(encoder_map) = &keymap.borrow().encoders {
                if let Some(encoder_layer) = encoder_map.get(layer as usize) {
                    if let Some(encoder) = encoder_layer.get(index as usize) {
                        let clockwise = to_via_keycode(encoder.clockwise());
                        let counter_clockwise = to_via_keycode(encoder.counter_clockwise());
                        BigEndian::write_u16(&mut report.input_data[0..2], counter_clockwise);
                        BigEndian::write_u16(&mut report.input_data[2..4], clockwise);
                        return;
                    }
                }
            }

            // Clear returned value, aka `KeyAction::No`
            report.input_data.fill(0x0);
        }
        VialCommand::SetEncoder => {
            let layer = report.output_data[2];
            let index = report.output_data[3];
            let clockwise = report.output_data[4];
            debug!(
                "Received Vial - SetEncoder, encoder idx: {} clockwise: {} at layer: {}",
                index, clockwise, layer
            );
            let _encoder = match keymap.borrow_mut().encoders {
                Some(ref mut encoder_map) => {
                    if let Some(encoder_layer) = encoder_map.get_mut(layer as usize) {
                        if let Some(encoder) = encoder_layer.get_mut(index as usize) {
                            if clockwise == 1 {
                                let keycode = BigEndian::read_u16(&report.output_data[5..7]);
                                let action = from_via_keycode(keycode);
                                info!("Setting clockwise action: {:?}", action);
                                encoder.set_clockwise(action);
                            } else {
                                let keycode = BigEndian::read_u16(&report.output_data[5..7]);
                                let action = from_via_keycode(keycode);
                                info!("Setting counter-clockwise action: {:?}", action);
                                encoder.set_counter_clockwise(action);
                            }
                            Some(*encoder)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };

            #[cfg(feature = "storage")]
            // Save the encoder action to the storage after the RefCell is released
            if let Some(encoder) = _encoder {
                // Save the encoder action to the storage
                use crate::host::storage_types::EncoderConfig;
                FLASH_CHANNEL
                    .send(FlashOperationMessage::VialMessage(VialData::Encoder(EncoderConfig {
                        idx: index,
                        layer,
                        action: encoder,
                    })))
                    .await;
            }
        }
        _ => (),
    }
}

fn vial_combo(combos: &heapless::Vec<Combo, COMBO_MAX_NUM>, idx: usize) -> Option<(usize, &Combo)> {
    combos
        .iter()
        .enumerate()
        .filter(|(_, combo)| combo.actions.len() <= VIAL_COMBO_MAX_LENGTH)
        .enumerate()
        .find_map(|(i, combo)| (i == idx).then_some(combo))
}

fn vial_combo_mut(combos: &mut heapless::Vec<Combo, COMBO_MAX_NUM>, idx: usize) -> Option<(usize, &mut Combo)> {
    combos
        .iter_mut()
        .enumerate()
        .filter(|(_, combo)| combo.actions.len() <= VIAL_COMBO_MAX_LENGTH)
        .enumerate()
        .find_map(|(i, combo)| (i == idx).then_some(combo))
}
