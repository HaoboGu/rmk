# RMK

Keyboard firmware written in Rust. Tested on STM32H7.

## Prerequisites

This crate requires nightly Rust. `openocd` is used for flashing & debugging.

## Usage

Steps for creating your own firmware:

1. Add rmk to your `Cargo.toml`
2. Choose your target. For stm32h7, you can use `rustup target add thumbv7em-none-eabihf`
3. Create `.cargo/config.toml` in your project's root, specify your target. See [`boards/stm32h7/.cargo/config.toml`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/.cargo/config.toml)
4. Create `main.rs`, initialize your MCU in rtic app, create usb polling task and keyboard task. There are three main parts of `main.rs`:
    1. MCU and peripherals initialization 
      
        ``` rust
        mod app {
            #[shared]
            struct Shared {
                usb: (
                    HIDClass<'static, UsbBus<USB1>>,
                    UsbDevice<'static, UsbBus<USB1>>,
                ),
                led: PE3<Output>,
            }

            #[local]
            struct Local {
                keyboard: Keyboard<ErasedPin<Input>, ErasedPin<Output>, 4, 3, 2>,
            }

            #[init]
            fn init(cx: init::Context) -> (Shared, Local) {
                rtt_logger::init();
                let cp = cx.core;
                let dp = cx.device;

                // Initialize the systick interrupt & obtain the token to prove that we did
                let systick_mono_token = rtic_monotonics::create_systick_token!();
                // Default clock rate is 225MHz
                Systick::start(cp.SYST, 225_000_000, systick_mono_token);

                // Power config
                let pwr = dp.PWR.constrain();
                let pwrcfg = pwr.freeze();

                // Clock config
                let rcc = dp.RCC.constrain();
                let mut ccdr = rcc
                    .use_hse(25.MHz())
                    .sys_ck(225.MHz())
                    .hclk(225.MHz())
                    .per_ck(225.MHz())
                    .freeze(pwrcfg, &dp.SYSCFG);
                // Check HSI 48MHZ
                let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
                // Config HSI
                ccdr.peripheral.kernel_usb_clk_mux(Hsi48);

                // GPIO config
                let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
                let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
                let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
                let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

                // USB config
                let usb_dm = gpioa.pa11.into_alternate();
                let usb_dp = gpioa.pa12.into_alternate();
                let usb: USB1 = USB1::new(
                    dp.OTG1_HS_GLOBAL,
                    dp.OTG1_HS_DEVICE,
                    dp.OTG1_HS_PWRCLK,
                    usb_dm,
                    usb_dp,
                    ccdr.peripheral.USB1OTG,
                    &ccdr.clocks,
                );
                let usb_bus = cortex_m::singleton!(
                    : usb_device::class_prelude::UsbBusAllocator<UsbBus<USB1>> =
                        UsbBus::new(usb, unsafe { &mut EP_MEMORY })
                )
                .unwrap();
                let (hid, usb_dev) = create_usb_device_and_hid_class(
                    usb_bus, 0x16c0, 0x27dd, "haobogu", "fancer", "00000001",
                );

                // Led config
                let mut led = gpioe.pe3.into_push_pull_output();
                led.set_high();

                // Initialize keyboard matrix pins
                let (input_pins, output_pins) = config_matrix_pins!(input: [gpiod.pd9, gpiod.pd8, gpiob.pb13, gpiob.pb12], output: [gpioe.pe13,gpioe.pe14,gpioe.pe15]);
                // Initialize keyboard
                let keyboard = Keyboard::new(input_pins, output_pins, crate::keymap::KEYMAP);

                // Spawn keyboard task
                scan::spawn().ok();

                // RTIC resources
                (
                    Shared {
                        usb: (hid, usb_dev),
                        led,
                    },
                    Local { keyboard },
                )
            }
        }
        ```

    2. keyboard task
    
        ```rust
            #[task(local = [keyboard], shared = [usb])]
            async fn scan(mut cx: scan::Context) {
                // Keyboard scan task
                info!("Start matrix scanning");
                loop {
                    cx.local.keyboard.keyboard_task().await.unwrap();
                    cx.shared.usb.lock(|(hid, _usb_device)| {
                        cx.local.keyboard.send_report(hid);

                    })
                }
            }
        ```

    3. usb polling task
  
        ```rust
            #[task(binds = OTG_HS, shared = [usb])]
            fn usb_poll(mut cx: usb_poll::Context) {
                cx.shared.usb.lock(|(hid, usb_device)| {
                    usb_device.poll(&mut [hid]);
                });
            }
        ```
5. An example can be found at [`boards/stm32h7`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7)

## Compile

```
cargo build
```

### compile and check size
```
cargo size --release
cargo size --profile dev
```

## Flash

Requires `openocd`.

VSCode: Press `F5`, the firmware will be automatically compiled and flashed. A debug session is started after flashing. Check `.vscode/tasks.json` for details.

Or you can do it manually using this command after compile:
```shell
openocd -f openocd.cfg -c "program target/thumbv7em-none-eabihf/debug/rmk-stm32h7 preverify verify reset exit"
``` 

## TODOs

- [x] basic keyboard functions
- [ ] system/media keys
- [ ] layer
- [ ] macro
- [ ] via/vial support
- [ ] encoder
- [ ] RGB
- [ ] cli