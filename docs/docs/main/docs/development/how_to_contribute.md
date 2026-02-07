# Contributing

ANY contributions are welcome! Here is a simple step-by-step guide for developers:

1. Before you start, you may want to read the [Under the Hood] section to understand how RMK works. [GitHub Issues](https://github.com/haobogu/rmk/issues) is also a good place for questions.

2. Check out the active PRs to make sure that what you want to add isn't already being implemented by others.

3. Write your code!

4. Open a PR to merge your code into the main repo. Make sure all CIs pass.

## Under the Hood

If you're not familiar with RMK, the following is a simple introduction to the source code of RMK.

### Project Architecture

There are four crates in the RMK project: `rmk`, `rmk-config`, `rmk-types` and `rmk-macro`.

- `rmk`: The main crate that contains the core firmware logic, including matrix scanning, key processing, USB/BLE communication, and all the runtime services.
- `rmk-macro`: A proc-macro helper for RMK that reads the `keyboard.toml` config file, converts the TOML config to RMK config, and generates the boilerplate code.
- `rmk-config`: Contains the configuration data structures and parsing logic shared between `rmk-macro` and `rmk`, defining how keyboard configurations are represented in memory.
- `rmk-types`: Provides common type definitions used across all RMK crates, such as keyboard actions, key events, and other shared data structures.

So, if you want to contribute new features to RMK, look into the `rmk` core crate. If you want to add support for a new chip, both `rmk` and `rmk-macro` should be updated so that users can use `keyboard.toml` to configure keyboards with your new chip. If you want to add new configurations, look into both `rmk-macro/config` and `rmk/config`.

### RMK Core

The `rmk` crate is the main crate. It provides several entry APIs to start the keyboard firmware. All the entry APIs are similar; they:

- Initialize the storage, keymap, and matrix first
- Create services: main keyboard service, matrix service, USB service, BLE service, vial service, light service, etc.
- Run all tasks in an infinite loop; if a task fails, wait some time and rerun it

Generally, there are 4-5 running tasks at the same time, depending on the user's config. Communication between tasks is done via channels. There are several built-in channels:

- `FLASH_CHANNEL`: a multi-sender, single-receiver channel. Many tasks send `FlashOperationMessage`, such as the BLE task (which saves bond info) and the vial task (which saves keys), etc.
- **Input event channels**: RMK uses a type-safe event system where each input event type (e.g., `KeyboardEvent`, `PointingEvent`, `BatteryAdcEvent`) has its own dedicated channel. Input devices publish events to their typed channels, and input processors subscribe to the event types they care about.
- **Controller event channels**: Similarly, controller events (e.g., `LayerChangeEvent`, `BatteryStateEvent`) each have their own `PubSubChannel`, allowing controllers to subscribe to specific event types independently.
- `KEYBOARD_REPORT_CHANNEL`: a single-sender, single-receiver channel. The keyboard task sends keyboard reports to the channel after the key event is processed, and the USB/BLE task receives the keyboard report and sends the key to the host.

### Matrix Scanning & Key Processing

An important part of keyboard firmware is how it performs [matrix scanning](https://en.wikipedia.org/wiki/Keyboard_matrix_circuit) and how it processes the scanning result to generate keys.

In RMK, this work is done by `Matrix` and `Keyboard` respectively. The `Matrix` scans the key matrix and sends a `KeyboardEvent` if there's a key change in the matrix. Then the `Keyboard` receives the `KeyboardEvent` and processes it into an actual keyboard report. Finally, the keyboard report is sent to the USB/BLE tasks and forwarded to the host via USB/BLE.
