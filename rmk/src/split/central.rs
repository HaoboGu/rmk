use core::cell::RefCell;

use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;

use crate::action::KeyAction;
#[cfg(feature = "_nrf_ble")]
use crate::ble::nrf::initialize_nrf_ble_keyboard_and_run;
use crate::config::RmkConfig;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::{DebounceState, DebouncerTrait};
use crate::event::KeyEvent;
use crate::keyboard::{Keyboard, KEYBOARD_REPORT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::keymap::KeyMap;
use crate::light::LightService;
use crate::matrix::{KeyState, MatrixTrait};
use crate::run_usb_keyboard;
use crate::usb::KeyboardUsbDevice;
use crate::via::process::VialService;

#[cfg(not(feature = "_nrf_ble"))]
use embedded_io_async::{Read, Write};
#[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
use {crate::storage::Storage, embedded_storage_async::nor_flash::NorFlash};

/// Run RMK split central keyboard service. This function should never return.
///
/// # Arguments
///
/// * `input_pins` - input gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `output_pins` - output gpio pins
/// * `usb_driver` - (optional) embassy usb driver instance. Some microcontrollers would enable the `_no_usb` feature implicitly, which eliminates this argument
/// * `flash` - (optional) flash storage, which is used for storing keymap and keyboard configs. Some microcontrollers would enable the `_no_external_storage` feature implicitly, which eliminates this argument
/// * `default_keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `central_addr` - (optional) central's BLE static address. This argument is enabled only for nRF BLE split central now
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
#[allow(unused_variables)]
#[allow(unreachable_code)]
pub async fn run_rmk_split_central<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(not(feature = "_no_external_storage"))] F: NorFlash,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const CENTRAL_ROW: usize,
    const CENTRAL_COL: usize,
    const CENTRAL_ROW_OFFSET: usize,
    const CENTRAL_COL_OFFSET: usize,
    const NUM_LAYER: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; CENTRAL_ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; CENTRAL_COL],
    #[cfg(feature = "col2row")] output_pins: [Out; CENTRAL_COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; CENTRAL_ROW],
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(not(feature = "_no_external_storage"))] flash: F,
    default_keymap: &mut [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],

    keyboard_config: RmkConfig<'static, Out>,
    #[cfg(feature = "_nrf_ble")] central_addr: [u8; 6],
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    // Create the debouncer, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<CENTRAL_ROW, CENTRAL_COL> = RapidDebouncer::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<CENTRAL_COL, CENTRAL_ROW> = RapidDebouncer::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<CENTRAL_ROW, CENTRAL_COL> = DefaultDebouncer::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<CENTRAL_COL, CENTRAL_ROW> = DefaultDebouncer::new();

    // Keyboard matrix, use COL2ROW by default
    #[cfg(feature = "col2row")]
    let matrix = CentralMatrix::<
        In,
        Out,
        _,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_ROW,
        CENTRAL_COL,
    >::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = CentralMatrix::<
        In,
        Out,
        _,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_COL,
        CENTRAL_ROW,
    >::new(input_pins, output_pins, debouncer);

    #[cfg(feature = "_nrf_ble")]
    let fut = initialize_nrf_ble_keyboard_and_run::<_, _, D, TOTAL_ROW, TOTAL_COL, NUM_LAYER>(
        matrix,
        usb_driver,
        default_keymap,
        keyboard_config,
        Some(central_addr),
        spawner,
    )
    .await;

    #[cfg(not(any(feature = "_nrf_ble", feature = "_esp_ble")))]
    let fut = initialize_usb_split_central_and_run::<_, _, D, F, TOTAL_ROW, TOTAL_COL, NUM_LAYER>(
        matrix,
        usb_driver,
        flash,
        default_keymap,
        keyboard_config,
    )
    .await;

    fut
}

/// Run RMK split central keyboard service. This function should never return.
///
/// # Arguments
///
/// * `direct_pins` - direct gpio pins, if `async_matrix` is enabled, the input pins should implement `embedded_hal_async::digital::Wait` trait
/// * `usb_driver` - (optional) embassy usb driver instance. Some microcontrollers would enable the `_no_usb` feature implicitly, which eliminates this argument
/// * `flash` - (optional) flash storage, which is used for storing keymap and keyboard configs. Some microcontrollers would enable the `_no_external_storage` feature implicitly, which eliminates this argument
/// * `default_keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
/// * `low_active`: pin active level
/// * `central_addr` - (optional) central's BLE static address. This argument is enabled only for nRF BLE split central now
/// * `spawner`: (optional) embassy spawner used to spawn async tasks. This argument is enabled for non-esp microcontrollers
#[allow(unused_variables)]
#[allow(unreachable_code)]
pub async fn run_rmk_split_central_direct_pin<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    #[cfg(not(feature = "_no_usb"))] D: Driver<'static>,
    #[cfg(not(feature = "_no_external_storage"))] F: NorFlash,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const CENTRAL_ROW: usize,
    const CENTRAL_COL: usize,
    const CENTRAL_ROW_OFFSET: usize,
    const CENTRAL_COL_OFFSET: usize,
    const NUM_LAYER: usize,
    const SIZE: usize,
