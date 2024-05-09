# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)
[![Discord](https://img.shields.io/discord/1166665039793639424?label=discord)](https://discord.gg/HHGA7pQxkG)

该文档暂时是机翻+人工修改，后续会更新，欢迎PR！

## 特性

- **支持范围广**：基于 [embassy](https://github.com/embassy-rs/embassy)，RMK 支持非常多的MCU系列，例如 stm32/nrf/rp2040/esp32等。
- **实时键位编辑**：使用 vial 进行实时键位编辑，可以在编译时定制键盘布局。
- **高级键盘功能**：RMK 默认提供许多高级键盘功能，如层切换、媒体控制、系统控制、鼠标控制等。
- **无线支持**：（实验性功能）RMK 支持 BLE 无线功能，包括自动重新连接和多设备功能，已经在 nrf52840 和 esp32c3 上进行了测试。


## 新闻

- [2024.04.07] 现在esp32c3和esp32s3的蓝牙支持已经在主分支上可用，示例可以参考  [`examples/use_rust/esp32c3_ble`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/esp32c3_ble/src/main.rs) 和 [`examples/use_rust/esp32s3_ble`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/esp32s3_ble/src/main.rs)

- [2024.03.07] RMK 添加了对 nrf52840/nrf52832 的 BLE 支持，包括自动重新连接和多设备功能！具体用法可以参考 [examples/use_rust/nrf52840_ble](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/src/main.rs) 和 [examples/use_rust/nrf52832_ble](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52832_ble/src/main.rs) 下的示例

<details>

<summary>点击查看更多</summary>

[2024.02.18] 版本 `0.1.4` 发布了！这个版本加入了一个自动生成 vial 配置的构建脚本，一点点 API 更新以及全新的[用户文档](https://haobogu.github.io/rmk)。

- 下面懒得翻译了，就酱
 
- [2024.01.26] 🎉[rmk-template](https://github.com/HaoboGu/rmk-template) is released! Now you can create your own keyboard firmware with a single command: `cargo generate --git https://github.com/HaoboGu/rmk-template`

- [2024.01.18] RMK just released version `0.1.0`! By migrating to [Embassy](https://github.com/embassy-rs/embassy), RMK now has better async support, more supported MCUs and much easier usages than before. For examples, check [`examples`](https://github.com/HaoboGu/rmk/tree/main/examples) folder!

</details>

## [用户文档（英文）](https://haobogu.github.io/rmk/guide_overview.html) 

## [API 文档](https://docs.rs/rmk/latest/rmk/)

## 使用 RMK

### 选项 1：从模板初始化
你可以使用RMK提供的模板仓库 [rmk-template](https://github.com/HaoboGu/rmk-template) 来初始化你的固件工程

```shell
cargo install cargo-generate
cargo generate --git https://github.com/HaoboGu/rmk-template
```

生成固件工程之后，按照`README.md`中的步骤进行操作。有关详细信息，请查看 RMK 的 [用户指南](https://haobogu.github.io/rmk/guide_overview.html)。

### 选项 2：尝试内置的例子

RMK 内置了一些常见MCU的示例，这些示例可以在 [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples) 中找到。下面是 rp2040 和 stm32h7 的示例的简单说明：

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

   如果你的 rp2040 已经通过调试器连接，那么可以使用下面的命令把RMK固件烧录到开发板上：

   ```shell
   cd examples/use_rust/rp2040
   cargo run
   ```

4. 通过USB烧录

   如果你没有调试器，那么可以使用 `elf2uf2-rs` 通过 USB 烧录固件，但是这种方式需要一些额外的步骤：

   1. 安装 `elf2uf2-rs`: `cargo install elf2uf2-rs`
   2. 更新 `examples/use_rust/rp2040/.cargo/config.toml`文件，使用 `elf2uf2`作为默认的烧录命令
      ```diff
      - runner = "probe-rs run --chip RP2040"
      + runner = "elf2uf2-rs -d"
      ```
   3. 按住BOOTSEL的同时插上你的rp2040的USB线，然后应该有一个叫`rp`的U盘出现
   4. 使用下面的命令烧录
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

RMK 现在的roadmap在[这里](https://haobogu.github.io/rmk/roadmap.html).

## 最小支持的 Rust 版本（MSRV）

RMK 需要 Rust 1.77 稳定版本及以上。

## 许可证

RMK 根据以下任一许可证许可：

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

你可以自由选择.
