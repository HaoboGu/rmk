# Configuration(Draft)

The goal of RMK's configuration system is to provide users an easy and accessible way to set up keyboards (with or without Rust).

Apparently, a config file could be better for more people who don't know Rust, but we also want to keep some flexibility for customizing keyboard with Rust code.

There are two choices right now:

- [`cfg-toml`](https://github.com/jamesmunns/toml-cfg)
  - pros: 
    - a widely used lib
    - could overwrite default configs defined in RMK
    - easy to use 
  - cons:
    - need to add extra annotations to all config structs
    - some fields are not support
    - hard to expand to other types, accepts only numbers/strings in toml

- `build.rs`: Load the config in `build.rs`, then generate Rust code, which could be passed to RMK as config struct
  - pros:
    - Extendable, flexible, can do everything
    - No extra dependency
    - Need to access RMK config at build time
  - cons:
    - Need to distribute `build.rs`, users cannot use the lib without this file, which is not a common way generally
    - LOTS OF work

## Configuration file

A `toml` file named `rmk.toml` is used as a configuration file. The following is the spec of `toml`:
  - [English](https://toml.io/en/v1.0.0)
  - [中文](https://toml.io/cn/v1.0.0)

### What's in the config file?

The config file should contain EVERYTHING that users could customize.

The following is an example of RMK config file:

```toml
[keyboard]
name = "RMK Keyboard"
vendor_id = 0x4c4b
product_id = 0x4643
manufacturer = "RMK"
chip = "stm32h7b0vb"

[matrix]
rows = 4
cols = 3
layers = 2
# Input and output pins are mandatory
input_pins = ["PD4", "PD5", "PD6", "PD3"]
output_pins = ["PD7", "PD8", "PD9"]
# Default is col2row, uncomment if your pcb is row2col
# row2col = true

[layout]
# TODO: keyboard's default layout and keymap, be compatible with vial json and KLE
# TODO: Could `VIAL_KEYBOARD_DEF/ID` be generated using this? If so, we don't need a vial.json anymore

[light]
# All light pins are high-active by default, uncomment if you want it to be low-active
capslock.pin = "PA4"
# capslock.low_active = true
scrolllock.pin = "PA3"
# scrolllock.low_active = true
# Just ignore if no light pin is used for it
# numslock.pin = "PA5"
# numslock.low_active = true

# TODO: RGB configs
# rgb.driver = "ws2812"

[storage]
# Enable storage by default?
enabled = true
# num_sectors = 2
# start_addr = 0x10000

[ble]
enabled = true
battery_pin = "PA0"
charge_state.pin = "PA0"
charge_state.low_active = true


```

## Problems

Besides the above choosing, there's some other problems that have to be addressed.

1. The first one is, how to deserialize those configs to RMK Config? 
   1. Using serde would be a way, but it requires some other annotations on RMK Config structs(may cause extra flash usage? TODO: test it)
   2. Another way is to define every field in config and convert then to RMK Config struct by hand. Seems to be a lot of works, but it's one-time investment.

2. The second problem is, how to convert different representations of GPIOs of different chips? For example, STMs have something like `PA1`, `PB2`, `PC3`, etc. nRFs have `P0_01`, ESPs have `gpio1`, rp2040 has `PIN_1`. Do we need a common representation of those different pin names? Or we just save strings in toml and process them differently.

3. There are some other pheriphals are commonly used in keyboards, such as spi, i2c, pwm and adc. There are some HAL traits for spi/i2c, so there're good. But for adc, there is no common trait AFAIK. For example, in `embassy-nrf`, it's called `SAADC` and it does not impl any external trait! How to be compatible with so many pheriphals?

4. What if the config in toml is conflict with feature gate in `Cargo.toml`? Move some of configs to `Cargo.toml`, or put them all in config file and update feature gate by config?

## `build.rs` method

First, I would give `build.rs` method a try. 

`toml` crate will be used to parse config file. BUT, there's an issue: