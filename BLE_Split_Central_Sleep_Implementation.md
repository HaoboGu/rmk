# BLE Split Central 睡眠功能实现

## 功能概述

为BLE Split Central实现了基于按键超时的睡眠功能，除了现有的广播超时睡眠外，新增以下特性：

1. **按键超时睡眠**：在连接到主机后，如果30分钟没有任何按键活动（包括从Peripheral传来的按键），Central进入睡眠状态
2. **睡眠状态连接管理**：保留Central对主机的连接，降低与Peripheral的通信频率到50ms来节省电量
3. **按键唤醒**：按下Central或Peripheral的任意按键，退出睡眠状态，恢复正常连接参数
4. **可重复睡眠**：唤醒后如果再次30分钟没有按键活动，可以再次进入睡眠

## 实现细节

### 新增的常量和类型

```rust
// 睡眠超时：30分钟
const SLEEP_TIMEOUT_MS: u64 = 30 * 60 * 1000;
// 睡眠状态下的连接间隔：50ms
const SLEEP_PERIPHERAL_INTERVAL_US: u64 = 50000;
// 正常连接间隔：7.5ms
const NORMAL_PERIPHERAL_INTERVAL_US: u64 = 7500;

/// 睡眠状态枚举
enum SleepState {
    Awake,      // 清醒状态
    Sleeping,   // 睡眠状态
}
```

### 新增的信号和状态管理

```rust
// 睡眠状态信号
pub(crate) static CENTRAL_SLEEP: Signal<crate::RawMutex, bool> = Signal::new();
// 最后活动时间信号
pub(crate) static LAST_ACTIVITY_TIME: Signal<crate::RawMutex, Instant> = Signal::new();
```

### 核心功能函数

#### 1. 活动时间更新函数
```rust
/// 更新最后活动时间以指示用户活动
pub(crate) fn update_activity_time() {
    LAST_ACTIVITY_TIME.signal(Instant::now());
    debug!("Updated last activity time due to user activity");
}

/// 检查Central是否正在睡眠
pub(crate) fn is_central_sleeping() -> bool {
    CENTRAL_SLEEP.signaled()
}
```

#### 2. 睡眠管理任务
`sleep_manager_task` 函数负责：
- 监控活动时间，检测是否应该进入睡眠
- 在睡眠超时后调整连接参数
- 检测活动并唤醒系统
- 恢复正常连接参数

#### 3. 连接参数调整
`adjust_peripheral_connection_params` 函数负责：
- 在睡眠模式下设置50ms连接间隔，增加延迟
- 在正常模式下设置7.5ms连接间隔，低延迟

### 集成点

#### 1. 在BLE Split Central驱动中检测按键活动
在 `BleSplitCentralDriver::read()` 方法中：
- 当接收到 `SplitMessage::Key` 或 `SplitMessage::Event` 时
- 调用 `update_activity_time()` 更新活动时间

#### 2. 在键盘处理中检测本地按键活动
在 `Keyboard::process_inner()` 方法中：
- 每次处理按键事件时调用 `update_activity_time()`
- 确保Central本地的按键也能重置睡眠计时器

#### 3. 在连接管理中添加睡眠任务
在 `connect_and_run_peripheral_manager()` 函数中：
- 使用 `select3` 同时运行睡眠管理任务
- 初始化最后活动时间

## 工作流程

### 睡眠流程
1. Central连接到主机和Peripheral
2. 初始化活动时间
3. 睡眠管理任务每5秒检查一次活动时间
4. 如果30分钟无活动，进入睡眠状态：
   - 设置睡眠标志
   - 调整Peripheral连接参数为50ms间隔
   - 增加连接延迟以节省电量

### 唤醒流程
1. 检测到按键活动（Central或Peripheral）
2. 更新活动时间
3. 睡眠管理任务检测到新的活动
4. 退出睡眠状态：
   - 清除睡眠标志
   - 恢复正常连接参数（7.5ms间隔）
   - 降低连接延迟以获得低延迟响应

## 电量优化

### 睡眠状态优化
- **连接间隔**：从7.5ms增加到50ms，减少无线通信频率
- **连接延迟**：从400增加到800，允许设备跳过更多连接事件
- **保持连接**：维持与主机的连接，避免重新配对

### 兼容性
- 与现有的广播超时睡眠机制共存
- 不影响正常的键盘功能
- 支持所有类型的按键活动检测

## 配置参数

| 参数 | 值 | 说明 |
|------|-----|------|
| `SLEEP_TIMEOUT_MS` | 30分钟 | 进入睡眠的超时时间 |
| `SLEEP_PERIPHERAL_INTERVAL_US` | 50ms | 睡眠时的连接间隔 |
| `NORMAL_PERIPHERAL_INTERVAL_US` | 7.5ms | 正常工作时的连接间隔 |
| 睡眠延迟 | 800 | 睡眠时的连接延迟 |
| 正常延迟 | 400 | 正常工作时的连接延迟 |

## 注意事项

1. **功能开关**：睡眠功能仅在 `feature = "split"` 和 `feature = "_ble"` 同时启用时可用
2. **活动检测**：包括Central本地按键和从Peripheral接收到的按键事件
3. **连接保持**：睡眠状态下仍然保持与主机的BLE连接
4. **参数调整**：连接参数的调整是渐进的，避免连接中断

这个实现提供了一个平衡的解决方案，在保持响应性的同时显著降低了功耗。