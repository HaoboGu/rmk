use crate::action::KeyAction;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::{DebounceState, DebouncerTrait};
use crate::keyboard::{communication_task, Keyboard, KeyboardReportMessage};
use crate::keymap::KeyMap;
use crate::matrix::{KeyState, MatrixTrait};
use crate::run_usb_keyboard;
use crate::storage::Storage;
use crate::usb::KeyboardUsbDevice;
use crate::via::process::VialService;
use crate::{
    keyboard::keyboard_task,
    light::{led_hid_task, LightService},
    via::vial_task,
};

use core::cell::RefCell;
use defmt::*;
use embassy_futures::select::{select, select4, Either4};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Instant, Timer};
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_io_async::Read;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use futures::pin_mut;
use postcard::experimental::max_size::MaxSize;
use rmk_config::RmkConfig;
use serde::{Deserialize, Serialize};

use super::driver::serial::SerialSplitMasterReceiver;

/// Channels for synchronization between master and slave threads
const SYNC_CHANNEL_VALUE: Channel<CriticalSectionRawMutex, KeySyncMessage, 8> = Channel::new();
pub(crate) static MASTER_SYNC_CHANNELS: [Channel<CriticalSectionRawMutex, KeySyncMessage, 8>; 4] =
    [SYNC_CHANNEL_VALUE; 4];

