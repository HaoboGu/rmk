# Create RMK project

In this section, you'll create your own RMK firmware project using [RMK project template](https://github.com/HaoboGu/rmk-template) and [cargo-generate](https://github.com/cargo-generate/cargo-generate).

## 1. Create from template

RMK provides a [project template](https://github.com/HaoboGu/rmk-template), making it much easier to create your own firmware using your favorate microcontroller. `cargo-generate` is required to use the template, you can install it using the following command:

```bash
cargo install cargo-generate
```

Then you can create your RMK firmware project with a single command:

```bash
cargo generate --git https://github.com/HaoboGu/rmk-template
```

This command would ask you to fill some basic info of your project, it requires a little bit deeper understanding of your chosen hardware. If you don't know what to fill, check [this section]() in overview first. The following is an example. In the example, a `stm32` microcontroller `stm32h7b0vb` is used, the corresponding target is `thumbv7em-none-eabihf`:

```shell
$ cargo generate --git https://github.com/HaoboGu/rmk-template
🤷   Project Name: rmk-demo
🔧   Destination: /Users/haobogu/Projects/keyboard/rmk-demo ...
🔧   project-name: rmk-demo ...
🔧   Generating template ...
✔ 🤷   Choose your microcontroller family · stm32
✔ 🤷   Choose your microcontroller's target · thumbv7em-none-eabihf
🤷   Enter your MCU model(Embassy feature name): stm32h7b0vb
️️👉👉👉 For the following steps, search 'TODO' in generated project
🔧   Moving generated files into: `/Users/haobogu/Projects/keyboard/rmk-demo`...
🔧   Initializing a fresh Git repository
✨   Done! New project created /Users/haobogu/Projects/keyboard/rmk-demo
```

A RMK firmware project will be automatically created after you fill out all required fields. Use `code <your-project-name>` to open the project in VSCode.

## 2. Modify the template project

If you're lucky enough, you could just compile your firmware using `cargo build`. But for the most of the cases, there are minor modifications you have to do. All TODOs are listed in the `README.md` file in the generated project.

The followings are the detailed steps:

### 2.1 Update memory.x

`memory.x` is the linker script of Rust embedded project, it's used to define the memory layout of the microcontroller. For most ARM Cortex-M microcontrollers, you only need to update the `LENGTH` of FLASH and RAM to your microcontroller. You can either checkout your microcontroller's datasheet or existing Rust project of your microcontroller for it. 

### 2.2 Update USB interrupt binding in `main.rs`

Next, you have to check generated `src/main.rs`, make sure that the binded USB interrupt is right. Different microcontrollers have different types of USB peripheral, so does binded interrupt. You can check out [Embassy's examples](https://github.com/embassy-rs/embassy/tree/main/examples) for how to bind the USB interrupt correctly.

For example, if you're using stm32f4, there is a [usb serial example](https://github.com/embassy-rs/embassy/blob/main/examples/stm32f4/src/bin/usb_serial.rs) there. And code for binding USB interrupt is at [line 15-17](https://github.com/embassy-rs/embassy/blob/main/examples/stm32f4/src/bin/usb_serial.rs#L15-L17):

```rust
bind_interrupts!(struct Irqs {
    OTG_FS => usb_otg::InterruptHandler<peripherals::USB_OTG_FS>;
});
```

Don't forget to import all used items!

### 2.3 Add your own layout

The next step is to add your own keymap layout for your firmware. RMK supports [vial app](https://get.vial.today/), an open-source cross-platform(windows/macos/linux/web) keyboard configurator. So the vial like keymap definition has to be imported to the firmware project. 

Fortunately, RMK does most of the heavy things for you, all you need to do is to create your own keymap definition and convert it to `vial.json` following vial's doc **[here](https://get.vial.today/docs/porting-to-via.html)**, and place it at the root of the firmware project, replacing the default one. RMK would do all the rest things for you.

### 2.4 Add your default keymap

After adding the layout of your keyboard, the default keymap should also be updated. The default keymap is defined in `src/keymap.rs`, update keyboard matrix constants and `KEYMAP` according to your keyboard. RMK provides a bunch of useful [macros](https://docs.rs/rmk/latest/rmk/#macros) helping you define your keymap. Check out [keycode(TODO)]() chapter for more details.

So far so good, you've done all necessary modifications of your firmware project. The next step is compiling and flashing your firmware!