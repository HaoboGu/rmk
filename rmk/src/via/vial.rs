use core::cell::RefCell;

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use num_enum::FromPrimitive;

use crate::action::KeyAction;
use crate::combo::Combo;
use crate::descriptor::ViaReport;
use crate::keymap::KeyMap;
use crate::via::keycode_convert::{from_via_keycode, to_via_keycode};
#[cfg(feature = "storage")]
use crate::{
    channel::FLASH_CHANNEL,
    storage::{ComboData, FlashOperationMessage},
    COMBO_MAX_LENGTH,
};
use crate::{COMBO_MAX_NUM, TAP_DANCE_MAX_NUM};

/// Vial communication commands. Check [vial-qmk/quantum/vial.h`](https://github.com/vial-kb/vial-qmk/blob/20d61fcb373354dc17d6ecad8f8176be469743da/quantum/vial.h#L36)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub(crate) enum VialCommand {
    GetKeyboardId = 0x00,
    GetSize = 0x01,
    GetKeyboardDef = 0x02,
    GetEncoder = 0x03,
    SetEncoder = 0x04,
    GetUnlockStatus = 0x05,
    UnlockStart = 0x06,
    UnlockPoll = 0x07,
    Lock = 0x08,
    QmkSettingsQuery = 0x09,
    QmkSettingsGet = 0x0A,
    QmkSettingsSet = 0x0B,
    QmkSettingsReset = 0x0C,
    DynamicEntryOp = 0x0D, /* operate on tapdance, combos, etc */
    #[num_enum(default)]
    Unhandled = 0xFF,
}

/// Vial dynamic commands. Check [vial-qmk/quantum/vial.h`](https://github.com/vial-kb/vial-qmk/blob/20d61fcb373354dc17d6ecad8f8176be469743da/quantum/vial.h#L53)
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, FromPrimitive)]
#[repr(u8)]
pub(crate) enum VialDynamic {
    DynamicVialGetNumberOfEntries = 0x00,
    DynamicVialTapDanceGet = 0x01,
    DynamicVialTapDanceSet = 0x02,
    DynamicVialComboGet = 0x03,
    DynamicVialComboSet = 0x04,
    DynamicVialKeyOverrideGet = 0x05,
    DynamicVialKeyOverrideSet = 0x06,
    #[num_enum(default)]
    Unhandled = 0xFF,
}

const VIAL_PROTOCOL_VERSION: u32 = 6;
const VIAL_EP_SIZE: usize = 32;
const VIAL_COMBO_MAX_LENGTH: usize = 4;

