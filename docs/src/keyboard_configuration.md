# Configuration

RMK provides an easy and accessible way to set up the keyboard with a toml config file, even without Rust code!

## Usage 

A `toml` file named `keyboard.toml` is used as the configuration file of RMK. The following is the spec of `toml` if you're unfamiliar with toml:
  - [English](https://toml.io/en/v1.0.0) / [中文](https://toml.io/cn/v1.0.0)

RMK provides a proc-macro to load the `keyboard.toml` at your projects root: `#[rmk_keyboard]`, add it to your `main.rs` like:

```rust
use rmk::macros::rmk_keyboard;

#[rmk_keyboard]
mod my_keyboard {}
```

And, that's it! The `#[rmk_keyboard]` macro would load your `keyboard.toml` config and create everything that's needed for creating a RMK keyboard instance.

If you don't want any other customizations beyond the `keyboard.toml`, `#[rmk_keyboard]` macro will just work. For the examples, please check the [`example/use_config`](https://github.com/HaoboGu/rmk/tree/main/examples/use_config) folder.

## What's in the config file?

The config file contains almost EVERYTHING to customize a keyboard. For the full reference of `keyboard.toml`, please refer to [**this doc**](configuration/appendix.md). Also, we have pre-defined default configurations for chips, at [`rmk-macro/src/default_config`](https://github.com/HaoboGu/rmk/blob/main/rmk-macro/src/default_config) folder. We're going to add default configurations for more chips, contributions are welcome!

The following are the available tables and related documentaion available in `keyboard.toml`:

- [Keyboard and matrix](configuration/keyboard_matrix.md): basic information and physical key matrix definition of the keyboard
- [Layout](configuration/layout.md): layout and default keymap configuration of the keyboard
- [Split keyboard](configuration/split.md): split keyboard configuration
- [Storage](configuration/storage.md): configuration for storage, which is used for on-board config and keymap
- [Behavior](configuration/behavior.md): configuration for advanced keyboard behaviors, such as one-shot key, tri-layer, tap-hold(including HRM mode), etc.
- [Input device](configuration/input_device.md): configuration for input devices, such as rotary encoder, joystick, etc.
- [Wireless/Bluetooth](configuration/wireless.md): configuration for wireless/bluetooth
- [Light](configuration/light.md): configuration for lights
- [Appendix](configuration/appendix.md): full spec and references of the `keyboard.toml`

## TODOs:

- [ ] read vial.json and check whether vial.json is consist of keyboard.toml
