use rmk_types::protocol::vial::{SettingKey, ViaCommand, VialCommand, VialDynamic};

use super::{SimHost, SimKeyboard};
use crate::host::via::keycode_convert::to_via_keycode;
use crate::types::action::{EncoderAction, KeyAction};

impl SimHost {
    pub fn vial<'k, 'a>(&self, keyboard: &'k mut SimKeyboard<'a>) -> SimVial<'k, 'a> {
        keyboard.enable_host();
        SimVial { keyboard }
    }
}

#[must_use = "host transactions must end with an expectation"]
pub struct SimHostReply<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
    data: [u8; 32],
}

impl<'k, 'a> SimHostReply<'k, 'a> {
    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(self.data, self.data);
        self.keyboard
    }

    pub fn expect(self, reply: [u8; 32]) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(self.data, reply);
        self.keyboard
    }
}

pub struct SimVial<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
}

impl<'k, 'a> SimVial<'k, 'a> {
    pub fn raw(self, data: [u8; 32]) -> SimHostReply<'k, 'a> {
        SimHostReply {
            keyboard: self.keyboard,
            data,
        }
    }

    pub fn get_protocol_version(self) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::GetProtocolVersion as u8;
        self.raw(data)
    }

    pub fn get_key(self, layer: u8, row: u8, col: u8) -> SimVialKeyReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::DynamicKeymapGetKeyCode as u8;
        data[1] = layer;
        data[2] = row;
        data[3] = col;
        SimVialKeyReply { reply: self.raw(data) }
    }

    pub fn set_key(self, layer: u8, row: u8, col: u8, action: KeyAction) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::DynamicKeymapSetKeyCode as u8;
        data[1] = layer;
        data[2] = row;
        data[3] = col;
        data[4..6].copy_from_slice(&to_via_keycode(action).to_be_bytes());
        self.raw(data)
    }

    pub fn get_encoder(self, layer: u8, encoder_id: u8) -> SimVialEncoderReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::GetEncoder as u8;
        data[2] = layer;
        data[3] = encoder_id;
        SimVialEncoderReply { reply: self.raw(data) }
    }

    pub fn set_encoder(self, layer: u8, encoder_id: u8, action: EncoderAction) -> SimVialSetEncoderReply<'k, 'a> {
        let mut clockwise = [0u8; 32];
        clockwise[0] = ViaCommand::Vial as u8;
        clockwise[1] = VialCommand::SetEncoder as u8;
        clockwise[2] = layer;
        clockwise[3] = encoder_id;
        clockwise[4] = 1;
        clockwise[5..7].copy_from_slice(&to_via_keycode(action.clockwise).to_be_bytes());

        let mut counter_clockwise = [0u8; 32];
        counter_clockwise[0] = ViaCommand::Vial as u8;
        counter_clockwise[1] = VialCommand::SetEncoder as u8;
        counter_clockwise[2] = layer;
        counter_clockwise[3] = encoder_id;
        counter_clockwise[4] = 0;
        counter_clockwise[5..7].copy_from_slice(&to_via_keycode(action.counter_clockwise).to_be_bytes());

        SimVialSetEncoderReply {
            keyboard: self.keyboard,
            clockwise,
            counter_clockwise,
        }
    }

    pub fn get_behavior_setting(self, setting: SettingKey) -> SimVialBehaviorSettingReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::GetBehaviorSetting as u8;
        data[2..4].copy_from_slice(&(setting as u16).to_le_bytes());
        SimVialBehaviorSettingReply { reply: self.raw(data) }
    }

    pub fn set_behavior_setting_u16(self, setting: SettingKey, value: u16) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::SetBehaviorSetting as u8;
        data[2..4].copy_from_slice(&(setting as u16).to_le_bytes());
        data[4..6].copy_from_slice(&value.to_le_bytes());
        self.raw(data)
    }

    pub fn get_morse(self, index: u8) -> SimVialMorseReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialMorseGet as u8;
        data[3] = index;
        SimVialMorseReply { reply: self.raw(data) }
    }

    pub fn set_morse(
        self,
        index: u8,
        tap: KeyAction,
        hold: KeyAction,
        double_tap: KeyAction,
        hold_after_tap: KeyAction,
        timeout_ms: u16,
    ) -> SimVialDynamicSetReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialMorseSet as u8;
        data[3] = index;
        data[4..6].copy_from_slice(&to_via_keycode(tap).to_le_bytes());
        data[6..8].copy_from_slice(&to_via_keycode(hold).to_le_bytes());
        data[8..10].copy_from_slice(&to_via_keycode(double_tap).to_le_bytes());
        data[10..12].copy_from_slice(&to_via_keycode(hold_after_tap).to_le_bytes());
        data[12..14].copy_from_slice(&timeout_ms.to_le_bytes());
        SimVialDynamicSetReply { reply: self.raw(data) }
    }

    pub fn get_combo(self, index: u8) -> SimVialComboReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialComboGet as u8;
        data[3] = index;
        SimVialComboReply { reply: self.raw(data) }
    }

    pub fn set_combo<const N: usize>(
        self,
        index: u8,
        actions: [KeyAction; N],
        output: KeyAction,
    ) -> SimVialDynamicSetReply<'k, 'a> {
        assert!(
            N <= crate::COMBO_MAX_LENGTH,
            "simulator combo helper received too many actions"
        );

        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialComboSet as u8;
        data[3] = index;
        for (idx, action) in actions.into_iter().enumerate() {
            let start = 4 + idx * 2;
            data[start..start + 2].copy_from_slice(&to_via_keycode(action).to_le_bytes());
        }
        let output_start = 4 + crate::COMBO_MAX_LENGTH * 2;
        data[output_start..output_start + 2].copy_from_slice(&to_via_keycode(output).to_le_bytes());

        SimVialDynamicSetReply { reply: self.raw(data) }
    }

    pub fn unsupported_dynamic_entry(self) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::Unhandled as u8;
        self.raw(data)
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialKeyReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

