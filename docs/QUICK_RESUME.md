# 快速恢复：Gazell 2.4G 无线项目

**最后更新：** 2026-02-13
**Git Commit：** `f376fff41`
**分支：** `feat/pointing-mode`

---

## 🎯 当前状态：代码 100% 完成，等待硬件测试

### 已完成 ✅
- ✅ **rmk-gazell-sys** FFI 底层库（C shim + Rust bindings）
- ✅ **rmk 无线模块集成**（gazell.rs 重构 + feature flags）
- ✅ **示例项目**（dongle 接收器 + keyboard 发射器）
- ✅ **完整文档**（13,000+ 字设置指南 + 状态跟踪）
- ✅ **Git 提交**（28 files, 5968+ lines）

### 待完成 ⏳
- ⏳ 安装 Nordic nRF5 SDK
- ⏳ 验证编译（需要 SDK）
- ⏳ 硬件测试（硬件在途）

---

## 📁 关键文件路径

### 代码实现
```
rmk-gazell-sys/
├── c/gazell_shim.c        # C 封装层（850 行）
├── c/gazell_shim.h        # C API 定义
├── build.rs               # 构建系统
└── src/lib.rs             # Rust FFI 绑定

rmk/src/wireless/
├── gazell.rs              # 主实现（FFI + Mock）
├── config.rs              # 配置结构
├── device.rs              # 设备管理
└── transport.rs           # Trait 定义

examples/use_rust/
├── nrf52840_dongle/       # USB 接收器（Host 模式）
└── nrf52840_2g4/          # 键盘发射器（Device 模式）
```

### 文档
```
docs/
├── GAZELL_SETUP_GUIDE.md             # 👈 完整设置教程（13,000 字）
├── GAZELL_IMPLEMENTATION_STATUS.md   # 👈 详细状态和测试计划
├── GAZELL_FFI_PLAN.md                # 原始设计文档
└── QUICK_RESUME.md                   # 👈 本文件（快速恢复）
```

---

## 🚀 下一步：安装 SDK 并验证编译

### 步骤 1：安装 Nordic SDK（5 分钟）

```bash
# 下载 SDK（约 200MB）
cd ~
wget https://nsscprodmedia.blob.core.windows.net/prod/software-and-other-downloads/sdks/nrf5/binaries/nrf5_sdk_17.1.0_ddde560.zip

# 解压
unzip nrf5_sdk_17.1.0_ddde560.zip -d ~/nRF5_SDK_17.1.0

# 设置环境变量（永久）
echo 'export NRF5_SDK_PATH=~/nRF5_SDK_17.1.0' >> ~/.bashrc
source ~/.bashrc

# 验证安装
ls $NRF5_SDK_PATH/components/proprietary_rf/gzll/gcc/
# 应该看到：libgzll_nrf52840_gcc.a 等文件
```

### 步骤 2：验证编译（10 分钟）

```bash
cd /home/qlg/wkspaces/rmk_q/rmk

# 编译 FFI 层
cd rmk-gazell-sys
cargo build --target thumbv7em-none-eabihf --features nrf52840

# 编译 RMK
cd ../rmk
cargo build --target thumbv7em-none-eabihf --features wireless_gazell_nrf52840

# 编译 Dongle 示例
cd ../examples/use_rust/nrf52840_dongle
cargo build --release --target thumbv7em-none-eabihf

# 编译 Keyboard 示例
cd ../nrf52840_2g4
cargo build --release --target thumbv7em-none-eabihf
```

**预期输出：**
```
Finished release [optimized] target(s) in 2m 15s
```

**如果失败：** 查看 `docs/GAZELL_SETUP_GUIDE.md` 的故障排除部分

### 步骤 3：运行 Mock 测试（可选，无需 SDK）

```bash
cd /home/qlg/wkspaces/rmk_q/rmk/rmk
cargo test wireless --lib
```

应该看到 5+ 个测试通过。

---

## 🔬 硬件到货后：测试计划

### 测试 1：基础通信（P0 - 最高优先级）

**目标：** 验证 Gazell 可以工作

```bash
# 1. 烧录 Dongle
cd examples/use_rust/nrf52840_dongle
probe-rs run --chip nRF52840_xxAA --release

# 2. 烧录 Keyboard（另一终端）
cd ../nrf52840_2g4
probe-rs run --chip nRF52840_xxAA --release

# 3. 观察日志（另一终端）
probe-rs attach --chip nRF52840_xxAA
```

**成功标准：**
- Dongle 显示：`Gazell: Initialized`
- Keyboard 显示：`Sent test packet #0 successfully`
- Dongle 显示：`Received 2.4G packet: 3 bytes`

### 测试 2-5：性能测试

详见 `docs/GAZELL_IMPLEMENTATION_STATUS.md` 的测试计划部分。

---

## 📝 架构速查

### 三层架构
```
┌─────────────────────────────┐
│  examples/nrf52840_dongle   │  ← 应用层
│  examples/nrf52840_2g4      │
└──────────────┬──────────────┘
               │
┌──────────────▼──────────────┐
│  rmk::wireless::            │  ← 安全封装层
│  GazellTransport            │
└──────────────┬──────────────┘
               │
┌──────────────▼──────────────┐
│  rmk-gazell-sys             │  ← FFI 绑定层
│  (C shim + bindgen)         │
└──────────────┬──────────────┘
               │
┌──────────────▼──────────────┐
│  Nordic nRF5 SDK v17.1.0    │  ← 协议栈
└─────────────────────────────┘
```

### Feature Flags
```toml
# rmk/Cargo.toml
wireless_gazell              # 启用 Gazell 支持
wireless_gazell_nrf52840     # + nRF52840 变体
wireless_gazell_nrf52833     # + nRF52833 变体
wireless_gazell_nrf52832     # + nRF52832 变体
```

### 关键 API
```rust
// 初始化（Device 模式 - 键盘）
let config = GazellConfig::low_latency();
let mut gazell = GazellTransport::new(config);
gazell.init()?;
gazell.set_device_mode()?;

// 发送数据包
let frame = [0xAA, 0xBB, 0xCC];
gazell.send_frame(&frame)?;  // 阻塞，等待 ACK

// 初始化（Host 模式 - 接收器）
gazell.set_host_mode()?;

// 接收数据包
if let Some(packet) = gazell.recv_frame()? {  // 非阻塞
    // 处理收到的数据
}
```

---

## 🐛 已知问题

1. **无加密** - 数据明文传输（后续添加 AES-CCM）
2. **单信道** - 固定信道，可能受 WiFi 干扰
3. **无配对** - 任何设备都可连接
4. **功耗未优化** - 持续轮询，未实现睡眠

详见 `docs/GAZELL_IMPLEMENTATION_STATUS.md` 的"已知问题和限制"部分。

---

## 📚 文档导航

| 文档 | 用途 | 何时阅读 |
|------|------|----------|
| **QUICK_RESUME.md** | 快速恢复工作 | 现在（你在这） |
| **GAZELL_SETUP_GUIDE.md** | 完整设置教程 | 开始安装 SDK 时 |
| **GAZELL_IMPLEMENTATION_STATUS.md** | 详细状态 | 需要详细信息时 |
| **GAZELL_FFI_PLAN.md** | 设计文档 | 理解架构决策时 |
| **rmk-gazell-sys/README.md** | FFI 使用说明 | 调试底层时 |

---

## 🔄 Git 信息

```bash
# 当前分支
git branch
# * feat/pointing-mode

# 最新 commit
git log -1 --oneline
# f376fff41 feat: implement Nordic Gazell 2.4G wireless protocol FFI

# 查看改动
git show f376fff41 --stat

# 切换到此状态（如果需要）
git checkout f376fff41
```

---

## 💡 常用命令速查

### 开发命令
```bash
# 编译检查（无需 SDK）
cd rmk
cargo check --features wireless_gazell_nrf52840

# 运行测试（Mock 模式）
cargo test wireless --lib

# 编译 ARM 目标（需要 SDK）
cargo build --target thumbv7em-none-eabihf --features wireless_gazell_nrf52840

# 查看日志
export DEFMT_LOG=trace  # 设置日志级别
```

