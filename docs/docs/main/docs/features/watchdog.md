# Watchdog

A hardware watchdog resets the MCU when the firmware stops feeding it in time,
recovering the keyboard from a hang without a physical unplug. RMK feeds the
watchdog from a dedicated Embassy task.

Because all `Runnable` tasks are joined cooperatively, a tight-loop stall in any
sibling task (for instance: matrix scan, USB, BLE) blocks the watchdog task from
feeding, so the timeout expires and the hardware resets the MCU.

The `watchdog` feature is enabled by default, no user configuration is
required.

Supported chips: RP2040, nRF52, ESP32 (see
[`rmk/src/watchdog`](https://github.com/HaoboGu/rmk/tree/main/rmk/src/watchdog)).
STM32 has no automatic watchdog codegen.

## How it works

- `WatchdogFeed` is a small trait each chip implements to feed its hardware
  timer.
- `WatchdogRunner<W>` wraps a `WatchdogFeed` and calls `feed()` on a fixed
  interval.
- `rmk-macro` generates chip-specific init code and joins a
  `watchdog_runner.run()` task alongside the keyboard and matrix tasks.
- Each chip module provides a `default_runner()` with a timeout/feed interval
  pair:

| Chip   | Hardware timeout  | Feed interval |
| ------ | ----------------- | ------------- |
| RP2040 | 8s                | 4s            |
| nRF52  | 10s               | 5s            |
| ESP32  | 10s               | 5s            |

## Adding support for other chips

`WatchdogFeed` and `WatchdogRunner` are public, so downstream consumers
using the Rust API aren't limited to the chips `rmk-macro` generates code
for. Implement `WatchdogFeed` for your chip's watchdog peripheral, build a
`WatchdogRunner` around it, and join its `.run()` alongside your other
tasks.
