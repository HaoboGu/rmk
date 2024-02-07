# Setup RMK environment

## 1. Prerequisites

Creating a RMK firmware requires basic knowledge of programming and embedded devices. We recommend you to read [The Embedded Rust Book](https://docs.rust-embedded.org/book/) first if you're not familiar with embedded Rust.

## 2. Install Rust

First you have to install Rust. Installing Rust is easy: checkout [https://rustup.rs](https://rustup.rs) and follow the instructions.

## 3. Choose your hardware

RMK firmware runs on microcontrollers, by using [Embassy](https://github.com/embassy-rs/embassy) as the runtime, RMK supports many series of microcontrollers, such as stm32, nrf52 and rp2040. Choose one of the supported microcontroller makes your journey of RMK much easier. 

## 4. Install your target

The next step is to add Rust's compilation target of your microcontroller. Rust's default installation doesn't include all compilation targets, you have to install them manually. For example, rp2040 is a Cortex-M0+ microcontroller, it's compilation target is `thumbv6m-none-eabi`. Use `rustup target add` command to intall it:

```bash
rustup target add thumbv6m-none-eabi
```

For Cortex-M microcontrollers, you can get the compilation target of your microcontroller [here](https://docs.rust-embedded.org/book/intro/install.html). The full list of targets can be found [here](https://doc.rust-lang.org/nightly/rustc/platform-support.html)

## 5. Add other tools

There are several other tools are highly recommended:

- `cargo generate`: needed for creating a RMK firmware project from offcial [project template](https://github.com/HaoboGu/rmk-template)

- `probe-rs`: used to flash and debug your firmware

You can use the following commands to install them:

```bash
cargo install cargo-generate
cargo install probe-rs --features cli
```

