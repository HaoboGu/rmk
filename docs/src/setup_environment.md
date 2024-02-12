# Setup RMK environment

In this section, you'll setup the Rust development environment, install all needed components for compiling and flashing RMK.

## 1. Install Rust

First you have to install Rust. 

Installing Rust is easy: checkout [https://rustup.rs](https://rustup.rs) and follow the instructions.

## 2. Choose your hardware

RMK firmware runs on microcontrollers, by using [Embassy](https://github.com/embassy-rs/embassy) as the runtime, RMK supports many series of microcontrollers, such as stm32, nrf52 and rp2040. Choose one of the supported microcontroller makes your journey of RMK much easier. 

## 3. Install your target

The next step is to add Rust's compilation target of your chosen microcontroller. Rust's default installation include only your host's compilation target, so you have to install the compilation target manually.

Different microcontrollers with different architectures have different compilation targets, you should choose it properly. [Here](https://docs.rust-embedded.org/book/intro/install.html#rust-toolchain) is a simple target list of ARM Cortex-M microcontrollers. The full list of targets can be found [here](https://doc.rust-lang.org/nightly/rustc/platform-support.html).

For example, rp2040 is a Cortex-M0+ microcontroller, it's compilation target is `thumbv6m-none-eabi`. Use `rustup target add` command to intall it:


```bash
rustup target add thumbv6m-none-eabi
```


## 4. Add other tools

There are several other tools are highly recommended:

- `cargo generate`: needed for creating a RMK firmware project from offcial [project template](https://github.com/HaoboGu/rmk-template)

- `probe-rs`: used to flash and debug your firmware

You can use the following commands to install them:

```bash
cargo install cargo-generate
cargo install probe-rs --features cli
```

