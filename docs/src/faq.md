# FAQ

### I can see a `RMK Start` log, but nothing else

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

### rust-lld: error: section will not fit in region 'FLASH': overflowed by x bytes

This is because your MCU's flash is too small. Try building in release mode: `cargo build --release`. If the error still there, follow our [`binary size optimization`](binary_size_optimization.md) doc to reduce your code size.

### I see ERROR: Storage is full error in the log

By default, RMK uses only 2 sectors of your microcontroller's internal flash. You may get the following error if 2 sectors is not big enough to store all your keymaps: 

```
ERROR Storage is full
└─ rmk::storage::print_sequential_storage_err @ /Users/haobogu/Projects/keyboard/rmk/rmk/src/storage.rs:577 
ERROR Got none when reading keymap from storage at (layer,col,row)=(1,5,8)
└─ rmk::storage::{impl#2}::read_keymap::{async_fn#0} @ /Users/haobogu/Projects/keyboard/rmk/rmk/src/storage.rs:460 
ERROR Keymap reading aborted!
└─ rmk::keymap::{impl#0}::new_from_storage::{async_fn#0} @ /Users/haobogu/Projects/keyboard/rmk/rmk/src/keymap.rs:38  
```

If you have more sectors available in your internal flash, you can increase `num_sectors` in `[storage]` section of your `keyboard.toml`, or change `storage_config` in your [`RmkConfig`](https://docs.rs/rmk-config/latest/rmk_config/keyboard_config/struct.RmkConfig.html) if you're using Rust API.

### panicked at embassy-executor: task arena is full.

The current embassy requires manually setting of the task arena size. By default, RMK set's it to 8192 in all examples:

```toml
# Cargo.toml
embassy-executor = { version = "0.6", features = [
    "defmt",
    "arch-cortex-m",
    "task-arena-size-8192",
    "executor-thread",
    "integrated-timers",
] }
```

If you got `ERROR panicked at 'embassy-executor: task arena is full.` error after flashing to your MCU, that means that you should increase your embassy's task arena. Embassy has a series cargo features to do this, for example, changing task arena size to 65536:

```diff
# Cargo.toml
embassy-executor = { version = "0.6", features = [
    "defmt",
    "arch-cortex-m",
-   "task-arena-size-8192",
+   "task-arena-size-65536",
    "executor-thread",
    "integrated-timers",
] }
```

In the latest git version of embassy, task arena size could be calculated automatically, but it requires **nightly** version of Rust.

If you're comfortable with nightly Rust, you can enable `nightly` feature of embassy-executor and remove `task-arena-size-*` feature.

