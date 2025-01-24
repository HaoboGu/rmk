use core::cell::RefCell;

use crate::action::KeyAction;
#[cfg(feature = "_nrf_ble")]
use crate::ble::nrf::initialize_nrf_sd_and_flash;
use crate::channel::KEY_EVENT_CHANNEL;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::DebounceState;
use crate::debounce::DebouncerTrait;
use crate::event::KeyEvent;
use crate::keyboard::Keyboard;
use crate::keymap::KeyMap;
use crate::light::LightController;
use crate::matrix::KeyState;
use crate::run_rmk_internal;
use crate::storage::Storage;
use crate::KeyboardConfig;
use crate::MatrixTrait;
#[cfg(not(feature = "_esp_ble"))]
use embassy_executor::Spawner;
use embassy_time::Instant;
use embassy_time::Timer;
#[cfg(not(feature = "_no_usb"))]
use embassy_usb::driver::Driver;
use embedded_hal;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(not(feature = "_no_external_storage"))]
use embedded_storage::nor_flash::NorFlash;
#[cfg(not(feature = "_no_external_storage"))]
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
#[cfg(feature = "_esp_ble")]
use esp_idf_svc::partition::EspPartition;
#[cfg(feature = "async_matrix")]
use {embassy_futures::select::select_slice, embedded_hal_async::digital::Wait, heapless::Vec};

/// Run RMK keyboard service. This function should never return.
///
/// # Arguments
///
/// * `direct_pins` - direct gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `usb_driver` - (optional) embassy usb driver instance. Some microcontrollers would enable the `_no_usb` feature implicitly, which eliminates this argument
/// * `flash` - (optional) flash storage, which is used for storing keymap and keyboard configs. Some microcontrollers would enable the `_no_external_storage` feature implicitly, which eliminates this argument
/// * `default_keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `low_active`: pin active level
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
pub async fn run_rmk_direct_pin<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(not(feature = "_no_external_storage"))] F: NorFlash,
    const ROW: usize,
    const COL: usize,
    // `let mut futs: Vec<_, {ROW * COL}>` is invalid because of
    // generic parameters may not be used in const operations.
    // Maybe we can use nightly only feature `generic_const_exprs`
    const SIZE: usize,
    const NUM_LAYER: usize,
>(
    direct_pins: [[Option<In>; COL]; ROW],
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(not(feature = "_no_external_storage"))] flash: F,
    default_keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: KeyboardConfig<'static, Out>,
    low_active: bool,
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    // Wrap `embedded-storage` to `embedded-storage-async`
    #[cfg(not(feature = "_no_external_storage"))]
    let async_flash = embassy_embedded_hal::adapter::BlockingAsync::new(flash);

    #[cfg(all(feature = "_no_usb", feature = "_no_external_storage"))]
    {
        run_rmk_direct_pin_with_async_flash::<_, _, ROW, COL, SIZE, NUM_LAYER>(
            direct_pins,
            default_keymap,
            keyboard_config,
            low_active,
            #[cfg(not(feature = "_esp_ble"))]
            spawner,
        )
        .await
    }

    #[cfg(all(not(feature = "_no_usb"), feature = "_no_external_storage"))]
    {
        run_rmk_direct_pin_with_async_flash::<_, _, _, ROW, COL, SIZE, NUM_LAYER>(
            direct_pins,
            usb_driver,
            default_keymap,
            keyboard_config,
            low_active,
            #[cfg(not(feature = "_esp_ble"))]
            spawner,
        )
        .await
    }

    #[cfg(all(feature = "_no_usb", not(feature = "_no_external_storage")))]
    {
        run_rmk_direct_pin_with_async_flash::<_, _, _, ROW, COL, SIZE, NUM_LAYER>(
            direct_pins,
            async_flash,
            default_keymap,
            keyboard_config,
            low_active,
            #[cfg(not(feature = "_esp_ble"))]
            spawner,
        )
        .await
    }

    #[cfg(all(not(feature = "_no_usb"), not(feature = "_no_external_storage")))]
    {
        run_rmk_direct_pin_with_async_flash::<_, _, _, _, ROW, COL, SIZE, NUM_LAYER>(
            direct_pins,
            usb_driver,
            async_flash,
            default_keymap,
            keyboard_config,
            low_active,
            #[cfg(not(feature = "_esp_ble"))]
            spawner,
        )
        .await
    }
}

/// Run RMK keyboard service. This function should never return.
///
/// # Arguments
///
/// * `direct_pins` - direct gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `usb_driver` - (optional) embassy usb driver instance. Some microcontrollers would enable the `_no_usb` feature implicitly, which eliminates this argument
/// * `flash` - (optional) async flash storage, which is used for storing keymap and keyboard configs. Some microcontrollers would enable the `_no_external_storage` feature implicitly, which eliminates this argument
/// * `default_keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `low_active`: pin active level
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
#[allow(unused_variables)]
#[allow(unreachable_code)]
pub async fn run_rmk_direct_pin_with_async_flash<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(not(feature = "_no_external_storage"))] F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const SIZE: usize,
    const NUM_LAYER: usize,
