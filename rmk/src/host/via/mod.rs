use byteorder::{BigEndian, ByteOrder, LittleEndian};
use embassy_time::Instant;
use rmk_types::protocol::vial::{VIA_FIRMWARE_VERSION, VIA_PROTOCOL_VERSION, ViaCommand, ViaKeyboardInfo};
use vial::process_vial;

use crate::channel::{HOST_REQUEST_CHANNEL, try_send_host_reply};
use crate::config::{RmkConfig, VialConfig};
use crate::core_traits::Runnable;
use crate::hid::ViaReport;
use crate::host::context::KeyboardContext;
use crate::host::via::keycode_convert::{from_via_keycode, to_via_keycode};
use crate::{MACRO_SPACE_SIZE, boot};

pub(crate) mod keycode_convert;
mod vial;
#[cfg(feature = "vial_lock")]
mod vial_lock;

pub struct VialService<'a> {
    ctx: &'a KeyboardContext<'a>,
    vial_config: VialConfig<'static>,
    #[cfg(feature = "vial_lock")]
    locker: vial_lock::VialLock<'a>,
}

impl<'a> VialService<'a> {
    pub fn new(ctx: &'a KeyboardContext<'a>, config: &RmkConfig<'static>) -> Self {
        Self {
            ctx,
            vial_config: config.vial_config,
            #[cfg(feature = "vial_lock")]
            locker: vial_lock::VialLock::new(config.vial_config.unlock_keys, ctx.keymap),
        }
    }

