# Create RMK firmware project

In this section, you'll create your own RMK firmware project
using [RMK project template](https://github.com/HaoboGu/rmk-template)
and [cargo-generate](https://github.com/cargo-generate/cargo-generate).

## 1. Create from template

RMK provides a [project template](https://github.com/HaoboGu/rmk-template), making it much easier to create your own
firmware using your favorite microcontroller. `cargo-generate` is required to use the template, you can install it using
the following command:

```bash
cargo install cargo-generate
```

Then you can create your RMK firmware project with a single command:

```bash
cargo generate --git https://github.com/HaoboGu/rmk-template
```

This command would ask you to fill some basic info of your project, it requires a little bit deeper understanding of
your chosen hardware. If you don't know what to fill, check [this section](setup_environment.md/#3-install-your-target) in overview first. The following is an
example. In the example, a `stm32` microcontroller `stm32h7b0vb` is used, the corresponding target
is `thumbv7em-none-eabihf`:

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

A RMK firmware project will be automatically created after you fill out all required fields.
Use `code <your-project-name>` to open the project in VSCode. If you're lucky enough, you project could just compile with `cargo build` command!
But for the most of the cases, there are minor modifications you have to do. There are two ways to use config your RMK keyboard in your firmware project:
  - [use a config file: `keyboard.toml`](config_rmk_project_toml.md)

    For new users, it's recommend to use `keyboard.toml` to config your keyboard. This config file contains almost all about your keyboard, with it, you can create your firmware very conveniently, no Rust code needed! Please check [Keyboard Configuration](configuration.md) feature for configuration details.

  - [use Rust code](config_rmk_project_rust.md)

    If the configuration doesn't satisfy all your needs(it would mostly do!), you can write your own Rust code to do more customization! RMK also provides some [examples](https://github.com/HaoboGu/rmk/tree/main/examples/use_rust) to help you quickly get throught it.

