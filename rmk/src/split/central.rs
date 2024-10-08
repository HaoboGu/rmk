use core::cell::RefCell;

use defmt::warn;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Instant, Timer};
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use rmk_config::RmkConfig;

use crate::action::KeyAction;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::{DebounceState, DebouncerTrait};
use crate::keyboard::{Keyboard, KeyboardReportMessage};
use crate::keymap::KeyMap;
use crate::light::LightService;
use crate::matrix::{KeyState, MatrixTrait};
use crate::run_usb_keyboard;
#[cfg(feature = "_nrf_ble")]
use crate::split::nrf::central::initialize_ble_split_central_and_run;
use crate::split::KeySyncSignal;
use crate::split::SYNC_SIGNALS;
use crate::usb::KeyboardUsbDevice;
use crate::via::process::VialService;

#[cfg(not(feature = "_nrf_ble"))]
use {
    crate::storage::Storage,
    embedded_io_async::{Read, Write},
    embedded_storage_async::nor_flash::NorFlash,
};

use super::{KeySyncMessage, CENTRAL_SYNC_CHANNELS};

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
    default_keymap: [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
    #[cfg(feature = "_nrf_ble")] central_addr: [u8; 6],
    #[cfg(not(feature = "_esp_ble"))] spawner: Spawner,
) -> ! {
    #[cfg(feature = "_nrf_ble")]
    let fut = initialize_ble_split_central_and_run::<
        In,
        Out,
        D,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW,
        CENTRAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        NUM_LAYER,
    >(
        input_pins,
        output_pins,
        usb_driver,
        default_keymap,
        keyboard_config,
        central_addr,
        spawner,
    )
    .await;

    #[cfg(not(any(feature = "_nrf_ble", feature = "_esp_ble")))]
    let fut = initialize_usb_split_central_and_run::<
        In,
        Out,
        D,
        F,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW,
        CENTRAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        NUM_LAYER,
    >(
        input_pins,
        output_pins,
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
pub(crate) async fn initialize_usb_split_central_and_run<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: Driver<'static>,
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
    default_keymap: [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    // For USB keyboard, the "external" storage means the storage initialized by the user.
    #[cfg(not(feature = "_no_external_storage"))]
    let (mut storage, keymap) = {
        let mut s = Storage::new(flash, &default_keymap, keyboard_config.storage_config).await;
        let keymap = RefCell::new(
            KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage(
                default_keymap,
                Some(&mut s),
            )
            .await,
        );
        (s, keymap)
    };
    #[cfg(feature = "_no_external_storage")]
    let keymap = RefCell::new(KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new(default_keymap).await);

    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let mut matrix = CentralMatrix::<
        In,
        Out,
        RapidDebouncer<CENTRAL_ROW, CENTRAL_COL>,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_ROW,
        CENTRAL_COL,
    >::new(input_pins, output_pins, RapidDebouncer::new());
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let mut matrix = CentralMatrix::<
        In,
        Out,
        DefaultDebouncer<CENTRAL_ROW, CENTRAL_COL>,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_ROW,
        CENTRAL_COL,
    >::new(input_pins, output_pins, DefaultDebouncer::new());
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let mut matrix = CentralMatrix::<
        In,
        Out,
        RapidDebouncer<CENTRAL_COL, CENTRAL_ROW>,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_COL,
        CENTRAL_ROW,
    >::new(input_pins, output_pins, RapidDebouncer::new());
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let mut matrix = CentralMatrix::<
        In,
        Out,
        DefaultDebouncer<CENTRAL_COL, CENTRAL_ROW>,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_COL,
        CENTRAL_ROW,
    >::new(input_pins, output_pins, DefaultDebouncer::new());

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let keyboard_report_sender = keyboard_channel.sender();
    let keyboard_report_receiver = keyboard_channel.receiver();

    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(&keymap, &keyboard_report_sender),
        KeyboardUsbDevice::new(usb_driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    loop {
        // Run all tasks, if one of them fails, wait 1 second and then restart
        run_usb_keyboard(
            &mut usb_device,
            &mut keyboard,
            &mut matrix,
            #[cfg(not(feature = "_no_external_storage"))]
            &mut storage,
            &mut light_service,
            &mut vial_service,
            &keyboard_report_receiver,
        )
        .await;

        warn!("Detected failure, restarting keyboard sevice after 1 second");
        Timer::after_secs(1).await;
    }
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub(crate) struct CentralMatrix<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    D: DebouncerTrait,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
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
    key_states: [[KeyState; TOTAL_COL]; TOTAL_ROW],
    /// Start scanning
    scan_start: Option<Instant>,
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > MatrixTrait
    for CentralMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    const ROW: usize = ROW;
    const COL: usize = COL;

    async fn scan(&mut self) {
        self.internal_scan().await;
        self.scan_peripheral().await;
    }

    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        self.key_states[row][col]
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        use super::SCAN_SIGNAL;
        use embassy_futures::select::{select, select_slice};
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

        // Enable SCAN_SIGNAL, wait for peripheral's report
        SCAN_SIGNAL.reset();

        // Current board's matrix
        let mut futs: Vec<_, INPUT_PIN_NUM> = self
            .input_pins
            .iter_mut()
            .map(|input_pin| input_pin.wait_for_high())
            .collect();

        // Wait for split event
        let split_event = SCAN_SIGNAL.wait();

        let _ = select(split_event, select_slice(futs.as_mut_slice())).await;

        // Set all output pins back to low
        for out in self.output_pins.iter_mut() {
            out.set_low().ok();
        }

        self.scan_start = Some(Instant::now());

        // Enable SCAN_SIGNAL, wait for peripheral's report
        SCAN_SIGNAL.reset();
    }
}

impl<
        #[cfg(feature = "async_matrix")] In: Wait + InputPin,
        #[cfg(not(feature = "async_matrix"))] In: InputPin,
        Out: OutputPin,
        D: DebouncerTrait,
        const ROW: usize,
        const COL: usize,
        const ROW_OFFSET: usize,
        const COL_OFFSET: usize,
        const INPUT_PIN_NUM: usize,
        const OUTPUT_PIN_NUM: usize,
    > CentralMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
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
            key_states: [[KeyState::default(); COL]; ROW],
            scan_start: None,
        }
    }

    pub(crate) async fn scan_peripheral(&mut self) {
        for (id, peripheral_channel) in CENTRAL_SYNC_CHANNELS.iter().enumerate() {
            // TODO: Skip unused peripherals
            if id > 0 {
                break;
            }
            // Signal that peripheral scanning is started
            SYNC_SIGNALS[id].signal(KeySyncSignal::Start);
            // Receive peripheral key states
            if let KeySyncMessage::StartSend(n) = peripheral_channel.receive().await {
                // Update peripheral's key states
                for _ in 0..n {
                    if let KeySyncMessage::Key(row, col, key_state) =
                        peripheral_channel.receive().await
                    {
                        if key_state != self.key_states[row as usize][col as usize].pressed {
                            self.key_states[row as usize][col as usize].pressed = key_state;
                            self.key_states[row as usize][col as usize].changed = true;
                        } else {
                            self.key_states[row as usize][col as usize].changed = false;
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn internal_scan(&mut self) {
        // Get the row and col index of current board in the whole key matrix
        for (out_idx, out_pin) in self.output_pins.iter_mut().enumerate() {
            // Pull up output pin, wait 1us ensuring the change comes into effect
            out_pin.set_high().ok();
            Timer::after_micros(1).await;
            for (in_idx, in_pin) in self.input_pins.iter_mut().enumerate() {
                #[cfg(feature = "col2row")]
                let (row_idx, col_idx) = (in_idx + ROW_OFFSET, out_idx + COL_OFFSET);
                #[cfg(not(feature = "col2row"))]
                let (row_idx, col_idx) = (out_idx + ROW_OFFSET, in_idx + COL_OFFSET);

                // Check input pins and debounce
                let debounce_state = self.debouncer.detect_change_with_debounce(
                    in_idx,
                    out_idx,
                    in_pin.is_high().ok().unwrap_or_default(),
                    &self.key_states[row_idx][col_idx],
                );

                match debounce_state {
                    DebounceState::Debounced => {
                        self.key_states[row_idx][col_idx].toggle_pressed();
                        self.key_states[row_idx][col_idx].changed = true;
                    }
                    _ => self.key_states[row_idx][col_idx].changed = false,
                }

                // If there's key changed or pressed, always refresh the self.scan_start
                if self.key_states[row_idx][col_idx].changed
                    || self.key_states[row_idx][col_idx].pressed
                {
                    #[cfg(feature = "async_matrix")]
                    {
                        self.scan_start = Some(Instant::now());
                    }
                }
            }
            out_pin.set_low().ok();
        }
    }

    /// Read key state OF CURRENT BOARD at position (row, col)
    pub(crate) fn get_key_state_current_board(
        &mut self,
        out_idx: usize,
        in_idx: usize,
    ) -> KeyState {
        #[cfg(feature = "col2row")]
        return self.key_states[in_idx + ROW_OFFSET][out_idx + COL_OFFSET];
        #[cfg(not(feature = "col2row"))]
        return self.key_states[out_idx + ROW_OFFSET][in_idx + COL_OFFSET];
    }
}
