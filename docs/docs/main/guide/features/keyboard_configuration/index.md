# Keyboard Configuration

RMK provides a simple and accessible way to configure your keyboard using a TOML configuration file, requiring no Rust programming knowledge!

## Overview

Keyboard configuration in RMK is handled through a `keyboard.toml` file that defines all aspects of your keyboard setup. TOML is a human-readable configuration format that's easy to understand and edit.

::: tip New to TOML?
If you're unfamiliar with TOML syntax, check out the TOML Specification:
- [English](https://toml.io/en/v1.0.0) / [中文](https://toml.io/cn/v1.0.0)

:::

## Quick Setup

Setting up your keyboard configuration is straightforward:

1. Create a `keyboard.toml` file in your project root
2. Add the RMK keyboard macro to your `main.rs`:

```rust
use rmk::macros::rmk_keyboard;

#[rmk_keyboard]
mod my_keyboard {}
```

That's it! The `#[rmk_keyboard]` macro automatically loads your `keyboard.toml` configuration and generates everything needed for your RMK keyboard.

For complete examples, explore the [`examples/use_config`](https://github.com/HaoboGu/rmk/tree/main/examples/use_config) directory.

## Configuration Sections

The `keyboard.toml` file contains comprehensive customization options for your keyboard. For the complete specification, refer to [**Configuration Reference**](configuration/appendix.md). 


### Available Configuration Sections

The following sections can be configured in your `keyboard.toml`:

- **[Keyboard and Matrix](configuration/keyboard_matrix.md)** - Basic keyboard information and physical key matrix definition
- **[Layout](configuration/layout.md)** - Keyboard layout and default keymap configuration  
- **[Split Keyboard](configuration/split_keyboard.md)** - Configuration for split keyboard setups
- **[Storage](configuration/storage.md)** - On-board configuration and keymap storage settings
- **[Behavior](configuration/behavior.md)** - Advanced keyboard behaviors (one-shot keys, tri-layer, tap-hold, morse key, home row mods, etc.)
- **[Input Devices](configuration/input_device.md)** - Configuration for rotary encoders, joysticks, and other input devices
- **[Wireless/Bluetooth](configuration/wireless.md)** - Wireless and Bluetooth connectivity settings
- **[Lighting](configuration/light.md)** - RGB lighting and LED configuration
- **[RMK Config](configuration/rmk_config.md)** - Internal RMK settings (communication channels, macro limits, etc.)
- **[Complete Reference](configuration/appendix.md)** - Full specification and examples for `keyboard.toml`

We also provide pre-configured templates for popular microcontroller chips in the [`rmk-config/src/default_config`](https://github.com/HaoboGu/rmk/blob/main/rmk-config/src/default_config) directory. You can use then when generating project using [rmkit](https://github.com/HaoboGu/rmkit). Contributions for additional chip configurations are welcome!
