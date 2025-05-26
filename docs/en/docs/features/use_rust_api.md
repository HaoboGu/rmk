# Use Rust

By default, the generated project uses `keyboard.toml` to config the RMK keyboard firmware. If you want to customize your firmware using Rust, there're steps to do to make the generated firmware project compile:

### Update memory.x

`memory.x` is the linker script of Rust embedded project, it's used to define the memory layout of the microcontroller. RMK enables `memory-x` feature for `embassy-stm32`, so if you're using stm32, you can just ignore this step.

For other ARM Cortex-M microcontrollers, you only need to update the `LENGTH` of FLASH and RAM to your microcontroller.

If you're using **nRF52840**, ensure that you have [Adafruit_nRF52_Bootloader](https://github.com/adafruit/Adafruit_nRF52_Bootloader) flashed to your board. Most nice!nano compatible boards have it already. As long as you can open a USB drive for your board and update uf2 firmware by dragging and dropping, you're all set.

You can either checkout your microcontroller's datasheet or existing Rust project of your microcontroller for it.

### Update `main.rs`

By default, generated `main.rs` uses proc-macro and `keyboard.toml`. To fully customize the firmware, you can copy the code from RMK's Rust example, such as <https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/rp2040/src/main.rs> to `src/main.rs`.

Next, you have to check `src/main.rs`, make sure that the binded USB interrupt is right. Different microcontrollers have different types of USB peripheral, so does bind interrupt. You can check out [Embassy's examples](https://github.com/embassy-rs/embassy/tree/main/examples) for how to bind the USB interrupt correctly.

For example, if you're using stm32f4, there is an [usb serial example](https://github.com/embassy-rs/embassy/blob/main/examples/stm32f4/src/bin/usb_serial.rs) there. And code for binding USB interrupt is at [line 15-17](https://github.com/embassy-rs/embassy/blob/main/examples/stm32f4/src/bin/usb_serial.rs#L15-L17):

```rust
bind_interrupts!(struct Irqs {
    OTG_FS => usb::InterruptHandler<peripherals::USB_OTG_FS>;
});
```

### Add your own layout

The next step is to add your own keymap layout for your firmware. RMK supports [vial app](https://get.vial.today/), an open-source cross-platform(windows/macos/linux/web) keyboard configurator. So the vial like keymap definition has to be imported to the firmware project.

Fortunately, RMK does most of the heavy things for you, all you need to do is to create your own keymap definition and convert it to `vial.json` following **[vial's doc here](https://get.vial.today/docs/porting-to-via.html)**, and place it at the root of the firmware project, replacing the default one. RMK would do all the rest things for you.

### Add your default keymap

After adding the layout of your keyboard, the default keymap should also be updated. The default keymap is defined in `src/keymap.rs`, update keyboard matrix constants and add a `get_default_keymap()` function which returns the default keymap of your keyboard.

RMK provides a bunch of useful [macros](https://docs.rs/rmk/latest/rmk/#macros) helping you define your keymap. Check out [keymap_configuration](keymap.md) chapter for more details. You can also check `src/keymap.rs` files under <https://github.com/HaoboGu/rmk/blob/main/examples/use_rust> examples for reference.

Some `KeyAction`s are not supported by the macros, plain `KeyAction`s also work, for example: `KeyAction::TapHold(Action::Key(KeyCode::Kc1), Action::Key(KeyCode::Kc2))`

### Define your matrix

Next, you're going to change the IO pins of keyboard matrix making RMK run on your own PCB. Generally, IO pins are defined in `src/main.rs`. RMK will generate a helper macro to help you to define the matrix. For example, if you're using rp2040, you can define your pins using `config_matrix_pins_rp!`:

```rust
let (input_pins, output_pins) = config_matrix_pins_rp!(
    peripherals: p,
    input: [PIN_6, PIN_7, PIN_8, PIN_9],
    output: [PIN_19, PIN_20, PIN_21]
);
```

`input` and `output` are lists of used pins, change them accordingly.

If your keys are directly connected to the microcontroller pins, you can define your pins like this:

```rust
    let direct_pins = config_matrix_pins_rp! {
        peripherals: p,
        direct_pins: [
            [PIN_0, PIN_1,  PIN_2],
            [PIN_3, _,  PIN_5],
        ]
    };
```

So far so good, you've done all necessary modifications of your firmware project. You can also check TODOs listed in the generated `README.md` file.
