# BLE Split Central 睡眠功能实现

## 功能概述

为BLE Split Central实现了基于按键超时的睡眠功能，除了现有的广播超时睡眠外，新增以下特性：

1. **按键超时睡眠**：在连接到主机后，如果指定时间（默认30分钟）没有任何按键活动（包括从Peripheral传来的按键），Central进入睡眠状态
2. **智能睡眠状态连接管理**：保留Central对主机的连接，根据连接状态动态调整与Peripheral的通信频率来节省电量
3. **按键唤醒**：按下Central或Peripheral的任意按键，立即退出睡眠状态，恢复正常连接参数
4. **可重复睡眠**：唤醒后如果再次超时没有按键活动，可以再次进入睡眠
5. **完全可配置**：所有睡眠参数都可以在`keyboard.toml`中配置

## 实现细节

### 新增的配置参数

在`keyboard.toml`的`[rmk]`部分可以配置以下睡眠参数：

```toml
[rmk]
# BLE Split Central 睡眠超时时间（分钟），0 = 禁用睡眠功能
split_central_sleep_timeout_minutes = 30
# 连接到主机时的睡眠连接间隔（微秒），确保快速打字不受影响
split_central_sleep_connected_interval_us = 15000  # 15ms
# 广播状态下的睡眠连接间隔（微秒），节省更多电量
split_central_sleep_advertising_interval_us = 200000  # 200ms
# 正常工作时的连接间隔（微秒）
split_central_normal_interval_us = 7500  # 7.5ms
```

### 核心数据结构

```rust
/// 睡眠配置结构
#[derive(Debug, Clone, Copy)]
pub struct SleepConfig {
    pub timeout_minutes: u32,           // 睡眠超时时间（分钟）
    pub connected_interval_us: u32,     // 连接状态下的睡眠间隔
    pub advertising_interval_us: u32,   // 广播状态下的睡眠间隔
    pub normal_interval_us: u32,        // 正常工作间隔
}

/// 睡眠状态枚举
enum SleepState {
    Awake,      // 清醒状态
    Sleeping,   // 睡眠状态
}
```

### 新增的信号管理

```rust
// 睡眠状态信号
pub(crate) static CENTRAL_SLEEP: Signal<crate::RawMutex, bool> = Signal::new();
// 活动唤醒信号（事件驱动）
pub(crate) static ACTIVITY_WAKEUP: Signal<crate::RawMutex, ()> = Signal::new();
```

### 核心功能函数

#### 1. 事件驱动的活动检测
```rust
/// 更新活动时间以指示用户活动
/// 这个函数触发活动唤醒信号用于睡眠管理
pub(crate) fn update_activity_time() {
    ACTIVITY_WAKEUP.signal(());
    debug!("Activity detected, signaling wakeup");
}
```

#### 2. 事件驱动的睡眠管理任务
`sleep_manager_task` 函数特性：
- **零轮询设计**：完全基于事件驱动，没有定期轮询
- **按需超时**：只在需要时设置定时器，节省CPU资源
- **智能连接参数**：根据连接状态动态选择睡眠间隔
- **可配置超时**：支持通过配置禁用睡眠功能（设为0）

#### 3. 智能连接参数调整
`adjust_peripheral_connection_params` 函数根据连接状态选择合适的睡眠参数：
- **连接状态下**：使用较短的睡眠间隔（默认15ms），确保突然的快速打字不受影响
- **广播状态下**：使用较长的睡眠间隔（默认200ms），最大化节能效果

### 集成点

#### 1. 在BLE Split Central驱动中检测按键活动
在 `BleSplitCentralDriver::read()` 方法中：
- 当接收到 `SplitMessage::Key` 或 `SplitMessage::Event` 时
- 调用 `update_activity_time()` 触发唤醒信号

#### 2. 在键盘处理中检测本地按键活动
在 `Keyboard::process_inner()` 方法中：
- 每次处理按键事件时调用 `update_activity_time()`
- 确保Central本地的按键也能重置睡眠计时器

#### 3. 配置驱动的初始化
在 `run_peripheral_manager()` 函数中：
- 从`rmk_config`读取睡眠配置参数
- 创建`SleepConfig`实例并传递给睡眠管理任务

## 工作流程

### 事件驱动的睡眠流程
1. Central连接到主机和Peripheral，睡眠管理器启动
2. 系统等待第一个按键活动或超时
3. 如果超时到达（默认30分钟无活动）：
   - 进入睡眠状态，设置睡眠标志
   - 检查连接状态，选择合适的睡眠间隔：
     - 连接状态：15ms间隔，适合快速响应
     - 广播状态：200ms间隔，最大化节能
   - 调整Peripheral连接参数

### 即时唤醒流程
1. 检测到任何按键活动（Central或Peripheral）
2. 立即触发唤醒信号，无延迟
3. 退出睡眠状态：
   - 清除睡眠标志
   - 恢复正常连接参数（7.5ms间隔）
   - 重新开始超时计时

## 电量优化策略

### 连接状态下的优化
- **适中节能**：15ms连接间隔，在节能和响应性之间取得平衡
- **快速响应**：确保用户突然开始快速打字时不受影响
- **保持连接**：维持与主机的连接，避免重新配对

### 广播状态下的优化
- **最大节能**：200ms连接间隔，显著降低功耗
- **可接受延迟**：广播状态下用户期望稍长的响应时间
- **智能切换**：根据CONNECTION_STATE自动选择优化策略

### 事件驱动优势
- **零CPU浪费**：无定期轮询，CPU在无活动时完全休眠
- **即时响应**：按键触发立即唤醒，无轮询延迟
- **内存优化**：去除时间戳存储，减少RAM占用

## 配置参数说明

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `split_central_sleep_timeout_minutes` | 30 | 睡眠超时（分钟），设为0禁用 |
| `split_central_sleep_connected_interval_us` | 15000 | 连接状态睡眠间隔（15ms） |
| `split_central_sleep_advertising_interval_us` | 200000 | 广播状态睡眠间隔（200ms） |
| `split_central_normal_interval_us` | 7500 | 正常工作间隔（7.5ms） |

## 配置示例

```toml
# keyboard.toml
[rmk]
# 睡眠相关配置
split_central_sleep_timeout_minutes = 45       # 45分钟后睡眠
split_central_sleep_connected_interval_us = 10000   # 连接时10ms间隔
split_central_sleep_advertising_interval_us = 300000 # 广播时300ms间隔
split_central_normal_interval_us = 7500         # 正常7.5ms间隔

# 禁用睡眠功能
# split_central_sleep_timeout_minutes = 0
```

## 优势总结

1. **事件驱动架构**：零轮询，最大化CPU效率
2. **智能功耗管理**：根据连接状态动态调整策略
3. **即时响应**：按键唤醒无延迟
4. **完全可配置**：用户可根据需求调整所有参数
5. **内存优化**：减少RAM占用，适合嵌入式环境
6. **渐进式节能**：在保持响应性的前提下最大化电池寿命

这个实现提供了一个高度优化和用户友好的睡眠管理解决方案，在保持优秀响应性的同时显著延长电池寿命。