>(
    direct_pins: [[Option<In>; CENTRAL_COL]; CENTRAL_ROW],
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(not(feature = "_no_external_storage"))] flash: F,
    default_keymap: &mut [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],

    keyboard_config: RmkConfig<'static, Out>,
    low_active: bool,
    #[cfg(feature = "_nrf_ble")] central_addr: [u8; 6],
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    info!("Debouncer");
    // Create the debouncer, use COL2ROW by default
    #[cfg(feature = "rapid_debouncer")]
    let debouncer: RapidDebouncer<CENTRAL_COL, CENTRAL_ROW> = RapidDebouncer::new();
    #[cfg(not(feature = "rapid_debouncer"))]
    let debouncer: DefaultDebouncer<CENTRAL_COL, CENTRAL_ROW> = DefaultDebouncer::new();

    // Keyboard matrix, use COL2ROW by default
    let matrix = CentralDirectPinMatrix::<
        _,
        _,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_ROW,
        CENTRAL_COL,
        SIZE,
    >::new(direct_pins, debouncer, low_active);

    #[cfg(feature = "_nrf_ble")]
    let fut = initialize_nrf_ble_keyboard_and_run::<_, _, D, TOTAL_ROW, TOTAL_COL, NUM_LAYER>(
        matrix,
        usb_driver,
        default_keymap,
        keyboard_config,
        Some(central_addr),
        spawner,
    )
    .await;

    #[cfg(not(any(feature = "_nrf_ble", feature = "_esp_ble")))]
    let fut = initialize_usb_split_central_and_run::<_, _, D, F, TOTAL_ROW, TOTAL_COL, NUM_LAYER>(
        matrix,
        usb_driver,
        flash,
        default_keymap,
        keyboard_config,
    )
    .await;

    fut
}

/// Run central's peripheral monitor task.
///
/// # Arguments
/// * `id` - peripheral id
/// * `addr` - (optional) peripheral's BLE static address. This argument is enabled only for nRF BLE split now
/// * `receiver` - (optional) serial port. This argument is enabled only for serial split now
pub async fn run_peripheral_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    #[cfg(not(feature = "_nrf_ble"))] S: Read + Write,
>(
    id: usize,
    #[cfg(feature = "_nrf_ble")] addr: [u8; 6],
    #[cfg(not(feature = "_nrf_ble"))] receiver: S,
) {
    #[cfg(feature = "_nrf_ble")]
    {
        use crate::split::nrf::central::run_ble_peripheral_monitor;
        run_ble_peripheral_monitor::<ROW, COL, ROW_OFFSET, COL_OFFSET>(id, addr).await;
    };

    #[cfg(not(feature = "_nrf_ble"))]
    {
        use crate::split::serial::run_serial_peripheral_monitor;
        run_serial_peripheral_monitor::<ROW, COL, ROW_OFFSET, COL_OFFSET, S>(id, receiver).await;
    };
}

