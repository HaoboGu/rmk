use core::cell::RefCell;

use embedded_hal::digital::InputPin;
#[cfg(feature = "controller")]
use {
    crate::channel::{send_controller_event, ControllerPub, CONTROLLER_CHANNEL},
    crate::event::ControllerEvent,
};

use super::{InputDevice, InputProcessor};
use crate::event::Event;
use crate::input_device::ProcessResult;
use crate::KeyMap;

pub struct ChargingStateReader<I: InputPin> {
    // Charging state pin or standby pin
    state_input: I,
    // True: low represents charging, False: high represents charging
    low_active: bool,
    // True: charging, False: not charging
    current_charging_state: bool,
    // First read done
    first_read: bool,
}

impl<I: InputPin> ChargingStateReader<I> {
    pub fn new(state_input: I, low_active: bool) -> Self {
        Self {
            state_input,
            low_active,
            current_charging_state: false,
            first_read: false,
        }
    }
}

impl<I: InputPin> InputDevice for ChargingStateReader<I> {
    async fn read_event(&mut self) -> Event {
        // For the first read, don't check whether the charging state is changed
        if !self.first_read {
            let charging_state = self.state_input.is_low().unwrap_or(false);
            self.current_charging_state = charging_state;
            self.first_read = true;
            return Event::ChargingState(charging_state);
        }

        loop {
            // Detect charging state
            let charging_state = self.state_input.is_low().unwrap_or(false);

            // Only send event when charging state changes
            if charging_state != self.current_charging_state {
                self.current_charging_state = charging_state;
                return Event::ChargingState(charging_state);
            }

            // Check charging state every 5 seconds
            embassy_time::Timer::after_secs(5).await;
        }
    }
}

pub struct BatteryProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    adc_divider_measured: u32,
    adc_divider_total: u32,
    /// Publisher for controller channel
    #[cfg(feature = "controller")]
    controller_pub: ControllerPub,
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
            #[cfg(feature = "controller")]
            controller_pub: unwrap!(CONTROLLER_CHANNEL.publisher()),
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
        if 500 < val && val < 1000 {
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
                debug!("Detected battery ADC value: {:?}", val);

                #[cfg(feature = "controller")]
                send_controller_event(&mut self.controller_pub, ControllerEvent::Battery(val));

                #[cfg(feature = "_ble")]
                {
                    let current_value =
                        crate::ble::trouble::battery_service::BATTERY_LEVEL.load(core::sync::atomic::Ordering::Relaxed);
                    if current_value < 100 || current_value == 255 {
                        // When charging, don't update the battery level(which is inaccurate)
                        crate::ble::trouble::battery_service::BATTERY_LEVEL
                            .store(self.get_battery_percent(val), core::sync::atomic::Ordering::Relaxed);
                    }
                }
                ProcessResult::Stop
            }
            Event::ChargingState(charging) => {
                info!("Charging state changed: {:?}", charging);

                #[cfg(feature = "controller")]
                send_controller_event(&mut self.controller_pub, ControllerEvent::ChargingState(charging));

                #[cfg(feature = "_ble")]
                {
                    if charging {
                        crate::ble::trouble::battery_service::BATTERY_LEVEL
                            .store(101, core::sync::atomic::Ordering::Relaxed);
                    } else {
                        // When discharging, the battery level is changed to 255(not available)
                        // Then wait for the `Event::Battery` to update the battery level to real value
                        crate::ble::trouble::battery_service::BATTERY_LEVEL
                            .store(255, core::sync::atomic::Ordering::Relaxed);
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