    async fn process_via_packet(&mut self, report: &mut ViaReport) {
        let command_id = report.output_data[0];

        // Caller pre-fills `input_data` from `output_data`, so individual arms
        // only need to overwrite the bytes they actually change.
        match command_id.into() {
            ViaCommand::GetProtocolVersion => {
                BigEndian::write_u16(&mut report.input_data[1..3], VIA_PROTOCOL_VERSION);
            }
            ViaCommand::GetKeyboardValue => {
                // Check the second u8
                match report.output_data[1].try_into() {
                    Ok(v) => match v {
                        ViaKeyboardInfo::Uptime => {
                            let value = Instant::now().as_millis() as u32;
                            BigEndian::write_u32(&mut report.input_data[2..6], value);
                        }
                        ViaKeyboardInfo::LayoutOptions => {
                            // TODO: retrieve layout option from storage
                            let layout_option: u32 = 0;
                            BigEndian::write_u32(&mut report.input_data[2..6], layout_option);
                        }
                        #[cfg(not(feature = "vial_lock"))]
                        ViaKeyboardInfo::SwitchMatrixState => {
                            error!("It is not secure to use matrix tester without vial lock");
                        }
                        #[cfg(feature = "vial_lock")]
                        ViaKeyboardInfo::SwitchMatrixState if self.locker.is_unlocked() => {
                            self.ctx.read_matrix_state(&mut report.input_data[2..]);
                        }
                        ViaKeyboardInfo::FirmwareVersion => {
                            BigEndian::write_u32(&mut report.input_data[2..6], VIA_FIRMWARE_VERSION);
                        }
                        _ => (),
                    },
                    Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e),
                }
            }
            ViaCommand::SetKeyboardValue => {
                // Check the second u8
                match report.output_data[1].try_into() {
                    Ok(v) => match v {
                        ViaKeyboardInfo::LayoutOptions => {
                            let layout_option = BigEndian::read_u32(&report.output_data[2..6]);
                            self.ctx.set_layout_options(layout_option).await;
                        }
                        ViaKeyboardInfo::DeviceIndication => {
                            let _device_indication = report.output_data[2];
                            warn!("SetKeyboardValue - DeviceIndication")
                        }
                        _ => (),
                    },
                    Err(e) => error!("Invalid subcommand: {} of GetKeyboardValue", e),
                }
            }
            ViaCommand::DynamicKeymapGetKeyCode => {
                let layer = report.output_data[1];
                let row = report.output_data[2];
                let col = report.output_data[3];
                let action = self.ctx.get_action(layer, row, col);
                let keycode = to_via_keycode(action);
                info!("Getting keycode: {:02X} at ({},{}), layer {}", keycode, row, col, layer);
                BigEndian::write_u16(&mut report.input_data[4..6], keycode);
            }
            ViaCommand::DynamicKeymapSetKeyCode => {
                let layer = report.output_data[1];
                let row = report.output_data[2];
                let col = report.output_data[3];
                let keycode = BigEndian::read_u16(&report.output_data[4..6]);
                let action = from_via_keycode(keycode);
                info!(
                    "Setting keycode: 0x{:02X} at ({},{}), layer {} as {:?}",
                    keycode, row, col, layer, action
                );
                self.ctx.set_action(layer, row, col, action).await;
            }
            ViaCommand::DynamicKeymapReset => {
                warn!("Dynamic keymap reset -- not supported")
            }
            ViaCommand::CustomSetValue => {
                // backlight/rgblight/rgb matrix/led matrix/audio settings here
                warn!("Custom set value -- not supported")
            }
            ViaCommand::CustomGetValue => {
                // backlight/rgblight/rgb matrix/led matrix/audio settings here
                warn!("Custom get value -- not supported")
            }
            ViaCommand::CustomSave => {
                // backlight/rgblight/rgb matrix/led matrix/audio settings here
                warn!("Custom get value -- not supported")
            }
            ViaCommand::EepromReset => {
                warn!("Resetting storage..");
                self.ctx.reset_storage().await;
                // TODO: Reboot after a eeprom reset?
            }
            ViaCommand::BootloaderJump => {
                warn!("Bootloader jumping");
                boot::jump_to_bootloader();
            }
            ViaCommand::DynamicKeymapMacroGetCount => {
                report.input_data[1] = 32;
                warn!("Macro get count -- to be implemented")
            }
            ViaCommand::DynamicKeymapMacroGetBufferSize => {
                report.input_data[1] = (MACRO_SPACE_SIZE as u16 >> 8) as u8;
                report.input_data[2] = (MACRO_SPACE_SIZE & 0xFF) as u8;
            }
            ViaCommand::DynamicKeymapMacroGetBuffer => {
                let offset = BigEndian::read_u16(&report.output_data[1..3]) as usize;
                let size = report.output_data[3] as usize;
                if size <= 28 {
                    self.ctx.read_macro_buffer(offset, &mut report.input_data[4..4 + size]);
                    debug!("Get macro buffer: offset: {}, data: {:?}", offset, report.input_data);
                } else {
                    report.input_data[0] = 0xFF;
                }
            }
            ViaCommand::DynamicKeymapMacroSetBuffer => {
                // Every write writes all buffer space of the macro(if it's not empty)
                let offset = BigEndian::read_u16(&report.output_data[1..3]);
                // Current sequence size, <= 28
                let size = report.output_data[3];
                // End of current sequence in the macro cache
                // The first sequence, reset the macro cache
                if offset == 0 {
                    self.ctx.reset_macro_buffer();
                }

                // Update macro cache + flush full buffer to storage
                info!("Setting macro buffer, offset: {}, size: {}", offset, size);
                self.ctx
                    .write_macro_buffer(offset as usize, &report.output_data[4..4 + size as usize])
                    .await;
            }
            ViaCommand::DynamicKeymapMacroReset => {
                warn!("Macro reset -- to be implemented")
            }
            ViaCommand::DynamicKeymapGetLayerCount => {
                report.input_data[1] = self.ctx.keymap_dimensions().2 as u8;
            }
            ViaCommand::DynamicKeymapGetBuffer => {
                let offset = BigEndian::read_u16(&report.output_data[1..3]);
                // size <= 28
                let size = report.output_data[3];
                debug!("Getting keymap buffer, offset: {}, size: {}", offset, size);
                let mut idx = 4;
                let start = (offset / 2) as usize;
                let count = (size / 2) as usize;
                for i in 0..count {
                    let a = self.ctx.get_action_flat(start + i);
                    let kc = to_via_keycode(a);
                    BigEndian::write_u16(&mut report.input_data[idx..idx + 2], kc);
                    idx += 2;
                }
            }
            ViaCommand::DynamicKeymapSetBuffer => {
                debug!("Dynamic keymap set buffer");
                let offset = BigEndian::read_u16(&report.output_data[1..3]);
                // size <= 28
                let size = report.output_data[3];
                let mut idx = 4;
                let (rows, cols, _) = self.ctx.keymap_dimensions();
                for i in 0..(size as usize) {
                    let via_keycode = LittleEndian::read_u16(&report.output_data[idx..idx + 2]);
                    let action = from_via_keycode(via_keycode);
                    let flat_index = offset as usize + i;
                    self.ctx.try_set_action_flat(flat_index, action, rows, cols);
                    idx += 2;
                }
            }
            ViaCommand::DynamicKeymapGetEncoder => {
                warn!("Keymap get encoder -- not supported");
            }
            ViaCommand::DynamicKeymapSetEncoder => {
                warn!("Keymap set encoder -- not supported");
            }
            ViaCommand::Vial => {
                process_vial(
                    report,
                    &self.vial_config,
                    #[cfg(feature = "vial_lock")]
                    &mut self.locker,
                    self.ctx,
                )
                .await
            }
            ViaCommand::Unhandled => {
                info!("Unknown cmd: {:?}", report.output_data);
                report.input_data[0] = ViaCommand::Unhandled as u8
            }
        }
    }
}

impl Runnable for VialService<'_> {
    async fn run(&mut self) -> ! {
        loop {
            let (transport, output_data) = HOST_REQUEST_CHANNEL.receive().await;
            let mut report = ViaReport {
                input_data: output_data,
                output_data,
            };
            self.process_via_packet(&mut report).await;
            try_send_host_reply(transport, report.input_data);
        }
    }
}
