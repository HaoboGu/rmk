# RMK

Keyboard firmware written in Rust. Tested on stm32 and rp2040.


## Features & TODOs

A lot of todos at the list, any contributions are welcomed :)

- [x] support rp2040
- [x] basic keyboard functions
- [x] layer
- [x] system/media keys
- [x] eeprom
- [ ] 🚧 via/vial support
- [ ] macro
- [ ] encoder
- [ ] RGB
- [ ] cli

## Prerequisites

This crate requires **nightly** Rust. `openocd`(stm32) or `probe-rs`(rp2040) is used for flashing & debugging.

## Usage

Example can be found at [`boards`](https://github.com/HaoboGu/rmk/blob/main/boards). The following is a simple step-to-step instruction for creating your own firmware:

1. Create a rust embedded project, Add rmk to your project
2. Choose your target, use `rustup target add <your-target-name>` to install the target. [Here](https://docs.rust-embedded.org/book/intro/install.html) is the doc for target choosing. For example, rp2040 is Cortex-M0+, so its corresponding target is `thumbv6m-none-eabi`.
3. Create `.cargo/config.toml` in your project's root, specify your target here. See [`boards/stm32h7/.cargo/config.toml`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/.cargo/config.toml)
4. Create `main.rs`, initialize your MCU in rtic's `mod app`, create usb polling task and keyboard task. See [`boards/stm32h7/src/main.rs`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/src/main.rs)

## Compile

Just run
```
# Compile stm32 example
cd boards/stm32h7
# Compile rp2040 example
# cd boards/rp2040
cargo build

# Check binary size after build
cargo size
```

## Flash

### pi-pico(rp2040)

Flashing rp2040 is quite simple:
```shell
cd boards/rp2040
cargo run
```

### stm32
Requires `openocd`.

VSCode: Press `F5`, the firmware will be automatically compiled and flashed. A debug session is started after flashing. Check `.vscode/tasks.json` for details.

Or you can do it manually using this command after compile:
```shell
openocd -f openocd.cfg -c "program target/thumbv7em-none-eabihf/debug/rmk-stm32h7 preverify verify reset exit"
``` 

