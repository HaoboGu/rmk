use core::sync::atomic::{AtomicBool, Ordering};

use critical_section::RestoreState;
use defmt::Encoder;

static mut ENCODER: Encoder = Encoder::new();
static LOGGER_ACQUIRED: AtomicBool = AtomicBool::new(false);
static mut RESTORE_STATE: critical_section::RestoreState = RestoreState::invalid();

static mut WRITER: Option<&'static dyn Fn(&[u8]) -> ()> = None;

pub fn set_logger(write: &'static dyn Fn(&[u8]) -> ()) {
    unsafe { WRITER = Some(write) }
}

fn uart_tx_write(bytes: &[u8]) {
    unsafe { WRITER.map(|write| write(bytes)) };
}

#[defmt::global_logger]
pub struct Logger;

#[allow(static_mut_refs)]
unsafe impl defmt::Logger for Logger {
    fn acquire() {
        let cs_handle = unsafe { critical_section::acquire() };
        let acquired = LOGGER_ACQUIRED.load(Ordering::Acquire);
        if acquired {
            // panic equivalent to avoid nesting
            loop {}
        }
        unsafe {
            RESTORE_STATE = cs_handle;
        }
        LOGGER_ACQUIRED.store(true, Ordering::Release);
        unsafe { ENCODER.start_frame(uart_tx_write) };
    }

    unsafe fn flush() {}

    unsafe fn release() {
        ENCODER.end_frame(uart_tx_write);
        let acquired = LOGGER_ACQUIRED.load(Ordering::Acquire);
        if !acquired {
            // panic equivalent to avoid nesting
            loop {}
        }
        LOGGER_ACQUIRED.store(false, Ordering::Release);
        critical_section::release(RESTORE_STATE);
    }

    unsafe fn write(bytes: &[u8]) {
        ENCODER.write(bytes, uart_tx_write);
    }
}