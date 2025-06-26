# BLE Split Central Sleep Feature Implementation

## Feature Overview

Implemented key-timeout-based sleep functionality for BLE Split Central, adding the following features beyond the existing advertising timeout sleep:

1. **Key Timeout Sleep**: After connecting to the host, if there is no key activity (including keys from Peripheral) for a specified time (default 30 minutes), Central enters sleep state
2. **Smart Sleep State Connection Management**: Maintains Central's connection to the host while dynamically adjusting communication frequency with Peripheral to save power based on connection state
3. **Key Wake-up**: Pressing any key on Central or Peripheral immediately exits sleep state and restores normal connection parameters
4. **Repeatable Sleep**: After waking up, if there is no key activity again for the timeout period, it can enter sleep again
5. **Fully Configurable**: All sleep parameters can be configured in `keyboard.toml` and compiled into constants

## Implementation Details

### New Configuration Parameters

The following sleep parameters can be configured in the `[rmk]` section of `keyboard.toml`:

```toml
[rmk]
# BLE Split Central sleep timeout in minutes, 0 = disable sleep feature
split_central_sleep_timeout_minutes = 30
# Sleep connection interval when connected to host (microseconds), ensures rapid typing is not affected
split_central_sleep_connected_interval_us = 15000  # 15ms
# Sleep connection interval when advertising (microseconds), saves more power
split_central_sleep_advertising_interval_us = 200000  # 200ms
# Normal working connection interval (microseconds)
split_central_normal_interval_us = 7500  # 7.5ms
```

### Compile-time Constant Generation

Configuration parameters are converted to compile-time constants during build via `build.rs`:

```rust
// Constants generated in build.rs
pub(crate) const SPLIT_CENTRAL_SLEEP_TIMEOUT_MINUTES: u32 = 30;
pub(crate) const SPLIT_CENTRAL_SLEEP_CONNECTED_INTERVAL_US: u32 = 15000;
pub(crate) const SPLIT_CENTRAL_SLEEP_ADVERTISING_INTERVAL_US: u32 = 200000;
pub(crate) const SPLIT_CENTRAL_NORMAL_INTERVAL_US: u32 = 7500;
```

### Core Data Structures

```rust
/// Sleep state enumeration
enum SleepState {
    Awake,      // Awake state
    Sleeping,   // Sleeping state
}
```

### New Signal Management

```rust
// Sleep state signal
pub(crate) static CENTRAL_SLEEP: Signal<crate::RawMutex, bool> = Signal::new();
// Activity wakeup signal (event-driven)
pub(crate) static ACTIVITY_WAKEUP: Signal<crate::RawMutex, ()> = Signal::new();
```

### Core Functions

#### 1. Event-driven Activity Detection
```rust
/// Update activity time to indicate user activity
/// This function triggers activity wakeup signal for sleep management
pub(crate) fn update_activity_time() {
    ACTIVITY_WAKEUP.signal(());
    debug!("Activity detected, signaling wakeup");
}
```

#### 2. Event-driven Sleep Management Task
`sleep_manager_task` function features:
- **Zero Polling Design**: Completely event-driven with no periodic polling
- **On-demand Timeout**: Only sets timers when needed, saving CPU resources
- **Smart Connection Parameters**: Dynamically selects sleep intervals based on connection state
- **Configurable Timeout**: Supports disabling sleep feature via configuration (set to 0)
- **Compile-time Optimization**: Uses compile-time constants with zero runtime overhead

#### 3. Smart Connection Parameter Adjustment
`adjust_peripheral_connection_params` function selects appropriate sleep parameters based on connection state:
- **When Connected**: Uses shorter sleep interval (default 15ms) to ensure sudden rapid typing is not affected
- **When Advertising**: Uses longer sleep interval (default 200ms) to maximize power saving

### Integration Points

#### 1. Key Activity Detection in BLE Split Central Driver
In `BleSplitCentralDriver::read()` method:
- When receiving `SplitMessage::Key` or `SplitMessage::Event`
- Call `update_activity_time()` to trigger wakeup signal