impl<'k, 'a> SimVialKeyReply<'k, 'a> {
    pub fn expect(self, action: KeyAction) -> &'k mut SimKeyboard<'a> {
        let mut expected = self.reply.data;
        expected[4..6].copy_from_slice(&to_via_keycode(action).to_be_bytes());
        self.reply.expect(expected)
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialEncoderReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

impl<'k, 'a> SimVialEncoderReply<'k, 'a> {
    pub fn expect(self, action: EncoderAction) -> &'k mut SimKeyboard<'a> {
        let mut expected = [0u8; 32];
        expected[0..2].copy_from_slice(&to_via_keycode(action.counter_clockwise).to_be_bytes());
        expected[2..4].copy_from_slice(&to_via_keycode(action.clockwise).to_be_bytes());
        self.reply.expect(expected)
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialSetEncoderReply<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
    clockwise: [u8; 32],
    counter_clockwise: [u8; 32],
}

impl<'k, 'a> SimVialSetEncoderReply<'k, 'a> {
    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(self.clockwise, self.clockwise);
        self.keyboard
            .vial_packet(self.counter_clockwise, self.counter_clockwise);
        self.keyboard
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialBehaviorSettingReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

impl<'k, 'a> SimVialBehaviorSettingReply<'k, 'a> {
    pub fn expect_u16(self, value: u16) -> &'k mut SimKeyboard<'a> {
        let mut expected = [0xFF; 32];
        expected[0] = 0;
        expected[1..3].copy_from_slice(&value.to_le_bytes());
        self.reply.expect(expected)
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialMorseReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

impl<'k, 'a> SimVialMorseReply<'k, 'a> {
    pub fn expect(
        self,
        tap: KeyAction,
        hold: KeyAction,
        double_tap: KeyAction,
        hold_after_tap: KeyAction,
        timeout_ms: u16,
    ) -> &'k mut SimKeyboard<'a> {
        let mut expected = self.reply.data;
        expected[0] = 0;
        expected[1..3].copy_from_slice(&to_via_keycode(tap).to_le_bytes());
        expected[3..5].copy_from_slice(&to_via_keycode(hold).to_le_bytes());
        expected[5..7].copy_from_slice(&to_via_keycode(double_tap).to_le_bytes());
        expected[7..9].copy_from_slice(&to_via_keycode(hold_after_tap).to_le_bytes());
        expected[9..11].copy_from_slice(&timeout_ms.to_le_bytes());
        self.reply.expect(expected)
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialComboReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

impl<'k, 'a> SimVialComboReply<'k, 'a> {
    pub fn expect<const N: usize>(self, actions: [KeyAction; N], output: KeyAction) -> &'k mut SimKeyboard<'a> {
        assert!(
            N <= crate::COMBO_MAX_LENGTH,
            "simulator combo helper received too many actions"
        );

        let mut expected = self.reply.data;
        expected[0] = 0;
        for (idx, action) in actions.into_iter().enumerate() {
            let start = 1 + idx * 2;
            expected[start..start + 2].copy_from_slice(&to_via_keycode(action).to_le_bytes());
        }
        let output_start = 1 + crate::COMBO_MAX_LENGTH * 2;
        expected[output_start..output_start + 2].copy_from_slice(&to_via_keycode(output).to_le_bytes());
        self.reply.expect(expected)
    }
}

#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialDynamicSetReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

impl<'k, 'a> SimVialDynamicSetReply<'k, 'a> {
    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a> {
        let mut expected = self.reply.data;
        expected[0] = 0;
        self.reply.expect(expected)
    }
}
