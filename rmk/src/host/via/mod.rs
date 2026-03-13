use byteorder::{BigEndian, ByteOrder, LittleEndian};
use embassy_time::{Instant, Timer};
use embassy_usb::class::hid::HidReaderWriter;
use embassy_usb::driver::Driver;
use rmk_types::protocol::vial::{VIA_FIRMWARE_VERSION, VIA_PROTOCOL_VERSION, ViaCommand, ViaKeyboardInfo};
use ssmarshal::serialize;
use vial::process_vial;

use crate::config::VialConfig;
use crate::descriptor::ViaReport;
use crate::event::KeyboardEventPos;
use crate::hid::{HidError, HidReaderTrait, HidWriterTrait};
#[cfg(feature = "storage")]
use crate::host::storage::{KeymapData, KeymapKey};
use crate::host::via::keycode_convert::{from_via_keycode, to_via_keycode};
use crate::keymap::KeyMap;
use crate::state::ConnectionState;
use crate::{CONNECTION_STATE, MACRO_SPACE_SIZE, boot};
#[cfg(feature = "storage")]
use crate::{channel::FLASH_CHANNEL, storage::FlashOperationMessage};

pub(crate) mod keycode_convert;
mod vial;
#[cfg(feature = "vial_lock")]
mod vial_lock;

pub(crate) struct VialService<'a, RW: HidWriterTrait<ReportType = ViaReport> + HidReaderTrait<ReportType = ViaReport>> {
    // VialService holds a reference of keymap, for updating
    keymap: &'a KeyMap<'a>,

    // Vial config
    vial_config: VialConfig<'static>,

    // Vial lock instance
    #[cfg(feature = "vial_lock")]
    locker: vial_lock::VialLock<'a>,

    // Usb vial hid reader writer
    pub(crate) reader_writer: RW,
}

impl<'a, RW: HidWriterTrait<ReportType = ViaReport> + HidReaderTrait<ReportType = ViaReport>> VialService<'a, RW> {
    // VialService::new() should be called only once.
    // Otherwise the `vial_buf.init()` will panic.
    pub(crate) fn new(keymap: &'a KeyMap<'a>, vial_config: VialConfig<'static>, reader_writer: RW) -> Self {
        Self {
            keymap,
            vial_config,
            #[cfg(feature = "vial_lock")]
            locker: vial_lock::VialLock::new(vial_config.unlock_keys, keymap),
            reader_writer,
        }
    }

    pub(crate) async fn run(&mut self) {
        loop {
            match self.process().await {
                Ok(_) => continue,
                Err(e) => {
                    if ConnectionState::Disconnected == ConnectionState::from_atomic(&CONNECTION_STATE) {
                        Timer::after_millis(1000).await;
                    } else {
                        error!("Process vial error: {:?}", e);
                        Timer::after_millis(10000).await;
                    }
                }
            }
        }
    }

    pub(crate) async fn process(&mut self) -> Result<(), HidError> {
        let mut via_report = self.reader_writer.read_report().await?;

        self.process_via_packet(&mut via_report, self.keymap).await;

        // Send via report back after processing
        self.reader_writer.write_report(via_report).await?;

        Ok(())
    }