### 烧录命令
```bash
# 使用 probe-rs（推荐）
probe-rs run --chip nRF52840_xxAA --release

# 使用 nrfjprog
nrfjprog --program target/firmware.hex --chiperase --verify --reset

# USB DFU（nRF52840 Dongle）
nrfutil dfu usb-serial -pkg dongle.zip -p /dev/ttyACM0
```

### 调试命令
```bash
# 附加到运行中的设备（查看日志）
probe-rs attach --chip nRF52840_xxAA

# 查看 USB 设备
lsusb | grep RMK

# 查看 HID 事件
sudo evtest
```

---

## ✅ 恢复工作流程

### 场景 1：继续开发（无硬件）

```bash
# 1. 切换到项目目录
cd /home/qlg/wkspaces/rmk_q/rmk

# 2. 阅读状态文档（可选）
less docs/GAZELL_IMPLEMENTATION_STATUS.md

# 3. 运行 Mock 测试
cd rmk
cargo test wireless --lib

# 4. 修改代码...
```

### 场景 2：安装 SDK 并验证编译

```bash
# 1. 安装 SDK（参考上面的"步骤 1"）
cd ~
wget https://...  # SDK 下载链接
unzip ...
export NRF5_SDK_PATH=~/nRF5_SDK_17.1.0

# 2. 验证编译（参考上面的"步骤 2"）
cd /home/qlg/wkspaces/rmk_q/rmk/rmk-gazell-sys
cargo build --target thumbv7em-none-eabihf --features nrf52840

# 3. 如果失败，查看故障排除
less docs/GAZELL_SETUP_GUIDE.md
# 跳转到 "Troubleshooting" 部分
```

### 场景 3：硬件测试

```bash
# 1. 阅读完整设置指南
less docs/GAZELL_SETUP_GUIDE.md

# 2. 按照 "Step 3: Flash Firmware" 执行

# 3. 按照 "Step 4: Test Wireless Connection" 验证

# 4. 记录测试结果（可以更新 STATUS 文档）
```

---

## 🎯 优先级任务清单

### 现在立即可做（无硬件）
- [ ] 安装 Nordic nRF5 SDK
- [ ] 验证 rmk-gazell-sys 编译通过
- [ ] 验证示例项目编译通过
- [ ] 阅读完整的 GAZELL_SETUP_GUIDE.md

### 硬件到货后（Day 1）
- [ ] 烧录 Dongle 固件
- [ ] 烧录 Keyboard 固件
- [ ] 验证基础通信
- [ ] 截图保存日志输出

### 后续集成（Week 1）
- [ ] 集成键盘矩阵扫描
- [ ] 集成 Elink 协议编码
- [ ] Dongle 端 USB HID 转发
- [ ] 测试真实键盘输入

### 性能优化（Week 2）
- [ ] 延迟测试（< 5ms）
- [ ] 可靠性测试（丢包率 < 0.01%）
- [ ] 范围测试（> 10m）
- [ ] 添加低功耗模式

---

## 📞 需要帮助？

### 编译问题
→ 查看 `docs/GAZELL_SETUP_GUIDE.md` 的 "Troubleshooting" 部分

### 硬件问题
→ 查看 `docs/GAZELL_SETUP_GUIDE.md` 的 "Runtime Issues" 部分

### 架构理解
→ 查看 `docs/GAZELL_FFI_PLAN.md` 和 `docs/GAZELL_IMPLEMENTATION_STATUS.md`

### API 使用
→ 查看 `rmk/src/wireless/gazell.rs` 中的文档注释

---

## 🎉 项目里程碑

- [x] **2026-02-13** - Phase 1-3 实现完成
- [x] **2026-02-13** - 文档完成
- [x] **2026-02-13** - Git commit 提交 (f376fff41)
- [ ] **待定** - SDK 安装和编译验证
- [ ] **待定** - 硬件基础测试
- [ ] **待定** - 性能测试通过
- [ ] **待定** - 完整键盘功能集成

---

**上次停止位置：** 代码实现完成，已提交 Git
**下次继续点：** 安装 Nordic SDK 并验证编译
**预计下次工作时长：** 30 分钟（SDK 安装 + 编译验证）

---

**版本：** v0.1.1
**提交哈希：** f376fff41
**分支：** feat/pointing-mode
**最后更新：** 2026-02-13 23:10 CST
