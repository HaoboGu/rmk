# Setup RMK environment

In this section, you'll setup the Rust development environment, install all needed components for compiling and flashing RMK.

## 1. Install Rust

RMK is written in Rust, so first you have to install Rust to your host. Installing Rust is easy, checkout [https://rustup.rs](https://rustup.rs) and follow the instructions.

[Here](https://doc.rust-lang.org/book/ch01-01-installation.html) is a more detailed guide for installing Rust.

## 2. Choose your hardware and install the target

RMK firmware runs on microcontrollers. By using [Embassy](https://github.com/embassy-rs/embassy) as the runtime, RMK supports many series of microcontrollers, such as stm32, nrf52 and rp2040. Choose one of the supported microcontroller makes your journey of RMK much easier. In RMK repo, there are many examples, microcontrollers in examples are safe options. If you're using other microcontrollers, make sure your microcontroller supports [Embassy](https://github.com/embassy-rs/embassy).

The next step is to add Rust's compilation target of your chosen microcontroller. Rust's default installation include only your host's compilation target, so you have to install the compilation target of your microcontroller manually.

Different microcontrollers with different architectures may have different compilation targets, if you're using ARM Cortex-M microcontrollers, [here](https://docs.rust-embedded.org/book/intro/install.html#rust-toolchain) is a simple target list.

For example, rp2040 is a Cortex-M0+ microcontroller, it's compilation target is `thumbv6m-none-eabi`. Use `rustup target add` command to install it:

```bash
rustup target add thumbv6m-none-eabi
```

nRF52840 is also commonly used in wireless keyboards, it's compilation target is `thumbv7em-none-eabihf`. To add the target, run:

```bash
rustup target add thumbv7em-none-eabihf
```

## 4. Add other tools

There are several other tools are highly recommended:

- `cargo generate`: needed for creating a RMK firmware project from [RMK project template](https://github.com/HaoboGu/rmk-template).

- `probe-rs`: used to flash and debug your firmware. [Here](https://probe.rs/docs/getting-started/installation/) is the installation instruction.

- `flip-link`: zero-cost stack overflow protection.

You can use the following commands to install them:

```bash
  # Install cargo-generate and flip-link
  cargo install cargo-generate flip-link

  # Install probe-rs using scripts
  # Linux, macOS
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
  # Windows
  irm https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.ps1 | iex
  ```

Now you're all set for RMK! In the next section, you'll learn how to create your own RMK firmware project. 