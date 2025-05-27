<!-- PROJECT LOGO -->
<br />
<p align="center">
  <a href="https://github.com/haobogu/rmk">
    <img src="https://github.com/HaoboGu/rmk/blob/dad1f922f471127f5449262c4cb4a922e351bf43/docs/images/rmk_logo.svg?raw=true" alt="Logo" width="150">
  </a>

  <p align="center">
  A feature-rich Rust keyboard firmware. 
  <br />
  <br />
  <a href="https://crates.io/crates/rmk"><img src="https://img.shields.io/crates/v/rmk"></a>
  <a href="https://docs.rs/rmk/latest/rmk/"><img src="https://img.shields.io/docsrs/rmk"></a>
  <a href="https://github.com/HaoboGu/rmk/actions"><img src="https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://discord.gg/HHGA7pQxkG"><img src="https://img.shields.io/discord/1166665039793639424?label=discord"></a>
  </p>
</p>

👉 QQ讨论群：196944384

-----
[English](https://github.com/HaoboGu/rmk/blob/main/README.md)


## 特性

- **MCU支持丰富**：基于 [embassy](https://github.com/embassy-rs/embassy)，RMK 支持非常多的MCU系列，例如 stm32/nrf/rp2040/esp32等。
- **实时键位编辑**：RMK 默认支持 Vial 进行实时键位编辑，即时生效。您甚至可以通过 BLE 使用 Vial 来编辑键位。
- **高级键盘功能**：RMK 默认提供许多高级键盘功能，如层切换、媒体控制、系统控制、鼠标控制等。
- **无线支持**：RMK 支持 BLE 无线连接，包括自动重连和多设备支持，已经在 nrf52840 和 esp32 上进行了测试。
- **易于配置**：RMK提供了一个非常简单的配置键盘的方法，你只需要一个`keyboard.toml`文件，就可以构建起你的键盘固件（不需要写任何Rust代码）！当然，对于 Rust 开发者来说，你仍然可以使用代码方式来使用 RMK 从而获得更大的灵活性。
- **低延迟、低电量消耗**：根据测试，RMK在有线模式下延迟约为2ms，蓝牙模式下延迟约为10ms。在开启`async_matrix` feature之后，RMK有着非常低的电量消耗，一块2000mah的电池可以续航好几个月。

## [用户文档](https://haobogu.github.io/rmk/user_guide/1_guide_overview.html) | [API文档](https://docs.rs/rmk/latest/rmk/) | [FAQs](https://haobogu.github.io/rmk/faq.html) | [更新日志](https://github.com/HaoboGu/rmk/blob/main/rmk/CHANGELOG.md)

## 真实例子

### [rmk-ble-keyboard](https://github.com/HaoboGu/rmk-ble-keyboard)

<img src="https://github.com/HaoboGu/rmk/blob/main/docs/images/rmk_ble_keyboard.jpg?raw=true" width="60%">

## 使用 RMK

### 选项 1：从模板初始化
你可以使用[rmkit](https://github.com/HaoboGu/rmkit) 来从RMK的项目模版初始化你的固件工程

```shell
cargo install rmkit flip-link
# 如果你在Windows下安装rmkit报错，那么可以尝试改用下面的命令安装:
# powershell -ExecutionPolicy ByPass -c "irm https://github.com/haobogu/rmkit/releases/download/v0.0.9/rmkit-installer.ps1 | iex"
rmkit init
```

详细步骤介绍，请查看 RMK 的 [用户指南](https://haobogu.github.io/rmk/user_guide/1_guide_overview.html)。

### 选项 2：尝试内置的例子

RMK 内置了一些常见 MCU 的示例，这些示例可以在 [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples) 中找到。下面是 rp2040 的示例的简单说明：

#### rp2040

1. 安装 [probe-rs](https://github.com/probe-rs/probe-rs)

   ```shell
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
   ```

2. 构建固件

   ```shell
   cd examples/use_rust/rp2040
   cargo build --release
   ```

3. 烧录固件

   如果你的 rp2040 已经通过调试器连接，那么可以使用下面的命令把 RMK 固件烧录到开发板上：

   ```shell
   cd examples/use_rust/rp2040
   cargo run --release
   ```

4. 通过USB烧录

   如果你没有调试器，那么可以使用 `elf2uf2-rs` 通过 USB 为 rp2040 烧录固件，但是这种方式需要一些额外的步骤：

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
      cargo run --release
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

RMK 现在的 roadmap 在[这里](https://haobogu.github.io/rmk/roadmap.html)。

## 最小支持的 Rust 版本（MSRV）

RMK 默认使用最新的稳定版本 Rust。其他版本也应该可以使用，但是未经测试。

## 许可证

RMK 根据以下任一许可证许可：

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

你可以自由选择。
