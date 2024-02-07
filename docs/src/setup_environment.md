# Setup RMK environment

## 1. Prerequisites

Creating a RMK firmware requires basic knowledge of programming and embedded devices. We recommend you to read [The Embedded Rust Book](https://docs.rust-embedded.org/book/) first if you're not familiar with embedded Rust.

## 2. Install Rust

First you have to install Rust. Installing Rust is easy: checkout [https://rustup.rs](https://rustup.rs) and follow the instructions.

## 3. Choose your hardware

RMK firmware runs on your microcontroller, which is quite different from a normal Rust development environment. So you need to know, which microcontroller you want to use. By using [Embassy](https://github.com/embassy-rs/embassy) as the runtime, RMK supports many microcontrollers, like stm32, nrf52 and rp2040. Choose one of the supported microcontroller makes your journal of RMK much easier. Note that different microcontrollers have different architechtures, you have to know the target of your microcontroller, making sure that you're compiling correct firmware for your hardware.

## 4. Install your target

The Rust target of your microcontroller should be installed. Use `rustup target add` command to intall it:

```bash
rustup target add <target>
```

For example, rp2040's is a cortex-M0 microcontroller, so it's target is `thumbv6m-none-eabi`. You have to run `rustup target add thumbv6m-none-eabi` for compiling a firmware for rp2040.