#### 2. Local Key Activity Detection in Keyboard Processing
In `Keyboard::process_inner()` method:
- Call `update_activity_time()` when processing each key event
- Ensure Central's local keys can also reset the sleep timer

#### 3. Compile-time Constant Integration
In `rmk/build.rs`:
- Read configuration parameters from `keyboard.toml`
- Generate compile-time constants using `const_declaration!` macro
- Use these constants directly in code with no runtime overhead

## Workflow

### Event-driven Sleep Process
1. Generate sleep constants at compile time based on configuration
2. Central connects to host and Peripheral, sleep manager starts
3. System waits for first key activity or timeout
4. If timeout is reached (default 30 minutes of no activity):
   - Enter sleep state, set sleep flag
   - Check connection state, select appropriate sleep interval:
     - Connected state: 15ms interval, suitable for quick response
     - Advertising state: 200ms interval, maximize power saving
   - Adjust Peripheral connection parameters

### Instant Wake-up Process
1. Detect any key activity (Central or Peripheral)
2. Immediately trigger wakeup signal with no delay
3. Exit sleep state:
   - Clear sleep flag
   - Restore normal connection parameters (7.5ms interval)
   - Restart timeout timer

## Power Optimization Strategy

### Optimization When Connected
- **Moderate Power Saving**: 15ms connection interval balances power saving and responsiveness
- **Quick Response**: Ensures sudden rapid typing by user is not affected
- **Maintain Connection**: Keep connection to host, avoid re-pairing

### Optimization When Advertising
- **Maximum Power Saving**: 200ms connection interval significantly reduces power consumption
- **Acceptable Delay**: Users expect slightly longer response time when advertising
- **Smart Switching**: Automatically select optimization strategy based on CONNECTION_STATE

### Event-driven Advantages
- **Zero CPU Waste**: No periodic polling, CPU completely sleeps when no activity
- **Instant Response**: Key trigger immediately wakes up with no polling delay
- **Memory Optimization**: Remove timestamp storage, reduce RAM usage
- **Compile-time Optimization**: All configurations are compile-time constants with zero runtime overhead

## Configuration Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `split_central_sleep_timeout_minutes` | 30 | Sleep timeout (minutes), set to 0 to disable |
| `split_central_sleep_connected_interval_us` | 15000 | Connected state sleep interval (15ms) |
| `split_central_sleep_advertising_interval_us` | 200000 | Advertising state sleep interval (200ms) |
| `split_central_normal_interval_us` | 7500 | Normal working interval (7.5ms) |

## Configuration Examples

```toml
# keyboard.toml
[rmk]
# Sleep-related configuration
split_central_sleep_timeout_minutes = 45       # Sleep after 45 minutes
split_central_sleep_connected_interval_us = 10000   # 10ms interval when connected
split_central_sleep_advertising_interval_us = 300000 # 300ms interval when advertising
split_central_normal_interval_us = 7500         # Normal 7.5ms interval

# Disable sleep feature
# split_central_sleep_timeout_minutes = 0
```

## Implementation Advantages

### Compile-time Optimization
- **Zero Runtime Configuration Overhead**: All parameters are compile-time constants
- **Better Compiler Optimization**: Compiler can perform more aggressive optimizations
- **Reduced Binary Size**: No runtime configuration parsing code needed

### Architecture Design
1. **Event-driven Architecture**: Zero polling, maximize CPU efficiency
2. **Smart Power Management**: Dynamically adjust strategy based on connection state
3. **Instant Response**: Key wake-up with no delay
4. **Fully Configurable**: Users can adjust all parameters according to needs
5. **Memory Optimization**: Reduce RAM usage, suitable for embedded environments
6. **Progressive Power Saving**: Maximize battery life while maintaining responsiveness

This implementation provides a highly optimized and user-friendly sleep management solution that significantly extends battery life while maintaining excellent responsiveness through compile-time constants and event-driven architecture, with minimal runtime overhead.