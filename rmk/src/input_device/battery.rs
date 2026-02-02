use core::cell::RefCell;

use embassy_sync::signal::Signal;
use embedded_hal::digital::InputPin;
use rmk_macro::{input_device, input_processor};

use crate::event::{BatteryEvent, ChargingStateEvent};
#[cfg(all(feature = "controller", feature = "_ble"))]
use crate::event::{BatteryLevelEvent, publish_controller_event};
use crate::KeyMap;

pub(crate) static BATTERY_UPDATE: Signal<crate::RawMutex, BatteryState> = Signal::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BatteryState {
    // The battery state is not available
    NotAvailable,
    // The value range is 0~100
    Normal(u8),
    // Charging
    Charging,
    // Charging completed, ideally the battery level after charging completed is 100
    Charged,
}

/// Reads charging state from a GPIO pin and publishes ChargingStateEvent.
///
/// This input device monitors a charging state pin and publishes events when
/// the charging state changes.
#[input_device(publish = ChargingStateEvent)]
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

    /// Read the charging state and return an event.
    /// This method waits until there's a state change to report.
    async fn read_charging_state_event(&mut self) -> ChargingStateEvent {
        // For the first read, don't check whether the charging state is changed
        if !self.first_read {
            // Wait 2s before reading the first value
            embassy_time::Timer::after_secs(2).await;
            let charging_state = if self.low_active {
                self.state_input.is_low().unwrap_or(false)
            } else {
                self.state_input.is_high().unwrap_or(false)
            };
            self.current_charging_state = charging_state;
            self.first_read = true;
            return ChargingStateEvent { charging: charging_state };
        }

        loop {
            // Check charging state every 5 seconds
            embassy_time::Timer::after_secs(5).await;

            // Detect charging state
            let charging_state = if self.low_active {
                self.state_input.is_low().unwrap_or(false)
            } else {
                self.state_input.is_high().unwrap_or(false)
            };

            // Only return event when charging state changes
            if charging_state != self.current_charging_state {
                self.current_charging_state = charging_state;
                return ChargingStateEvent { charging: charging_state };
            }
        }
    }
}

#[input_processor(subscribe = [BatteryEvent, ChargingStateEvent])]
pub struct BatteryProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    adc_divider_measured: u32,
    adc_divider_total: u32,
    /// Current battery state
    battery_state: BatteryState,
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
            battery_state: BatteryState::NotAvailable,
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
    BatteryProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn on_battery_event(&mut self, event: BatteryEvent) {
        let val = event.0;
        trace!("Detected battery ADC value: {:?}", val);

        #[cfg(feature = "_ble")]
        {
            if matches!(self.battery_state, BatteryState::Normal(_) | BatteryState::NotAvailable) {
                let battery_percent = self.get_battery_percent(val);

                #[cfg(feature = "controller")]
                publish_controller_event(BatteryLevelEvent { level: battery_percent });

                // Update the battery state
                if self.battery_state != BatteryState::Normal(battery_percent) {
                    self.battery_state = BatteryState::Normal(battery_percent);
                    // Send signal
                    BATTERY_UPDATE.signal(self.battery_state);
                }
            }
        }
    }

    async fn on_charging_state_event(&mut self, event: ChargingStateEvent) {
        let charging = event.charging;
        info!("Charging state changed: {:?}", charging);

        #[cfg(feature = "_ble")]
        {
            #[cfg(feature = "controller")]
            publish_controller_event(ChargingStateEvent { charging });

            if charging {
                self.battery_state = BatteryState::Charging;
            } else {
                // When discharging, the battery state is changed to not available
                // Then wait for the `Event::Battery` to update the battery level to real value
                self.battery_state = BatteryState::NotAvailable;
            }
        }
    }
}