/// Note: vial uses little endian, while via uses big endian
pub(crate) async fn process_vial<
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    report: &mut ViaReport,
    vial_keyboard_Id: &[u8],
    vial_keyboard_def: &[u8],
    keymap: &RefCell<KeyMap<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
) {
    // report.output_data[0] == 0xFE -> vial commands
    let vial_command = VialCommand::from_primitive(report.output_data[1]);
    debug!("Received vial command: {:?}", vial_command);
    match vial_command {
        VialCommand::GetKeyboardId => {
            // Returns vial protocol version + vial keyboard id
            LittleEndian::write_u32(&mut report.input_data[0..4], VIAL_PROTOCOL_VERSION);
            report.input_data[4..12].clone_from_slice(vial_keyboard_Id);
            debug!("Vial return: {:?}", report.input_data);
        }
        VialCommand::GetSize => {
            LittleEndian::write_u32(&mut report.input_data[0..4], vial_keyboard_def.len() as u32);
        }
        VialCommand::GetKeyboardDef => {
            let page = LittleEndian::read_u16(&report.output_data[2..4]) as usize;
            let start = page * VIAL_EP_SIZE;
            let mut end = start + VIAL_EP_SIZE;
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
            // Unlocked
            report.input_data[0] = 1;
            // Unlock in progress
            report.input_data[1] = 0;
        }
        VialCommand::QmkSettingsQuery => {
            report.input_data.fill(0xFF);
        }
        VialCommand::DynamicEntryOp => {
            let vial_dynamic = VialDynamic::from_primitive(report.output_data[2]);
            match vial_dynamic {
                VialDynamic::DynamicVialGetNumberOfEntries => {
                    debug!("DynamicEntryOp - DynamicVialGetNumberOfEntries");
                    report.input_data[0] = core::cmp::min(TAP_DANCE_MAX_NUM, 255) as u8; // Tap dance entries
                    report.input_data[1] = core::cmp::min(COMBO_MAX_NUM, 255) as u8; // Combo entries
                                                                                     // TODO: Support dynamic key override
                    report.input_data[2] = 0; // Key override entries
                }
                VialDynamic::DynamicVialTapDanceGet => {
                    debug!("DynamicEntryOp - DynamicVialTapDanceGet");
                    report.input_data[0] = 0; // Index 0 is the return code, 0 means success

                    let tap_dance_idx = report.output_data[3] as usize;
                    let tap_dances = &keymap.borrow().behavior.tap_dance.tap_dances;
                    if let Some(tap_dance) = tap_dances.get(tap_dance_idx) {
                        // Pack tap dance data into report
                        LittleEndian::write_u16(
                            &mut report.input_data[1..3],
                            to_via_keycode(*tap_dance.tap_actions.get(0).unwrap_or(&KeyAction::No)),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[3..5],
                            to_via_keycode(*tap_dance.hold_actions.get(0).unwrap_or(&KeyAction::No)),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[5..7],
                            to_via_keycode(*tap_dance.tap_actions.get(1).unwrap_or(&KeyAction::No)),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[7..9],
                            to_via_keycode(*tap_dance.hold_actions.get(1).unwrap_or(&KeyAction::No)),
                        );
                        LittleEndian::write_u16(
                            &mut report.input_data[9..11],
                            tap_dance.tapping_term.as_millis() as u16,
                        );
                    } else {
                        report.input_data[1..11].fill(0);
                    }
                }
                VialDynamic::DynamicVialTapDanceSet => {
                    debug!("DynamicEntryOp - DynamicVialTapDanceSet");
                    report.input_data[0] = 0; // Index 0 is the return code, 0 means success

                    use crate::tap_dance::TapDance;
                    let tap_dance_idx = report.output_data[3] as usize;
                    let tap_dances = &mut keymap.borrow_mut().behavior.tap_dance.tap_dances;

                    if tap_dance_idx < tap_dances.len() {
                        // Extract tap dance data from report
                        let tap = from_via_keycode(LittleEndian::read_u16(&report.output_data[4..6]));
                        let hold = from_via_keycode(LittleEndian::read_u16(&report.output_data[6..8]));
                        let double_tap = from_via_keycode(LittleEndian::read_u16(&report.output_data[8..10]));
                        let hold_after_tap = from_via_keycode(LittleEndian::read_u16(&report.output_data[10..12]));
                        let tapping_term_ms = LittleEndian::read_u16(&report.output_data[12..14]);

                        let new_tap_dance = TapDance::new_from_vial(
                            tap,
                            hold,
                            hold_after_tap,
                            double_tap,
                            embassy_time::Duration::from_millis(tapping_term_ms as u64),
                        );

                        // Update the tap dance in keymap
                        if let Some(tap_dance) = tap_dances.get_mut(tap_dance_idx) {
                            *tap_dance = new_tap_dance.clone();
                        }

                        #[cfg(feature = "storage")]
                        {
                            // Save to storage
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::WriteTapDance(tap_dance_idx as u8, new_tap_dance))
                                .await;
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
                            if action != KeyAction::No {
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
                        .send(FlashOperationMessage::WriteCombo(ComboData {
                            idx: real_idx,
                            actions,
                            output,
                        }))
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
            let _encoder = if let Some(ref mut encoder_map) = keymap.borrow_mut().encoders {
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
            } else {
                None
            };

            #[cfg(feature = "storage")]
            // Save the encoder action to the storage after the RefCell is released
            if let Some(encoder) = _encoder {
                // Save the encoder action to the storage
                FLASH_CHANNEL
                    .send(FlashOperationMessage::EncoderKey {
                        idx: index,
                        layer,
                        action: encoder,
                    })
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