>(
    direct_pins: [[Option<In>; COL]; ROW],
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(not(feature = "_no_external_storage"))] flash: F,
    default_keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: KeyboardConfig<'static, Out>,
    low_active: bool,
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    let rmk_config = keyboard_config.rmk_config;
    #[cfg(feature = "_nrf_ble")]
    let (sd, flash) =
        initialize_nrf_sd_and_flash(rmk_config.usb_config.product_name, spawner, None);

    #[cfg(feature = "_esp_ble")]
    let flash = {
        let f = unsafe {
            EspPartition::new("rmk")
                .expect("Create storage partition error")
                .expect("Empty partition")
        };
        let async_flash = embassy_embedded_hal::adapter::BlockingAsync::new(f);
        async_flash
    };

    let mut storage = Storage::new(flash, default_keymap, rmk_config.storage_config).await;
    let keymap = RefCell::new(KeyMap::new_from_storage(default_keymap, Some(&mut storage)).await);
    let keyboard = Keyboard::new(&keymap, rmk_config.behavior_config);
    let light_controller = LightController::new(keyboard_config.controller_config.light_config);

    // Create the debouncer
    #[cfg(feature = "rapid_debouncer")]
    let debouncer = RapidDebouncer::<COL, ROW>::new();
    #[cfg(not(feature = "rapid_debouncer"))]
    let debouncer = DefaultDebouncer::<COL, ROW>::new();

    // Keyboard matrix
    let matrix = DirectPinMatrix::<_, _, ROW, COL, SIZE>::new(direct_pins, debouncer, low_active);

    run_rmk_internal(
        matrix,   // matrix input device
        keyboard, // key processor
        &keymap,
        #[cfg(not(feature = "_no_usb"))]
        usb_driver,
        storage,
        light_controller,
        rmk_config,
        #[cfg(feature = "_nrf_ble")]
        sd,
    )
    .await
}

/// DirectPinMartex only has input pins.
pub(crate) struct DirectPinMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    D: DebouncerTrait,
    const ROW: usize,
    const COL: usize,
    const SIZE: usize,
> {
    /// Input pins of the pcb matrix
    direct_pins: [[Option<In>; COL]; ROW],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; COL]; ROW],
    /// Start scanning
    scan_start: Option<Instant>,
    /// Pin active level
    low_active: bool,
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > DirectPinMatrix<In, D, ROW, COL, SIZE>
{
    /// Create a matrix from input and output pins.
    pub(crate) fn new(
        direct_pins: [[Option<In>; COL]; ROW],
        debouncer: D,
        low_active: bool,
    ) -> Self {
        DirectPinMatrix {
            direct_pins,
            debouncer,
            key_states: [[KeyState::new(); COL]; ROW],
            scan_start: None,
            low_active,
        }
    }
}

impl<
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > MatrixTrait for DirectPinMatrix<In, D, ROW, COL, SIZE>
{
    const ROW: usize = ROW;
    const COL: usize = COL;

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        if let Some(start_time) = self.scan_start {
            // If no key press over 1ms, stop scanning and wait for interupt
            if start_time.elapsed().as_millis() <= 1 {
                return;
            } else {
                self.scan_start = None;
            }
        }
        Timer::after_micros(1).await;
        info!("Waiting for active level");

        if self.low_active {
            let mut futs: Vec<_, SIZE> = Vec::new();
            for direct_pins_row in self.direct_pins.iter_mut() {
                for direct_pin in direct_pins_row.iter_mut() {
                    if let Some(direct_pin) = direct_pin {
                        let _ = futs.push(direct_pin.wait_for_low());
                    }
                }
            }
            let _ = select_slice(futs.as_mut_slice()).await;
        } else {
            let mut futs: Vec<_, SIZE> = Vec::new();
            for direct_pins_row in self.direct_pins.iter_mut() {
                for direct_pin in direct_pins_row.iter_mut() {
                    if let Some(direct_pin) = direct_pin {
                        let _ = futs.push(direct_pin.wait_for_high());
                    }
                }
            }
            let _ = select_slice(futs.as_mut_slice()).await;
        }
        self.scan_start = Some(Instant::now());
    }

    /// Do matrix scanning, the result is stored in matrix's key_state field.
    async fn scan(&mut self) {
        info!("Matrix scanning");
        loop {
            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;

            // Scan matrix and send report
            for (row_idx, pins_row) in self.direct_pins.iter_mut().enumerate() {
                for (col_idx, direct_pin) in pins_row.iter_mut().enumerate() {
                    if let Some(direct_pin) = direct_pin {
                        let pin_state = if self.low_active {
                            direct_pin.is_low().ok().unwrap_or_default()
                        } else {
                            direct_pin.is_high().ok().unwrap_or_default()
                        };

                        let debounce_state = self.debouncer.detect_change_with_debounce(
                            col_idx,
                            row_idx,
                            pin_state,
                            &self.key_states[row_idx][col_idx],
                        );

                        match debounce_state {
                            DebounceState::Debounced => {
                                self.key_states[row_idx][col_idx].toggle_pressed();
                                let key_state = self.key_states[row_idx][col_idx];

                                KEY_EVENT_CHANNEL
                                    .send(KeyEvent {
                                        row: row_idx as u8,
                                        col: col_idx as u8,
                                        pressed: key_state.pressed,
                                    })
                                    .await;
                            }
                            _ => (),
                        }

                        // If there's key still pressed, always refresh the self.scan_start
                        #[cfg(feature = "async_matrix")]
                        if self.key_states[row_idx][col_idx].pressed {
                            self.scan_start = Some(Instant::now());
                        }
                    }
                }
            }

            Timer::after_micros(100).await;
        }
    }

    /// Read key state at position (row, col)
    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        return self.key_states[row][col];
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }
}