    async fn process_via_packet(&mut self, report: &mut ViaReport, keymap: &KeyMap<'_>) {
        let command_id = report.output_data[0];

        // `report.input_data` is initialized using `report.output_data`
        report.input_data = report.output_data;
        // debug!("Received via command: {}, report: {:02X?}", via_command, report.output_data);
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
                        ViaKeyboardInfo::SwitchMatrixState => {
                            #[cfg(feature = "vial_lock")]
                            if self.locker.is_unlocked() {
                                self.keymap.read_matrix_state(&mut report.input_data[2..]);
                            }

                            #[cfg(all(feature = "host_security", not(feature = "vial_lock")))]
                            {
                                self.keymap.read_matrix_state(&mut report.input_data[2..]);
                                error!("It is not secure to use matrix tester without vial lock");
                            }
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
                        #[cfg(feature = "storage")]
                        ViaKeyboardInfo::LayoutOptions => {
                            let layout_option = BigEndian::read_u32(&report.output_data[2..6]);
                            FLASH_CHANNEL
                                .send(FlashOperationMessage::LayoutOptions(layout_option))
                                .await;
                        }
                        ViaKeyboardInfo::DeviceIndication => {
                            let _device_indication = report.output_data[2];
                            warn!("SetKeyboardValue - DeviceIndication")
                        }
                        _ => (),
                    },
                    Err(e) => error!("Invalid subcommand: {} of SetKeyboardValue", e),
                }
            }
            ViaCommand::DynamicKeymapGetKeyCode => {
                let layer = report.output_data[1] as usize;
                let row = report.output_data[2] as usize;
                let col = report.output_data[3] as usize;
                let action = keymap.get_action_at(KeyboardEventPos::key_pos(col as u8, row as u8), layer);
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
                keymap.set_action_at(KeyboardEventPos::key_pos(col, row), layer as usize, action);
                #[cfg(feature = "storage")]
                FLASH_CHANNEL
                    .send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(KeymapKey {
                        layer,
                        col,
                        row,
                        action,
                    })))
                    .await;
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
                #[cfg(feature = "storage")]
                FLASH_CHANNEL.send(FlashOperationMessage::Reset).await
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
                    self.keymap
                        .read_macro_buffer(offset, &mut report.input_data[4..4 + size]);
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
                    self.keymap.reset_macro_buffer();
                }

                // Update macro cache
                info!("Setting macro buffer, offset: {}, size: {}", offset, size);
                self.keymap
                    .write_macro_buffer(offset as usize, &report.output_data[4..4 + size as usize]);

                // Then flush macros to storage
                #[cfg(feature = "storage")]
                {
                    let buf = self.keymap.get_macro_sequences();
                    FLASH_CHANNEL
                        .send(FlashOperationMessage::HostMessage(KeymapData::Macro(buf)))
                        .await;
                    info!("Flush macros to storage")
                }
            }
            ViaCommand::DynamicKeymapMacroReset => {
                warn!("Macro reset -- to be implemented")
            }
            ViaCommand::DynamicKeymapGetLayerCount => {
                report.input_data[1] = keymap.get_keymap_config().2 as u8;
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
                    let a = keymap.get_action_by_flat_index(start + i);
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
                let (row_num, col_num, _layer_num) = keymap.get_keymap_config();
                for i in 0..(size as usize) {
                    let via_keycode = LittleEndian::read_u16(&report.output_data[idx..idx + 2]);
                    let action: rmk_types::action::KeyAction = from_via_keycode(via_keycode);
                    let flat_index = offset as usize + i;
                    keymap.set_action_by_flat_index(flat_index, action);
                    idx += 2;
                    let (row, col, layer) = get_position_from_offset(flat_index, row_num, col_num);
                    info!(
                        "Setting keymap buffer of offset: {}, row,col,layer: {},{},{}",
                        offset, row, col, layer
                    );
                    #[cfg(feature = "storage")]
                    if let Err(_e) =
                        FLASH_CHANNEL.try_send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(KeymapKey {
                            layer: layer as u8,
                            col: col as u8,
                            row: row as u8,
                            action,
                        })))
                    {
                        error!("Send keymap setting command error")
                    }
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
                    keymap,
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

fn get_position_from_offset(offset: usize, max_row: usize, max_col: usize) -> (usize, usize, usize) {
    let layer = offset / (max_col * max_row);
    let current_layer_offset = offset % (max_col * max_row);
    let row = current_layer_offset / max_col;
    let col = current_layer_offset % max_col;
    (row, col, layer)
}

pub struct UsbVialReaderWriter<'a, 'd, D: Driver<'d>> {
    pub(crate) vial_reader_writer: &'a mut HidReaderWriter<'d, D, 32, 32>,
}

impl<'a, 'd, D: Driver<'d>> UsbVialReaderWriter<'a, 'd, D> {
    pub(crate) fn new(vial_reader_writer: &'a mut HidReaderWriter<'d, D, 32, 32>) -> Self {
        Self { vial_reader_writer }
    }
}

impl<'d, D: Driver<'d>> HidWriterTrait for UsbVialReaderWriter<'_, 'd, D> {
    type ReportType = ViaReport;

    async fn write_report(&mut self, report: Self::ReportType) -> Result<usize, HidError> {
        let mut buffer = [0u8; 32];
        let n = serialize(&mut buffer, &report).map_err(|_| HidError::ReportSerializeError)?;
        self.vial_reader_writer
            .write(&buffer[0..n])
            .await
            .map_err(HidError::UsbEndpointError)?;
        Ok(n)
    }
}

impl<'d, D: Driver<'d>> HidReaderTrait for UsbVialReaderWriter<'_, 'd, D> {
    type ReportType = ViaReport;

    async fn read_report(&mut self) -> Result<ViaReport, HidError> {
        let mut read_report = ViaReport {
            input_data: [0; 32],
            output_data: [0; 32],
        };
        self.vial_reader_writer
            .read(&mut read_report.output_data)
            .await
            .map_err(HidError::UsbReadError)?;

        Ok(read_report)
    }
}

impl<'a, RW: HidWriterTrait<ReportType = ViaReport> + HidReaderTrait<ReportType = ViaReport>> crate::host::HostService
    for VialService<'a, RW>
{
    async fn run(&mut self) {
        VialService::run(self).await;
    }
}
