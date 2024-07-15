# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)
[![Discord](https://img.shields.io/discord/1166665039793639424?label=discord)](https://discord.gg/HHGA7pQxkG)

## 特性

- **MCU支持丰富**：基于 [embassy](https://github.com/embassy-rs/embassy)，RMK 支持非常多的MCU系列，例如 stm32/nrf/rp2040/esp32等。
- **实时键位编辑**：RMK 默认支持 vial 进行实时键位编辑，即时生效。
- **高级键盘功能**：RMK 默认提供许多高级键盘功能，如层切换、媒体控制、系统控制、鼠标控制等。
- **无线支持**：RMK 支持 BLE 无线连接，包括自动重连和多设备支持，已经在 nrf52840 和 esp32 上进行了测试。
- **易于配置**：RMK提供了一个非常简单的配置键盘的方法，你只需要一个`keyboard.toml`文件，就可以构建起你的键盘固件（不需要写任何Rust代码）！当然，对于 Rust 开发者来说，你仍然可以使用代码方式来使用 RMK 从而获得更大的灵活性。
- **低延迟、低电量消耗**：根据测试，RMK在有线模式下延迟约为2ms，蓝牙模式下延迟约为10ms。在开启`async_matrix` feature之后，RMK有着非常低的电量消耗，一块2000mah的电池可以续航好几个月。

## [用户文档](https://haobogu.github.io/rmk/guide_overview.html) | [API文档](https://docs.rs/rmk/latest/rmk/) | [FAQs](https://haobogu.github.io/rmk/faq.html) | [更新日志](https://github.com/HaoboGu/rmk/blob/main/rmk/CHANGELOG.md)

## 使用 RMK

### 选项 1：从模板初始化
你可以使用RMK提供的模板仓库 [rmk-template](https://github.com/HaoboGu/rmk-template) 来初始化你的固件工程

```shell
cargo install cargo-generate
cargo generate --git https://github.com/HaoboGu/rmk-template
```

生成固件工程之后，按照`README.md`中的步骤进行操作。有关详细信息，请查看 RMK 的 [用户指南](https://haobogu.github.io/rmk/guide_overview.html)。

### 选项 2：尝试内置的例子

RMK 内置了一些常见 MCU 的示例，这些示例可以在 [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples) 中找到。下面是 rp2040 的示例的简单说明：

#### rp2040

1. 安装 [probe-rs](https://github.com/probe-rs/probe-rs)

   ```shell
   cargo install probe-rs --features cli
   ```

2. 构建固件

   ```shell
   cd examples/use_rust/rp2040
   cargo build
   ```

3. 烧录固件

   如果你的 rp2040 已经通过调试器连接，那么可以使用下面的命令把 RMK 固件烧录到开发板上：

   ```shell
   cd examples/use_rust/rp2040
   cargo run
   ```

4. 通过USB烧录

   如果你没有调试器，那么可以使用 `elf2uf2-rs` 通过 USB 烧录固件，但是这种方式需要一些额外的步骤：

   1. 安装 `elf2uf2-rs`: `cargo install elf2uf2-rs`
   2. 更新 `examples/use_rust/rp2040/.cargo/config.toml` 文件，使用 `elf2uf2` 作为默认的烧录命令
      ```diff
      - runner = "probe-rs run --chip RP2040"
      + runner = "elf2uf2-rs -d"
      ```
   3. 按住 BOOTSEL 的同时插上你的 rp2040 开发板的 USB 线，然后应该有一个叫 `rp` 的U盘出现
   4. 执行下面的命令烧录
      ```shell
      cd examples/use_rust/rp2040
      cargo run
      ```
      如果你看到下面这样的日志，那说明烧录成功了
      ```shell
      Finished release [optimized + debuginfo] target(s) in 0.21s
      Running `elf2uf2-rs -d 'target\thumbv6m-none-eabi\release\rmk-rp2040'`
      Found pico uf2 disk G:\
      Transfering program to pico
      173.00 KB / 173.00 KB [=======================] 100.00 % 193.64 KB/s  
      ```

## [Roadmap](https://haobogu.github.io/rmk/roadmap.html)

RMK 现在的 roadmap 在[这里](https://haobogu.github.io/rmk/roadmap.html).

## 最小支持的 Rust 版本（MSRV）

RMK 需要 Rust 1.77 稳定版本及以上。之前的版本可能能用，但不保证。

## 许可证

RMK 根据以下任一许可证许可：

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

你可以自由选择.
