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

