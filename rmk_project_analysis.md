# RMK（Rust Mechanical Keyboard）项目分析

## 项目概述

RMK是一个用Rust编写的机械键盘固件项目，旨在提供功能丰富、易于配置的键盘固件解决方案。项目当前版本为0.7.4，基于embassy生态系统构建，支持多种微控制器平台。

## 核心特性

### 1. 硬件兼容性
- **广泛的微控制器支持**：通过embassy框架支持多种微控制器
  - STM32系列（F1, F4, H7, G4等）
  - nRF52系列（nRF52840, nRF52832, nRF52833等）
  - RP2040/RP2350（树莓派Pico）
  - ESP32系列（ESP32C3, ESP32C6, ESP32S3）
  - PY32F07x等国产芯片

### 2. 连接方式
- **有线连接**：支持USB HID协议
- **无线连接**：支持BLE蓝牙连接
  - 多设备切换
  - 自动重连
  - 低功耗模式（使用async_matrix特性可实现数月续航）

### 3. 键盘功能
- **动态键盘映射**：原生支持Vial，支持实时键盘配置
- **高级键盘功能**：
  - 多层支持
  - 媒体控制键
  - 系统命令
  - 鼠标控制
  - 宏定义
  - 组合键（Combo）
  - 点击/长按（Tap/Hold）
  - 单次修饰键（One Shot）
  - 点击舞蹈（Tap Dance）

### 4. 分体键盘支持
- 有线分体键盘（通过串口或PIO UART）
- 无线分体键盘（通过BLE）

### 5. 输入设备支持
- 旋转编码器（Rotary Encoder）
- 摇杆（Joystick）
- ADC输入设备

## 项目架构

### 目录结构
```
rmk/
├── rmk/                 # 核心固件库
│   ├── src/
│   │   ├── action.rs    # 按键动作处理
│   │   ├── ble/         # BLE蓝牙相关
│   │   ├── split/       # 分体键盘相关
│   │   ├── usb/         # USB协议相关
│   │   ├── via/         # VIA协议支持
│   │   ├── storage/     # 存储管理
│   │   └── ...
│   └── Cargo.toml
├── rmk-config/          # 配置工具
│   └── src/
│       ├── lib.rs       # 配置文件解析
│       ├── chip.rs      # 芯片配置
│       └── ...
├── rmk-macro/           # 宏定义库
├── examples/            # 示例项目
│   ├── use_config/      # 使用配置文件方式
│   └── use_rust/        # 使用Rust代码方式
└── docs/                # 文档
```

### 核心组件

#### 1. 矩阵扫描（Matrix）
- 支持常规矩阵和直连引脚
- 可配置的防抖动算法
- 异步矩阵扫描（async_matrix）

#### 2. 键盘映射（Keymap）
- 多层支持
- 动态键盘映射
- 行为配置（Behavior）

#### 3. 通信协议
- USB HID协议
- BLE GATT协议
- 分体键盘通信协议

#### 4. 存储管理
- 基于Sequential Storage的配置存储
- 键盘映射持久化
- 可配置的扇区数量和地址

## 使用方式

### 1. 基于配置文件（use_config）
通过`keyboard.toml`配置文件定义键盘参数，使用`rmk_keyboard`宏自动生成代码：

```rust
#![no_std]
#![no_main]

use rmk::macros::rmk_keyboard;

#[rmk_keyboard]
mod keyboard {}
```

### 2. 基于Rust代码（use_rust）
直接使用Rust代码配置键盘，提供更高的灵活性：

```rust
// 配置USB参数
let keyboard_usb_config = KeyboardUsbConfig {
    vid: 0x4c4b,
    pid: 0x4643,
    manufacturer: "Haobo",
    product_name: "RMK Keyboard",
    serial_number: "vial:f64c2b3c:000001",
};

// 初始化键盘
let mut keyboard = Keyboard::new(&keymap);

// 运行键盘
run_rmk(&keymap, driver, &stack, &mut storage, &mut light_controller, rmk_config).await;
```

## 配置系统

### keyboard.toml配置文件结构
```toml
[keyboard]
name = "My Keyboard"
vendor_id = 0x4c4b
product_id = 0x4643
manufacturer = "Haobo"
chip = "nrf52840"

[matrix]
matrix_type = "normal"
input_pins = ["P0_30", "P0_31", "P0_29"]
output_pins = ["P0_28", "P0_03", "P1_10"]

[ble]
enabled = true
battery_adc_pin = "P0_05"

[storage]
start_addr = 0xA0000
num_sectors = 6

[layout]
rows = 3
cols = 10
layers = 4
```

### 可配置参数
- **键盘基本信息**：名称、VID/PID、制造商等
- **矩阵配置**：输入输出引脚、矩阵类型等
- **BLE配置**：电池检测、充电状态等
- **存储配置**：起始地址、扇区数量等
- **布局配置**：行列数、层数等
- **行为配置**：点击/长按、组合键等

## 性能特点

- **低延迟**：有线模式约2ms，无线模式约10ms
- **低功耗**：异步矩阵扫描可实现数月续航
- **高可靠性**：基于embassy的async/await模式

## 开发工具

- **rmkit**：项目初始化和管理工具
- **Vial**：实时键盘配置工具
- **probe-rs**：固件烧录和调试工具

## 与其他固件的对比

| 特性 | RMK | Keyberon | QMK | ZMK |
|------|-----|----------|-----|-----|
| 编程语言 | Rust | Rust | C | C |
| USB支持 | ✅ | ✅ | ✅ | ✅ |
| BLE支持 | ✅ | ❌ | ❌ | ✅ |
| 实时配置 | ✅ | ❌ | ✅ | 🚧 |
| 有线分体 | ✅ | ✅ | ✅ | ❌ |
| 无线分体 | ✅ | ❌ | ❌ | ✅ |
| ARM芯片 | ✅ | ✅ | ✅ | ✅ |
| RISC-V/Xtensa | ✅ | ❌ | ❌ | ❌ |
| 配置难度 | 易（toml） | 难（Rust） | 中（json） | 难（Kconfig） |

## 总结

RMK项目为Rust生态系统提供了一个功能完整、易于使用的键盘固件解决方案。通过配置文件和Rust代码两种方式，满足了不同用户的需求。项目的模块化设计、广泛的硬件支持和丰富的功能特性，使其成为现代键盘固件开发的优秀选择。