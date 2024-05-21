# RMK Examples

RMK provides two ways to create your firmware: 

1. Create a `keyboard.toml` and then uses `#[rmk_keyboard]` macro, see [examples/use_config](https://github.com/HaoboGu/rmk/tree/main/examples/use_config)

2. Write your own firmware using RMK API, see [examples/use_rust](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust)

The toml configuration + macro way is super easy to use, everything is configured in `keyboard.toml`, no Rust code required.

The second way suits for people who want flexibility and extendability and are comfortable writing Rust code. 

RMK provides examples for both way, check out the [use_config](https://github.com/HaoboGu/rmk/tree/main/examples/use_config) and [use_rust](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust) folder in examples.