/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional **async** flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub async fn initialize_split_master_and_run<
    F: AsyncNorFlash,
    D: Driver<'static>,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const MASTER_ROW: usize,
    const MASTER_COL: usize,
    const MASTER_ROW_OFFSET: usize,
    const MASTER_COL_OFFSET: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    #[cfg(feature = "col2row")] input_pins: [In; MASTER_ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; MASTER_COL],
    #[cfg(feature = "col2row")] output_pins: [Out; MASTER_COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; MASTER_ROW],
    flash: Option<F>,
    default_keymap: [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    let (mut storage, keymap) = match flash {
        Some(f) => {
            let mut s = Storage::new(f, &default_keymap, keyboard_config.storage_config).await;
            let keymap = RefCell::new(
                KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage(
                    default_keymap,
                    Some(&mut s),
                )
                .await,
            );
            (Some(s), keymap)
        }
        None => {
            let keymap = RefCell::new(
                KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage::<F>(
                    default_keymap,
                    None,
                )
                .await,
            );
            (None, keymap)
        }
    };

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let mut keyboard_report_sender = keyboard_channel.sender();
    let mut keyboard_report_receiver = keyboard_channel.receiver();

    // Create keyboard services and devices

    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let matrix = MasterMatrix::<
        In,
        Out,
        RapidDebouncer<MASTER_ROW, MASTER_COL>,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_ROW,
        MASTER_COL,
    >::new(input_pins, output_pins, RapidDebouncer::new());
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let matrix = MasterMatrix::<
        In,
        Out,
        DefaultDebouncer<MASTER_ROW, MASTER_COL>,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_ROW,
        MASTER_COL,
    >::new(input_pins, output_pins, DefaultDebouncer::new());
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let matrix = MasterMatrix::<
        In,
        Out,
        RapidDebouncer<MASTER_COL, MASTER_ROW>,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_COL,
        MASTER_ROW,
    >::new(input_pins, output_pins, RapidDebouncer::new());
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let matrix = MasterMatrix::<
        In,
        Out,
        DefaultDebouncer<MASTER_COL, MASTER_ROW>,
        TOTAL_ROW,
        TOTAL_COL,
        MASTER_ROW_OFFSET,
        MASTER_COL_OFFSET,
        MASTER_COL,
        MASTER_ROW,
    >::new(input_pins, output_pins, DefaultDebouncer::new());

    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(matrix, &keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    loop {
        // Run all tasks, if one of them fails, wait 1 second and then restart
        if let Some(ref mut s) = storage {
            run_usb_keyboard(
                &mut usb_device,
                &mut keyboard,
                s,
                &mut light_service,
                &mut vial_service,
                &mut keyboard_report_receiver,
                &mut keyboard_report_sender,
            )
            .await;
        } else {
            // Run 5 tasks: usb, keyboard, led, vial, communication
            let usb_fut = usb_device.device.run();
            let keyboard_fut = keyboard_task(&mut keyboard, &mut keyboard_report_sender);
            let communication_fut = communication_task(
                &mut keyboard_report_receiver,
                &mut usb_device.keyboard_hid_writer,
                &mut usb_device.other_hid_writer,
            );
            let led_fut = led_hid_task(&mut usb_device.keyboard_hid_reader, &mut light_service);
            let via_fut = vial_task(&mut usb_device.via_hid, &mut vial_service);
            // let slave_fut = select_slice(&mut slave_futs);
            pin_mut!(usb_fut);
            pin_mut!(keyboard_fut);
            pin_mut!(led_fut);
            pin_mut!(via_fut);
            pin_mut!(communication_fut);
            match select4(
                usb_fut,
                select(keyboard_fut, communication_fut),
                // select(led_fut, slave_fut),
                led_fut,
                via_fut,
            )
            .await
            {
                Either4::First(_) => {
                    error!("Usb task is died");
                }
                Either4::Second(_) => error!("Keyboard task is died"),
                Either4::Third(_) => error!("Led task is died"),
                Either4::Fourth(_) => error!("Via task is died"),
            }
        }

        warn!("Detected failure, restarting keyboard sevice after 1 second");
        Timer::after_secs(1).await;
    }
}

/// Receive split message from slave and process it
///
/// Generic parameters:
/// - `const ROW`: row number of the slave's matrix
/// - `const COL`: column number of the slave's matrix
/// - `const ROW_OFFSET`: row offset of the slave's matrix in the whole matrix
/// - `const COL_OFFSET`: column offset of the slave's matrix in the whole matrix
/// - `R`: the type of the receiver
pub async fn run_slave_receiver<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    R: Read,
>(
    receiver: R,
    id: usize,
) {
    let slave = SerialSplitMasterReceiver::<ROW, COL, ROW_OFFSET, COL_OFFSET, R>::new(receiver, id);
    slave.run().await;
}

/// Message used from master & slave communication
#[derive(Serialize, Deserialize, Debug, Clone, Copy, MaxSize)]
#[repr(u8)]
pub enum SplitMessage {
    /// Activated key info (row, col, pressed), from slave to master
    Key(u8, u8, bool),
    /// Led state, on/off
    LedState(bool),
}

/// Message used for synchronization between master thread and slave receiver(both in master board)
pub(crate) enum KeySyncMessage {
    /// Sent from master to slave thread, indicating master starts to read the key state matrix
    StartRead,
    /// Response of `StartRead`, sent from slave to master, indicating that the slave starts to send the key state matrix.
    /// u8 is the number of sent key states
    StartSend(u16),
    /// Key state: (row, col, key_pressing_state)
    Key(u8, u8, bool),
}

/// Matrix is the physical pcb layout of the keyboard matrix.
pub(crate) struct MasterMatrix<
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
    for MasterMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    async fn scan(&mut self) {
        self.internal_scan().await;
        self.scan_slave().await;
    }

    fn get_key_state(&mut self, row: usize, col: usize) -> KeyState {
        self.key_states[row][col]
    }

    fn update_key_state(&mut self, row: usize, col: usize, f: impl FnOnce(&mut KeyState)) {
        f(&mut self.key_states[row][col]);
    }

    #[cfg(feature = "async_matrix")]
    async fn wait_for_key(&mut self) {
        todo!()
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
    > MasterMatrix<In, Out, D, ROW, COL, ROW_OFFSET, COL_OFFSET, INPUT_PIN_NUM, OUTPUT_PIN_NUM>
{
    /// Initialization of master
    pub(crate) fn new(
        input_pins: [In; INPUT_PIN_NUM],
        output_pins: [Out; OUTPUT_PIN_NUM],
        debouncer: D,
    ) -> Self {
        MasterMatrix {
            input_pins,
            output_pins,
            debouncer,
            key_states: [[KeyState::default(); COL]; ROW],
            scan_start: None,
        }
    }

    pub(crate) async fn scan_slave(&mut self) {
        for slave_channel in MASTER_SYNC_CHANNELS.iter() {
            // TODO: Continue when the slave is not connected
            slave_channel.send(KeySyncMessage::StartRead).await;
            if let KeySyncMessage::StartSend(n) = slave_channel.receive().await {
                for _ in 0..n {
                    if let KeySyncMessage::Key(row, col, key_state) = slave_channel.receive().await
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