/// Split central is connected to host via usb
pub async fn initialize_usb_split_central_and_run<
    M: MatrixTrait,
    Out: OutputPin,
    D: Driver<'static>,
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))] F: NorFlash,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const NUM_LAYER: usize,
>(
    mut matrix: M,
    #[cfg(not(feature = "_no_usb"))] usb_driver: D,
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))] flash: F,
    default_keymap: &mut [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],

    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    // For USB keyboard, the "external" storage means the storage initialized by the user.
    #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
    let (mut storage, keymap) = {
        let mut s = Storage::new(flash, default_keymap, keyboard_config.storage_config).await;
        let keymap = RefCell::new(
            KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage(
                default_keymap,
                Some(&mut s),
            )
            .await,
        );
        (s, keymap)
    };

    #[cfg(all(not(feature = "_nrf_ble"), feature = "_no_external_storage"))]
    let keymap = RefCell::new(KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new(default_keymap).await);

    let keyboard_report_sender = KEYBOARD_REPORT_CHANNEL.sender();
    let keyboard_report_receiver = KEYBOARD_REPORT_CHANNEL.receiver();

    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(
            &keymap,
            &keyboard_report_sender,
            keyboard_config.behavior_config,
        ),
        KeyboardUsbDevice::new(usb_driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    // Run usb keyboard
    run_usb_keyboard(
        &mut usb_device,
        &mut keyboard,
        &mut matrix,
        #[cfg(any(feature = "_nrf_ble", not(feature = "_no_external_storage")))]
        &mut storage,
        &mut light_service,
        &mut vial_service,
        &keyboard_report_receiver,
    )
    .await
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub(crate) struct CentralMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    const INPUT_PIN_NUM: usize,
    const OUTPUT_PIN_NUM: usize,
> {
    /// Input pins of the pcb matrix
    input_pins: [In; INPUT_PIN_NUM],
    /// Output pins of the pcb matrix
    output_pins: [Out; OUTPUT_PIN_NUM],
    /// Debouncer
    debouncer: D,
    /// Key state matrix
    key_states: [[KeyState; INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
    /// Start scanning
    scan_start: Option<Instant>,
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MatrixTrait
    for CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    #[cfg(feature = "col2row")]
    const ROW: usize = INPUT_PIN_NUM;
    #[cfg(feature = "col2row")]
    const COL: usize = OUTPUT_PIN_NUM;
    #[cfg(not(feature = "col2row"))]
    const ROW: usize = OUTPUT_PIN_NUM;
    #[cfg(not(feature = "col2row"))]
    const COL: usize = INPUT_PIN_NUM;

    async fn scan(&mut self) {
        info!("Central matrix scanning");
        loop {
            #[cfg(feature = "async_matrix")]
            self.wait_for_key().await;

            // Scan matrix and send report
            for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
                // Pull up output pin, wait 1us ensuring the change comes into effect
                out_pin.set_high().ok();
                Timer::after_micros(1).await;
                for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
                    // Check input pins and debounce
                    let debounce_state = self.debouncer.detect_change_with_debounce(
                        in_idx,
                        out_idx,
                        in_pin.is_high().ok().unwrap_or_default(),
                        &self.key_states[out_idx][in_idx],
                    );

                    match debounce_state {
                        DebounceState::Debounced => {
                            self.key_states[out_idx][in_idx].toggle_pressed();
                            #[cfg(feature = "col2row")]
                            let (row, col, key_state) = (
                                (in_idx + ROW_OFFSET) as u8,
                                (out_idx + COL_OFFSET) as u8,
                                self.key_states[out_idx][in_idx],
                            );
                            #[cfg(not(feature = "col2row"))]
                            let (row, col, key_state) = (
                                (out_idx + ROW_OFFSET) as u8,
                                (in_idx + COL_OFFSET) as u8,
                                self.key_states[out_idx][in_idx],
                            );

                            KEY_EVENT_CHANNEL
                                .send(KeyEvent {
                                    row,
                                    col,
                                    pressed: key_state.pressed,
                                })
                                .await;
                        }
                        _ => (),
                    }

                    // If there's key still pressed, always refresh the self.scan_start
                    #[cfg(feature = "async_matrix")]
                    if self.key_states[out_idx][in_idx].pressed {
                        self.scan_start = Some(Instant::now());
                    }
                }
                out_pin.set_low().ok();
            }

            embassy_time::Timer::after_micros(100).await;
        }
    }

    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        self.key_states[row][col]
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        use embassy_futures::select::select_slice;
        use heapless::Vec;

        if let Some(start_time) = self.scan_start {
            // If not key over 2 secs, wait for interupt in next loop
            if start_time.elapsed().as_secs() < 1 {
                return;
            } else {
                self.scan_start = None;
            }
        }
        // First, set all output pin to high
        for out in self.output_pins.iter_mut() {
            out.set_high().ok();
        }
        Timer::after_micros(1).await;
        info!("Waiting for high");
        let mut futs: Vec<_, INPUT_PIN_NUM> = self
            .input_pins
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_high())
            .collect();
        let _ = select_slice(futs.as_mut_slice()).await;

        // Set all output pins back to low
        for out in self.output_pins.iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());
    }
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > CentralMatrix<In, Out, D, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Initialization of central
    pub(crate) fn new(
        input_pins: [In; INPUT_PIN_NUM],
        output_pins: [Out; OUTPUT_PIN_NUM],
        debouncer: D,
    ) -> Self {
        CentralMatrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::default(); INPUT_PIN_NUM]; OUTPUT_PIN_NUM],
            scan_start: None,
        }
    }
}

/// DirectPinMartex only has input pins.
pub(crate) struct CentralDirectPinMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    D: DebouncerTrait,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
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
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > CentralDirectPinMatrix<In, D, ROW_OFFSET, COL_OFFSET, ROW, COL, SIZE>
{
    /// Create a matrix from input and output pins.
    pub(crate) fn new(
        direct_pins: [[Option<In>; COL]; ROW],
        debouncer: D,
        low_active: bool,
    ) -> Self {
        CentralDirectPinMatrix {
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
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const ROW: usize,
        const COL: usize,
        const SIZE: usize,
    > MatrixTrait for CentralDirectPinMatrix<In, D, ROW_OFFSET, COL_OFFSET, ROW, COL, SIZE>
{
    const ROW: usize = ROW;
    const COL: usize = COL;

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        use embassy_futures::select::select_slice;
        use heapless::Vec;
        if let Some(start_time) = self.scan_start {
            // If no key press over 1ms, stop scanning and wait for interrupt
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
        info!("Central Direct Pin Matrix scanning");
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
                                let (col, row, key_state) = (
                                    (col_idx + COL_OFFSET) as u8,
                                    (row_idx + ROW_OFFSET) as u8,
                                    self.key_states[row_idx][col_idx],
                                );

                                KEY_EVENT_CHANNEL
                                    .send(KeyEvent {
                                        row,
                                        col,
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
        self.key_states[row][col]
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }
}
