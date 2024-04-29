# FAQ

## I can see a `RMK Start` log, but nothing else

First you need to check the RCC config of your board, make sure that the USB's clock is enabled and set to 48MHZ. For example, if you're using stm32f1, you can set the RCC as the following:

```rust
// If you're using a keyboard.toml
#[rmk_keyboard]
mod keyboard {
    use embassy_stm32::{time::Hertz, Config};

    #[Override(chip_config)]
    fn config() -> Config {
        let mut config = Config::default();
        config.rcc.hse = Some(Hertz(8_000_000));
        config.rcc.sys_ck = Some(Hertz(48_000_000));
        config.rcc.pclk1 = Some(Hertz(24_000_000)); 
        config
    }
}
```

If the keyboard still doesn't work, enabling full logging trace at `.cargo/config.toml`:

```toml
[env]
DEFMT_LOG = "trace"
```

run `cargo clean` and then `cargo run --release`. Open an [issue](https://github.com/HaoboGu/rmk/issues) with the detailed logs.

## rust-lld: error: section will not fit in region 'FLASH': overflowed by x bytes

This is because your MCU's flash is too small. Try building in release mode: `cargo build --release`. For check out our [`binary size optimization`](https://haobogu.github.io/rmk/binary_size.html) doc
