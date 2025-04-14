use core::cell::RefCell;
use core::sync::atomic::AtomicU8;

use super::InputProcessor;
use crate::event::Event;
use crate::input_device::ProcessResult;
use crate::KeyMap;

// 0: normal, 1: low, 2: charging
pub static BATTERY_STATE: AtomicU8 = AtomicU8::new(0);

pub struct BatteryProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    adc_divider_measured: u32,
    adc_divider_total: u32,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    BatteryProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(
        adc_divider_measured: u32,
        adc_divider_total: u32,
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    ) -> Self {
        BatteryProcessor {
            keymap,
            adc_divider_measured,
            adc_divider_total,
        }
    }

    #[cfg(feature = "_ble")]
    fn get_battery_percent(&self, val: u16) -> u8 {
        // Avoid overflow
        let val = val as i32;

        // According to nRF52840's datasheet, for single_ended saadc:
        // val = v_adc * (gain / reference) * 2^(resolution)
        //
        // When using default setting, gain = 1/6, reference = 0.6v, resolution = 12bits, so:
        // val = v_adc * 1137.8
        //
        // For example, rmk-ble-keyboard uses two resistors 820K and 2M adjusting the v_adc, then,
        // v_adc = v_bat * measured / total => val = v_bat * 1137.8 * measured / total
        //
        // If the battery voltage range is 3.6v ~ 4.2v, the adc val range should be (4096 ~ 4755) * measured / total
        let mut measured = self.adc_divider_measured as i32;
        let mut total = self.adc_divider_total as i32;
        if 500 < val && val < 1200 {
            // Thing becomes different when using vddh as reference
            // The adc value for vddh pin is actually vddh/5,
            // so we use this rough range to detect vddh
            measured = 1;
            total = 5;
        }
        if val > 4755_i32 * measured / total {
            // 4755 ~= 4.2v * 1137.8
            100_u8
        } else if val < 4055_i32 * measured / total {
            // 4096 ~= 3.6v * 1137.8
            // To simplify the calculation, we use 4055 here
            0_u8
        } else {
            ((val * total / measured - 4055) / 7) as u8
        }
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER> for BatteryProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Battery(val) => {
                info!("Detected battery ADC value: {:?}", val);

                #[cfg(feature = "_ble")]
                {
                    let battery_percent = self.get_battery_percent(val);
                    crate::ble::trouble::battery_service::BATTERY_LEVEL
                        .store(battery_percent, core::sync::atomic::Ordering::Relaxed);

                    // 检查当前是否为USB连接
                    match crate::state::CURRENT_CONNECTION
                        .load(core::sync::atomic::Ordering::Relaxed)
                        .into()
                    {
                        crate::state::ConnectionType::Usb => {
                            // USB连接，充电中：状态2
                            BATTERY_STATE.store(2, core::sync::atomic::Ordering::Relaxed);
                        }
                        crate::state::ConnectionType::Ble => {
                            if battery_percent < 10 {
                                // 电量低于10%，USB未连接：状态1
                                BATTERY_STATE.store(1, core::sync::atomic::Ordering::Relaxed);
                            } else {
                                // 电量正常(>10%)且USB未连接：状态0
                                BATTERY_STATE.store(0, core::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                }
                ProcessResult::Stop
            }
            _ => ProcessResult::Continue(event),
        }
    }

    /// Get the current keymap
    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        return self.keymap;
    }
}
