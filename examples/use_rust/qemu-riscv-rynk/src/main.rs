#![no_main]
#![no_std]

use core::convert::Infallible;
use core::ptr::NonNull;
use embassy_executor::Spawner;
use embassy_futures::yield_now;
use embedded_io_async::{Read, Write};
use panic_halt as _;
use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig};
use rmk::host::run_rynk_uart;
use rmk::keymap::KeymapData;
use rmk::types::action::KeyAction;
use rmk::{initialize_keymap, k, layer};
use semihosting::println;
use static_cell::StaticCell;
use uart_16550::backend::MmioBackend;
use uart_16550::Uart16550;

struct Uart(Uart16550<MmioBackend>);

impl Uart {
    fn new() -> Self {
        let addr = NonNull::new(0x1000_0000usize as *mut u8).unwrap();
        let mut uart = unsafe { Uart16550::new_mmio(addr, 1) }.unwrap();
        uart.init(uart_16550::Config::default()).unwrap();
        Self(uart)
    }
}

impl embedded_io_async::ErrorType for Uart {
    type Error = Infallible;
}

impl Read for Uart {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            let n = self.0.receive_bytes(buf);
            if n > 0 { return Ok(n) }
            else { yield_now().await }
        }
    }
}

impl Write for Uart {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut sent = 0;
        while sent < buf.len() {
            let n = self.0.send_bytes(&buf[sent..]);
            if n > 0 { sent += n }
            else { yield_now().await }
        }
        Ok(sent)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

const COL: usize = 3;
const ROW: usize = 3;
const NUM_LAYER: usize = 2;

#[rustfmt::skip]
const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    [
        layer!([
            [k!(Kp1), k!(Kp2), k!(Kp3)],
            [k!(Kp4), k!(Kp5), k!(Kp6)],
            [k!(Kp7), k!(Kp8), k!(Kp9)]
        ]),
        layer!([
            [k!(A), k!(B), k!(C)],
            [k!(D), k!(E), k!(F)],
            [k!(G), k!(H), k!(I)]
        ]),
    ]
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    println!("[RMK] starting");

    let rx = Uart::new();
    let tx = Uart::new();

    let mut keymap_data = KeymapData::new(get_default_keymap());
    let mut behavior_config = BehaviorConfig::default();
    let positional_config = PositionalConfig::default();
    let keymap = initialize_keymap(&mut keymap_data, &mut behavior_config, &positional_config).await;

    static RMK_CONFIG: StaticCell<RmkConfig<'static>> = StaticCell::new();
    let rmk_config = RMK_CONFIG.init(RmkConfig::default());

    let service = rmk::host::HostService::new(&keymap, rmk_config);
    run_rynk_uart(rx, tx, &service).await;
}
