# 三模 Dongle 技术方案

| | |
| --- | --- |
| **状态** | 设计评审 |
| **对应 roadmap** | Tri-mode dongle：在 USB、BLE 与 dongle 模式间切换 |
| **目标平台** | 首个目标 nRF52840；仅 RMK 对 RMK 链路 |
| **影响面** | `rmk`：新增 1 个模块（4 文件）+ 改动约 10 个文件 · `rmk-types`：改动 5 个文件 + 新增 1 个 · `rmk-config`：1 个文件 · `examples`：新增 1 个 |
| **键盘固件影响** | 必需——键盘须为版本匹配的 `rynk` BLE 固件；dongle 槽位、键码与 dongle-seeking 广播随 `rynk` + BLE 自动编入（§4.3） |
| English | [Tri-mode Dongle — Technical Design](./dongle_mode_design.md) |

## 1. 背景与目标

### 1.1 问题

无线键盘够不到没有蓝牙的机器——BIOS 界面、KVM、各种主机、没有射频的台式机。解法是一个 USB dongle。RMK 目前没有一个键盘可以**切换过去**的 dongle。

RMK 确实有 `examples/*/nrf52840_ble_split_dongle`，但那是另一种设备：dongle 作为分体 central 并持有键位表，两个半边跑的是 `run_rmk_split_peripheral`（`rmk/src/split/peripheral.rs:36`）——一个没有键位表也没有 HOGP 服务的入口。分体 peripheral 不可能同时又是一把独立的 BLE 键盘，因此根本不存在"在两者之间切换"。本文把那个称为 **split dongle**；它不受影响，两者并存。

### 1.2 目标

1. 键盘经由 dongle 连到主机，键位表与存储留在键盘上。
2. 用户在运行时、在键盘上切换 USB / BLE / dongle，无需重新烧录。
3. **键位配置器（Rynk/Vial）能经由 dongle 访问键盘。**
4. 配对dongle和键盘无需上位机、无需 dongle 上的按键、无需配对码，且是安全的。
5. Dongle内部只保存已经配对的键盘的信息
6. Rynk需要支持基于一个Dongle的多键盘设置（Vial可不支持）

### 1.3 非目标

- 支持未改动的键盘固件。键盘需要 dongle 槽位、一个键码和一种专用广播（§4.3）；dongle 与键盘必须是匹配的 RMK 版本。
- 非 RMK 键盘（通用 HOGP 的 report descriptor 解析）。
- 在延迟上胜过 BLE 直连。Dongle 多一个 USB 跳，这是接受的代价。

## 2. 行为

### 2.1 用户看到的是什么

一个没有任何操作部件的 USB 设备。插上后主机枚举出一个键盘、一个指针设备和一个串口。

### 2.2 三条数据通路

Dongle 承载三条相互独立的数据流，而这三条都是功能可用的必要条件：

| 通路 | 方向 | 载体 |
| --- | --- | --- |
| 按键与指针 | 键盘 → 主机 | 键盘 HID 服务（`0x1812`）的 notify → USB HID |
| 锁定灯 | 主机 → 键盘 | USB HID 输出报告 → 键盘 HID 输出特征 |
| 配置 | 主机 ↔ 键盘 | USB CDC ↔ 键盘 Rynk 服务（`10900067-…`） |

键盘在 dongle 模式下由 dongle 中继连接主机的配置器。

### 2.3 生命周期

**通常 dongle 只连接已经配对过的键盘。** 它只在两种情况下绑定新设备：

- **刚插上。** 上电后的 `dongle_pairing_window_secs` 内，接受一台近场的、正在寻找 dongle 的键盘。插 dongle 本来就是用户要做的动作，因此首次配对不多付出任何一步。
- **已配对的键盘授权了。** 一把已经与 dongle 绑定的键盘，可以通过加密链路让它打开配对窗口。因此加第二把键盘既不用拔插，也不用上位机。

于是重新配对只有两种形态，都不需要软件：

| 情况 | 用户怎么做 |
| --- | --- |
| 键盘未连接 dongle | 在键盘上长按 dongle 键，然后插拔 dongle |
| 键盘已连接 dongle | 在键盘上长按 dongle 键——键盘经由活动链路授权 dongle |

在这些窗口之外，dongle 从不绑定。

**日常运行。** Dongle 被动扫描，看到已绑定键盘广播就连上。没在用的键盘处于休眠、不广播，因此不占任何成本；拿起来，第一个按键落下之前它已经连上了。若所有链路都占满，下一把键盘就等着。

**键盘休眠时。** RMK 键盘在 300 秒无连接后停止广播并等待按键（`rmk/src/ble/mod.rs:214`）。在键盘休眠时插入 dongle，不按键则毫无反应。

**换一把键盘。** 流程同上表第一行，而且顺序不能反：先在新键盘上长按 dongle 键，*再*插拔 dongle。两者之间还没有连接，dongle 只能靠上电窗口发现这把键盘——键盘还没开始广播就先把窗口开掉，等于白白耗掉它。上电窗口不管槽位占用情况都会打开，因此不必先删掉旧绑定；而"已配对键盘授权"恰恰在唯一那把键盘丢了的时候用不上。旧槽位不用管——等到需要腾位置时它自会被淘汰。

### 2.4 多把键盘

Dongle 记住至多 `DONGLE_SLOTS_NUM` 把键盘，同时保持 `DONGLE_LINKS_NUM` 把连接。这是两个独立上限。

**HID 合并。** 无论连着几把，主机始终只看到一个键盘和一个指针设备——修饰键与键码取并集，指针增量直通。

**配置不合并，也从不广播。** 每把键盘拥有自己的键位表。只配了一把时不存在歧义，dongle 直接指向它，配置器用起来和直连完全一样；配了多把时，区分使用的协议：对于Rynk协议来说，需要在协议层面增加通过Rynk配置多把键盘的适配，上位机在配置的时候必须指明它指的是哪一把，未指明的配置帧一律被拒绝，而不是默认落到第一个槽位；Dongle不支持Vial。

**一把键盘同时只属于一个 dongle。** 这层关系是不对称的：dongle 能记住多把键盘，而键盘只有一个 dongle 槽位，因此要换到另一个 dongle，必须在键盘上重新配对。

### 2.5 行为细则

**连接**

| 情况 | 行为 |
| --- | --- |
| 键盘切到了普通蓝牙 profile | Dongle 就看不到它，也不会去打扰它。切回 dongle 模式即自动重连 |
| 键盘走远、没电或被关掉 | 它不再广播，dongle 也就没有可连的对象。Dongle 的扫描从不停止，键盘一恢复广播就立刻重连 |
| 某把键盘清掉了自己这边的 dongle 绑定 | 它转而开始请求配对。Dongle 不会自己去连它——插拔 dongle 打开配对窗口，才会重新配上 |
| 附近有一把键盘绑的是别的 dongle | 它的广播表明自己已有归属，本 dongle 直接忽略，绝不去打扰 |
| 键盘改绑到了别的 dongle，而本 dongle 还留着它的记录 | 连接会在加密这一步失败，本 dongle 随即清除该槽位。对端已经不再持有我们的密钥，这条记录永远不可能再用；留着只会一次次去拨一把早已另择他主的键盘 |
| 键盘固件版本与 dongle 不匹配 | 拒绝这把键盘，并且在重新配对之前不再重试 |
| 拔掉 dongle 再插回 | 已配对的键盘不会丢，上电后自行重连 |

**配对**

| 情况 | 行为 |
| --- | --- |
| 附近同时有两把键盘在找 dongle | 取信号最强的那把——通常就是你手上刚按过键的那把 |
| 配对完成后，dongle 发现固件版本不匹配 | 配对失败，槽位仍然空着 |
| Dongle 记住的键盘数已达上限 | 淘汰最久没有连接过的那把，腾出位置——配对不会因此卡住 |
| 配对没能在窗口内完成 | 放弃并恢复正常重连；重来一次即可 |

**打字时**

| 情况 | 行为 |
| --- | --- |
| 键盘中途断开（走远、没电） | 它按住的键在电脑上被释放，不会卡键。其他键盘照常工作 |
| 电脑还没枚举完 dongle | 按键被丢弃 |

**配置时**

| 情况 | 行为 |
| --- | --- |
| 工具还没指明要配哪把键盘，而 dongle 只配了一把 | 直接用那一把。没有第二把键盘可误改，也就无从歧义——配置器不必知道 dongle 的存在 |
| 工具还没指明要配哪把键盘，而 dongle 配了多把 | 拒绝，并回一个明确的错误说明必须先指定目标。绝不默认落到第一把，以免误改别的键盘 |
| 配置过程中目标键盘断开 | 会话终止并通知工具；绝不静默切到另一把键盘上 |
| 配置工具断开 | 不影响打字 |

## 3. 架构

### 3.1 Dongle 身兼三个角色

Dongle 是键盘两个 GATT 服务的客户端，同时是 USB 上的服务端：

```
        ┌──────────────────────── dongle ────────────────────────┐
        │                                                        │
主机 ◄──┤ USB HID     ◄── 合并 ◄── HID 客户端  (0x1812)        ├──► 键盘
        │                                                        │
主机 ──►┤ USB HID out ──────────► HID 客户端写                  ├──► 键盘
        │                                                        │
主机 ◄─►┤ Rynk 服务端(CDC) ◄──► 路由 ◄──► Rynk 客户端           ├◄─► 键盘
        │        │                          (10900067-…)         │
        │        └── 自有命令：槽位、配对、目标                   │
        └────────────────────────────────────────────────────────┘
```

1. **HID-over-GATT 客户端**（每条链路一个）——发现键盘的 HID 服务，订阅输入报告，回写 LED 状态。
2. **Rynk GATT 客户端**（目标链路上）——把请求分块写进 `output_data`，订阅 `input_data` 的 notify。
3. **Rynk 服务端**（USB CDC 上）——终结上位机的会话，自己应答 dongle 专属命令，其余交给角色 2。

角色 3 无法复用 `run_host_usb`（`rmk/src/usb/rynk.rs:35`）驱动的 `RynkService::run_session`——后者持有键位表和一张背靠存储的命令表，dongle 两样都没有。但 `run_host_usb` 本身只是"等连接、建 rx/tx、进会话"的骨架，把 service 参数换成 dongle 的路由器（§4.7）即可原样复用；新代码只有会话内部：切帧、看命令、要么自己答要么转发。

Dongle 模式在键盘看来不是第四种传输，而是指向另一个主机的 BLE 模式。在链路层面 dongle 就是一台电脑，因此 `ConnectionType`、`decide_active()`、report channel 和报告写入路径全部原样不动。唯一的区别是它占用一个**专用槽位**而不是普通 profile（§4.3）——正因如此，配对 dongle 碰不到用户与电脑之间的绑定。

| 键盘模式 | 如何选择 |
| --- | --- |
| USB | `User(NUM_BLE_PROFILE + 3)`——切换首选传输 |
| BLE | 该切换键 + `User0`..`User(N-1)` |
| Dongle | 该切换键 + `SwitchToDongle`（§4.3） |

USB/BLE 切换键与 profile 键码本来就存在（`rmk/src/keyboard.rs:1706-1725`）；三模为 dongle 槽位新增一个键码。

### 3.2 关键决策

| # | 决策 | 被否的替代方案 | 理由 |
| --- | --- | --- | --- |
| D1 | 做中继——键位表留在键盘 | 像 split dongle 那样做分体 central | 只有这种形态键盘才仍是一把完整的键盘，而这正是"可切换"的前提 |
| D2 | 把所有键盘合并成一个 USB HID 设备 | 按链路各暴露一套 HID 接口 | `embassy-usb` 以 `max-interface-count-8` 构建，而 Rynk 已占 4 个，按链路分配到第 3 条链路就耗尽 |
| D3 | Rynk 走自定义 GATT 服务（`10900067-…`） | 走厂商 HID 服务 `RynkHidService` | HID 那条路是为只能碰到 HOGP 的浏览器准备的；原生 central 没这个限制，而且自定义特征按 MTU 分块，不是固定 32 字节报告 |
| D4 | 保留 `0x09xx` 段给 dongle，其余帧按原始字节直通 | 用封装命令包住键盘帧 | 键盘固件、dongle 固件、上位机三端都是 RMK 自己的，保留号段只是一行约定，不存在兼容风险。封装则要付出上位机双重编码、dongle 拆包再编码、外加一个整帧缓冲——用来解决一个不会发生的冲突 |
| D5 | 键盘侧支持随 `rynk` + `_ble` 直接编入，不设独立 feature | 独立的 `dongle_mode` feature（初版实现即如此） | 实测成本仅 ~150B RAM + ~2KB flash（+1 bond 槽、1 字节特征、广播分支），而收益是任何版本匹配的 rynk BLE 键盘无需重编固件即可配 dongle；两个几乎同名的 feature（`dongle`/`dongle_mode`）也容易混淆。`dongle` feature 只属于中继固件本体（§4.2） |
| D6 | 键盘→dongle 的配对授权走 Rynk GATT 服务上的独立特征 | 复用 `input_data` 塞一个 topic 帧 | `input_data` 是被 dongle 原样直通给上位机的配置流（D4）；混入带内消息就要求 dongle 拆帧检查每个 notify，直通就没了。一个 1 字节的独立特征把控制面和数据面分开 |
| D7 | Dongle 槽位淘汰按持久化的 last-seen 逻辑序号 | RAM 里记 LRU；按挂钟时间戳；按累计连接时长 | 配对恰好发生在刚上电的窗口里（§2.3），RAM 状态在最需要它的时刻永远是空的。Dongle 没有挂钟，时间戳不可得；累计时长刻画的是 frequency 而淘汰要的是 recency——曾重度使用但半年没碰的键盘恰恰应该被淘汰。逻辑序号只表先后，正好够用（§4.8） |

## 4. 详细设计

### 4.1 复用地图

先立规矩：**dongle 不新造任何一层已有机制。** 下表把 dongle 需要的每项能力对到现有代码，"同构新写"指照抄现有模式换掉服务/特征常量。

| 能力 | 现有代码 | 复用方式 |
| --- | --- | --- |
| BLE 栈构建 | `trouble_host::new`（同 `BleTransport::run` 的自持栈模式） | 同构：`Dongle::run` 内部自建，按 `DONGLE_LINKS_NUM` 定径 |
| central 连接/扫描互斥 | split central 的 `SCANNING_MUTEX` / `STACK_STARTED` / `wait_for_stack_started`（`rmk/src/split/ble/central.rs:23-29`） | 原样搬用同一组原语 |
| 广播识别 | `ScanHandler::on_adv_reports`（`rmk/src/split/ble/central.rs:130`） | 同构新写 `DongleScanHandler`（匹配 §4.5 的 seeking 广播，记录 RSSI） |
| GATT 服务发现 / 订阅 / 回写 | `run_ble_peripheral_manager` + `BleSplitCentralDriver`（`rmk/src/split/ble/central.rs`） | 同构新写：换成 HID 服务 + Rynk 服务的特征表 |
| 连接参数 / PHY | `default_central_conn_param`、`update_conn_params`、`update_ble_phy` | 原样 |
| USB HID 设备与报告写出 | `UsbTransport` + `USB_REPORT_CHANNEL` + `UsbKeyboardWriter`（`rmk/src/usb/mod.rs:217`） | 原样；dongle 只是一个新的报告生产者 |
| USB 未枚举时丢报告 | `send_hid_report`（`rmk/src/channel.rs:53`）按 `active_transport()` 丢弃 | 原样，自动满足 §2.5"枚举前按键被丢弃" |
| LED 状态回读 | `UsbLedReader` → `run_led_reader` → `LedIndicatorEvent`（`rmk/src/hid.rs:397`） | 原样；dongle 订阅事件后写向各链路 |
| CDC-ACM 接口 | `build_host_usb`（`rmk/src/usb/rynk.rs:21`） | 原样；feature gate 从 `rynk` 放宽为 `any(rynk, dongle)` |
| CDC 会话骨架 | `run_host_usb`（`rmk/src/usb/rynk.rs:35`) | 原样；经 `RynkUsbService` trait 同时接受 `RynkService` 与 `DongleRouter` |
| Rynk 切帧 / 编帧 | `Deframer`、`encode_frame`、`RynkHeader`（`rmk-types/src/protocol/rynk/`） | 原样 |
| bond 持久化 | `Storage` + `StorageKey::BondInfo(slot)` + `FLASH_CHANNEL`（`rmk/src/storage/mod.rs`） | 原样；槽位语义变为 dongle 槽 |
| 无键位表的存储构造 | `new_storage_for_split_peripheral`（`rmk/src/storage/mod.rs:349`） | 原样复用这一先例 |
| 键盘侧连接服务 | `run_ble_keyboard` 全套（HID notify、Rynk 会话、电池、休眠） | 原样——dongle 链路上键盘的行为与连电脑完全一致 |

### 4.2 Dongle 固件结构

本方案只引入一个 feature，属于中继固件本体：

| feature | 谁启用 | 作用 |
| --- | --- | --- |
| `dongle` | dongle 固件（中继设备本体） | 编入 `rmk/src/dongle/` 模块；与 `rynk`/`vial` 正交——同一 crate 可同时编译键盘 bin 与 dongle bin，dongle bin 通过 `with_dongle_router` 挂载路由器 |

键盘侧不设 feature：dongle 槽位、`SwitchToDongle` 键码、专用广播与授权特征随 `rynk` + BLE 芯片 feature 自动编入（D5）。

新增模块：

```
rmk/src/dongle/
  mod.rs      Dongle（Runnable）：任务编排、槽位表、配对窗口管理
  link.rs     单链路任务：连接 → 加密 → 发现 → 握手 → 转发
  merge.rs    HID 合并器（纯函数 + 每链路快照，可单测）
  router.rs   DongleRouter：CDC 会话（0x09xx 自答 + 直通）
```

任务与数据流全景：

```
┌────────────────────────────── dongle 固件 ──────────────────────────────┐
│                                                                         │
│  ble_task(runner)                          trouble 栈驱动，常驻          │
│                                                                         │
│  pairing_manager        上电窗口 / 授权窗口 / 扫描选优 / 槽位淘汰        │
│        │ 新绑定                                                          │
│        ▼                                                                │
│  ┌─ 槽位表 (RAM + flash) ─┐                                             │
│  └──────────┬─────────────┘                                             │
│             │ 认领                                                       │
│  link_task ×DONGLE_LINKS_NUM                                            │
│   │  HID notify ──► merge ──► send_hid_report ──► USB_REPORT_CHANNEL ─┐ │
│   │  rynk notify ──► (仅 target) 整帧重组 ──► CDC 写 ◄────────────┐   │ │
│   │  ◄── LED 写 ◄── LedIndicatorEvent ◄──────────────────────┐   │   │ │
│   │  ◄── rynk 写 ◄── DongleRouter 转发 ◄──────────────┐      │   │   │ │
│   ▼                                                   │      │   │   │ │
│  UsbTransport::run（原样复用）                         │      │   │   │ │
│    ├─ UsbKeyboardWriter        ◄──────────────────────┼──────┼───┼───┘ │
│    ├─ UsbLedReader → run_led_reader ──────────────────┼──────┘   │     │
│    └─ run_host_usb(CDC) → DongleRouter::run_session ──┴──────────┘     │
│                             │ 0x09xx 自答                               │
│  storage_task               槽位 bond + meta 持久化                     │
└─────────────────────────────────────────────────────────────────────────┘
```

Dongle 的 USB 侧就是一台普通 RMK USB 键盘的 USB 侧：`UsbTransport` 原样运行，HID 报告从 `USB_REPORT_CHANNEL` 里来，LED 状态发布成 `LedIndicatorEvent`，CDC 会话由 `run_host_usb` 驱动。差别只在生产者与消费者：报告由链路转发任务生产，LED 事件由链路转发任务消费，CDC 会话跑的是 `DongleRouter` 而不是 `RynkService`。

用户侧入口（`examples/use_rust/nrf52840_dongle`）：

```rust
let (storage, dongle_slots) = new_storage_for_dongle(flash, storage_config).await;
let dongle = Dongle::new(sdc, ble_addr(), dongle_slots);
let usb = UsbTransport::new(driver, device_config)
    .with_dongle_router(dongle.router());
run_all!(usb, dongle, storage)
```

SDC 按 central-only 构建：`support_central` + `central_count(DONGLE_LINKS_NUM)`，无 peripheral 角色。`Dongle::run` 内部自建并按 `DONGLE_LINKS_NUM` 定径自己的 BLE stack（含 `HostResources`，用户不接触），与键盘 bin 的 stack 尺寸互不影响。

### 4.3 键盘侧改动

键盘侧的一切改动都编在 `rynk` + `_ble` 之下（D5），随任何 rynk BLE 键盘固件自动可用；`vial` 键盘不含这些改动（dongle 链路依赖 Rynk GATT 服务与版本门）。

键盘侧的原则：**dongle 槽位就是"第 N+1 个 profile"**，一切既有机制照走。

**槽位。** `DONGLE_PROFILE: u8 = NUM_BLE_PROFILE as u8`——普通 profile 编号空间之后的专用值。`ProfileManager`（`rmk/src/ble/profile.rs`）的 `bonded_devices` 容量 +1，bond 落在既有存储键 `StorageKey::BondInfo(NUM_BLE_PROFILE)`。`set_ble_profile` / `BleStatus.profile` / `BleProfileAction::Switch` 天然携带这个值，无需新通道。

**键码。** `SwitchToDongle = User(NUM_BLE_PROFILE + 5)`，紧随现有五个固定动作（`rmk/src/keyboard.rs:1706-1725`）之后：

| 动作 | 行为 |
| --- | --- |
| 短按（释放时） | `BleProfileAction::Switch(DONGLE_PROFILE)`——与普通 profile 切换完全同一条路 |
| 长按 5 s，且 dongle 链路已连接 | 触发 `DONGLE_AUTH_SIGNAL` → 键盘经加密链路 notify dongle 打开配对窗口（授权加第二把键盘） |
| 长按 5 s，其他情况 | `ClearSlot(DONGLE_PROFILE)` + `Switch(DONGLE_PROFILE)`——清掉本机的 dongle 绑定并转入 seeking |

长按检测复用 clear split peer 的 5 秒 hold 模式（`rmk/src/keyboard.rs:1679-1704`）。

**广播。** `advertise()`（`rmk/src/ble/mod.rs:544`）按当前槽位分支：

```
advertise()
 └─ current_profile() == DONGLE_PROFILE ?
     ├─ 否 → 现行 HID 广播（可发现，完全不变）
     └─ 是 ├─ 槽位有 bond → 定向广播到 dongle 地址
           │                （ConnectableNonscannableDirected，
           │                  复用 split peripheral 先例 rmk/src/split/ble/peripheral.rs:236）
           └─ 无 bond     → dongle-seeking 广播：
                             Flags = 仅 BR_EDR_NOT_SUPPORTED（不可发现）
                             ManufacturerSpecificData {
                               company_identifier: 0xe118,        // split 已用的先例
                               payload: [0xD0, RYNK_PROTO_MAJOR], // kind + 协议大版本
                             }
```

两种 dongle 广播都是**不可发现**的：手机和电脑的"添加设备"列表永远看不到它们，普通主机也不会来连。定向广播天然满足 §2.5 的两条：绑了别的 dongle 的键盘只对它的 dongle 定向广播，本 dongle 根本收不到；本 dongle 收到的定向广播必然是冲自己来的。seeking 的 kind 字节 `0xD0` 与 split peripheral 的 payload（首字节是 peripheral id，小整数）错开，且 seeking 广播不带 split 服务 UUID，两个扫描器互不误判。300 秒广播超时与睡眠唤醒逻辑（`rmk/src/ble/mod.rs:214`）原样生效。

**授权特征。** `RynkGattService`（`rmk/src/ble/ble_server.rs:57`）新增一个 1 字节特征：

```rust
#[characteristic(uuid = RYNK_DONGLE_CTRL_CHAR_UUID, notify, permissions(encrypted))]
pub(crate) dongle_ctrl: u8,   // 0x01 = OpenPairingWindow
```

`run_ble_keyboard` 挂一个小任务：等 `DONGLE_AUTH_SIGNAL`，若当前连接在 dongle 槽位上则 notify `0x01`。电脑作为对端时不订阅这个特征，零影响（D6）。

键盘侧改动到此为止——连接建立后跑的 `run_ble_keyboard`、HID notify、Rynk 会话、电池、休眠全部与连电脑时同一份代码。

### 4.4 链路管理

Dongle 侧的核心状态是**槽位表**：

```rust
struct DongleSlot {
    bond: BondInformation,          // trouble-host 的 bond（addr/IRK/LTK）
    name: heapless::String<32>,     // 握手时从 GetDeviceInfo 取，落盘
    last_seen: u32,                 // 全局单调逻辑序号，非时间戳；见 §4.8（D7）
    connected: Option<LinkId>,      // RAM
    version_bad: bool,              // RAM；重新配对或重启前不再重试（§2.5）
}
```

持久化复用 `Storage`：bond 存进既有的 `StorageKey::BondInfo(slot)`（dongle 固件里无键盘 profile，键空间无冲突）；`name` + `last_seen` 存进新增的 `StorageKey::DongleSlotMeta(slot)`。

每条链路一个 `link_task`，生命周期是一个五态循环：

```
      ┌──────┐  无可认领槽位：等槽位表变化
 ┌───►│ Idle │◄────────────────────────────────┐
 │    └──┬───┘                                 │
 │       │ 认领一个未连接、非 version_bad 的槽位  │
 │       ▼                                     │
 │    ┌─────────┐ 15s 超时 → 释放认领           │
 │    │ Connect │──────────────────────────────┤
 │    └──┬──────┘ filter_accept_list=[addr]    │
 │       ▼        （CONNECT_MUTEX 串行）        │
 │    ┌─────────┐ 加密失败 → 清除槽位（§2.5：   │
 │    │ Secure  │──对端已改绑，记录永不可用）───┤
 │    └──┬──────┘ request_security() 用 bond    │
 │       ▼                                     │
 │    ┌───────────┐ 版本不匹配 → version_bad ──┤
 │    │ Handshake │ GetVersion / GetDeviceInfo │
 │    └──┬────────┘ （经 Rynk 客户端）          │
 │       ▼                                     │
 │    ┌───────┐ 断链：清合并快照、重发合并报告   │
 │    │ Serve │────────────────────────────────┘
 │    └───────┘ 订阅转发 · LED 回写 · 配置直通
 └──── 进入与退出 Serve 各 bump 一次 last_seen（§4.8）
```

要点：

- **连接即扫描。** trouble 的 `central.connect` 带 `filter_accept_list` 本身就是"扫到即连"（split central 的用法，`rmk/src/split/ble/central.rs:190`）。已绑定键盘的重连不需要独立扫描任务；独立扫描只在配对窗口存在。多任务对 controller 的 initiate/scan 互斥沿用 split 的 `SCANNING_MUTEX` 模式。
- **认领制。** 槽位数可以大于链路数。`link_task(i)` 从槽位表认领"未被认领、未连接、非 version_bad"的下一个槽位（以 `i` 为起点轮转，避免两条链路争抢同一槽位），connect 超时即释放换下一个，让 `DONGLE_LINKS_NUM` 条链路公平轮询所有候选。
- **Serve 期间的四个转发方向**见 §4.6 / §4.7；`Handshake` 完成之前该链路不对配置直通开放，因此 dongle 自己的握手请求与上位机的直通帧天然串行，不存在 seq 混流。
- **连接参数**：复用 `default_central_conn_param()`（7.5 ms interval）+ 2M PHY——与 split 链路同款，这是打字延迟的主要保障。

### 4.5 配对

**窗口。** 两个来源，效果相同——打开 `dongle_pairing_window_secs` 的扫描窗：

1. dongle 上电（§2.3 的"刚插上"）；
2. 任一已连接键盘在 `dongle_ctrl` 特征上 notify `0x01`（§2.3 的"已配对键盘授权"）。

**扫描与选优。** 窗口内开被动扫描（`Scanner` + `DongleScanHandler`，同 split 的 START/STOP 信号模式），匹配 seeking 广播（`0xe118` + kind `0xD0` + `RYNK_PROTO_MAJOR` 相等），记录 `(addr, rssi)`。第一个候选出现后再聚合 2 秒取 RSSI 最强者——不必傻等整个窗口，配对体验是秒级的。

**槽位分配。** 有空槽用空槽；满员则淘汰 `last_seen` 最小且当前未连接的槽位（§2.5"腾出位置——配对不会因此卡住"）。

**时序：**

```
 键盘                                     dongle
  │ 长按 dongle 键：清本机 dongle bond      │ 上电，进入 dongle_pairing_window_secs 窗口
  │ 切到 dongle 槽位，发 seeking 广播        │ 被动扫描
  │──[0xe118 | 0xD0 | ver]───────────────►│ 记录 (addr, RSSI)，聚合 2s 取最强
  │◄————— connect ————————————————————————│
  │◄———— LESC Just Works 配对 ———————————►│ central 发起（request_security）
  │  PairingComplete → 存 bond 到 dongle 槽 │  PairingComplete → 拿到 bond
  │◄———— GATT 发现 + 订阅 ————————————————│ HID 服务 + Rynk 服务 + dongle_ctrl
  │◄———— Rynk GetVersion ————————————————►│ major 不等 → 断开，槽位不写
  │◄———— Rynk GetDeviceInfo ─————————————►│ 记下 name
  │                                        │ bond + meta 落盘，配对完成
  │════════ 加密链路，开始转发 ═════════════│
```

任何一步失败，槽位保持原状（§2.5"配对失败，槽位仍然空着"）。

**安全分析**（对应目标 4"无配对码且安全"）：

| 威胁 | 防线 |
| --- | --- |
| 被动窃听 | LESC（ECDH）配对，链路密钥不过空口 |
| 重连劫持 | 重连只走 bond 加密；没有 LTK 的冒充者在 `Secure` 一步即失败 |
| 配对窗口内的主动 MITM | Just Works 无法在协议层防御。缓解：窗口仅在上电 `dongle_pairing_window_secs` 秒内或已配对键盘显式授权时开放，且键盘必须同时处于用户长按触发的 seeking 态——攻击者需要在数秒窗口内、物理近场、实时中间人。残余风险与市售专有 2.4 GHz dongle（出厂配对方案）在同一量级 |
| 非 RMK 设备混入 | 配对流程内含版本门（`GetVersion`）；答不上 Rynk 的设备到不了落盘那一步 |

Dongle 在窗口之外从不发起对 accept list 以外地址的连接，也从不接受配对——"Dongle内部只保存已经配对的键盘的信息"（目标 5）由此成立。

### 4.6 打字通路

**订阅。** `Serve` 态对键盘 HID 服务（`rmk/src/ble/ble_server.rs:117` 定义的四个 input 特征）各建一个 `NotificationListener`：`input_keyboard [u8;8]`、`mouse_report [u8;5]`、`media_report [u8;2]`、`system_report [u8;1]`。这些就是 `BleCompositeReport` 的原始字节，布局固定，直接按字节解析回 `Report`。

**合并（`merge.rs`）。** 只有键盘报告需要真正合并；合并器维护每链路最后一帧键盘报告的快照：

| 报告 | 策略 |
| --- | --- |
| 键盘 | 修饰键按位 OR；键码取并集填入 6 槽，溢出丢弃（boot 布局上限，两把键盘合计按住 7 键以上的场景不值得为它引入 NKRO 描述符变更） |
| 指针 | 增量直通，无状态 |
| 媒体 / 系统 | 直通；断链时补发全零报告防"按住"残留 |

任一链路断开：清除该链路快照 → 重算合并 → 立即发一帧——§2.5 的"不卡键"。合并结果经 `send_hid_report` 入 `USB_REPORT_CHANNEL`，未枚举时该函数按现有语义丢弃（§2.5"枚举前按键被丢弃"），零新代码。

**LED 回程。** `UsbTransport` 里的 `run_led_reader` 收到主机 LED 输出报告后发布 `LedIndicatorEvent`（现有行为）；每条链路的 `link_task` 订阅该事件，把 1 字节 LED 位图 `write_without_response` 到键盘的 `output_keyboard` 特征。键盘侧收到即触发既有的 `LED_SIGNAL` 路径，无改动。

### 4.7 配置通路

**帧格式回顾。** Rynk 线上是 COBS 帧流，`0x00` 作帧界（`rmk-types/src/protocol/rynk/message.rs`）；键盘 BLE 侧的 `output_data` 写与 `input_data` notify 传的同样是这个字节流（`RYNK_BLE_RX_PIPE` 直接喂 `Deframer`）。所以"直通"就是字面意义的字节直通——dongle 不解包、不重编码。

**路由（`router.rs`，`DongleRouter::run_session(rx, tx)`）。** 形状与 `RynkService::run_session` 相同，因此 `run_host_usb` 原样驱动它：

```
主机 CDC 字节流
   │  按 0x00 切出一个编码帧（不解码整帧）
   ▼
decode_header —— 只 COBS 解码前几个字节，取出 3 字节头（rmk-types 新增 helper）
   │
   ├─ cmd ∈ 0x09xx ──► 整帧解码 → 本地 dispatch → 应答帧写 CDC
   │
   └─ 其他 ──► 有目标？
                ├─ 未定（多槽位已绑定）──► Err(NoTarget)，echo 原 seq
                ├─ 目标链路未连接 ────► Err(NotReady)
                └─ ok ──► 原始编码帧（含 0x00）按 ≤244 B 分块
                          write_without_response 到目标 output_data

键盘 input_data notify（原始字节，含回复与 0x8xxx topic）
   └─► 按 0x00 重组到整帧边界 ──► 写 CDC
        （与 0x09xx 应答共用一把帧级写锁，保证 CDC 流内帧不交错）
```

kb→host 方向必须重组到帧边界再写，否则 dongle 自答的 0x09xx 帧可能插进一个被 MTU 切开的键盘帧中间。缓冲一个 `RYNK_BUFFER_SIZE` 足够（协议本就以此为帧上限）。

**目标规则。**

| 状态 | 行为 |
| --- | --- |
| 恰好一个槽位有绑定 | 隐式 target，配置器无感（§2.5） |
| 多个槽位有绑定、未 SelectTarget | 一律 `Err(NoTarget)` |
| target 断链 | 清除 target，推送 `DongleSlotsChange` topic——工具据此终止会话，绝不静默换目标 |
| CDC 会话结束（`run_session` 返回） | target 重置为默认规则 |

**Dongle 命令段（`0x09xx` / topic `0x89xx`，rmk-types 命令表新增）：**

```rust
// Dongle (0x09xx) — dongle 自答，永不转发。
GetDongleInfo     = 0x0901: () => DongleInfo;      // 探测 + 上限 + dongle 自身版本
GetDongleSlots    = 0x0902: () => DongleSlots;     // 槽位表快照
SelectDongleTarget = 0x0903: u8 => ();             // 设定配置目标槽位
ForgetDongleSlot  = 0x0904: u8 => ();              // 删除一个绑定

// topic
DongleSlotsChange = 0x8901: DongleSlots;           // 槽位连接状态变化时推送
```

payload（新文件 `rmk-types/src/protocol/rynk/payload/dongle.rs`）：

```rust
/// wire 上的槽位数上限；dongle_slots_num 不得超过它。
pub const MAX_DONGLE_SLOTS: usize = 8;

pub struct DongleInfo {
    pub version: ProtocolVersion,     // dongle 自身的 Rynk 协议版本
    pub slots_num: u8,
    pub links_num: u8,
}

pub struct DongleSlot {
    pub slot: u8,
    pub connected: bool,
    pub name: heapless::String<32>,   // 握手时取自 GetDeviceInfo
}

pub struct DongleSlots {
    pub slots: heapless::Vec<DongleSlot, MAX_DONGLE_SLOTS>,
    pub target: Option<u8>,
}
```

**上位机流程（目标 6）：**

```
连上 CDC ── GetDongleInfo ──┬─ Ok(DongleInfo) ─► 是 dongle：GetDongleSlots
                            │                    └ 多槽 → UI 选择 → SelectDongleTarget
                            │                    此后一切照旧（既有配置流程直通目标键盘）
                            └─ Err(UnknownCmd) ─► 是键盘直连：既有流程，零改动
```

单键盘用户从头到尾不需要探测——直通让 dongle 完全透明（§2.5）。`GetVersion` 等一切既有命令由**目标键盘**应答，上位机拿到的能力表就是那把键盘的真实能力表。

### 4.8 存储

Dongle 固件复用整套 `Storage`（含 `FLASH_CHANNEL` 任务模型），构造走 `new_storage_for_split_peripheral` 的同款先例（无键位表的最小构造）。键空间：

| 键 | 内容 | 新旧 |
| --- | --- | --- |
| `StorageKey::BondInfo(slot)` | 槽位 bond（`ProfileInfo` 原样，`cccd_table` 闲置为空） | 复用 |
| `StorageKey::DongleSlotMeta(slot)` | `{ name: String<32>, last_seen: u32 }` | 新增（`cfg(dongle)`） |

**`last_seen` 是逻辑序号，不是时间。** Dongle 没有挂钟，"最久没有连接"无法按绝对时间记；但淘汰只需要各槽位之间的**先后全序**，Lamport 式计数就够了：每次 bump 取全表最大值 +1 写给该槽位。

Bump 两个时机——**连接建立时**和**正常断开时**（键盘走远、没电、切走 profile）。只在建立时 bump 是错的：一把连上后长期保持连接的键盘，序号会停留在很久以前，拔插 dongle 后它尚未重连的间隙里若发生满员配对，它会被误判为"最旧"而淘汰；断开一并 bump 后，序号刻画的是"最近一次使用的结束"，语义正确。Dongle 整体断电（拔出）来不及写 flash，但那种断开对所有连接同时发生，相对顺序不变，淘汰判断无偏。

写入频率 = 连接/断开事件的频率，flash 磨损可以忽略。

键盘侧无新键：dongle bond 落在 `BondInfo(NUM_BLE_PROFILE)`，即现有键的下一个槽位编号。

### 4.9 配置项

`keyboard.toml` 的 `[rmk]` 段新增三项，经现有 `rmk-types/build.rs` → `constants.rs` 机制变成编译期常量（`rmk-config/src/lib.rs` 加字段与默认值）：

| 配置 | 默认 | 说明 |
| --- | --- | --- |
| `dongle_slots_num` | 4 | 记住的键盘数上限（≤ `MAX_DONGLE_SLOTS` = 8） |
| `dongle_links_num` | 2 | 同时保持的连接数；决定 `CONNECTIONS_MAX` 与 SDC `central_count`，直接影响 RAM |
| `dongle_pairing_window_secs` | 30 | 上电 / 授权后配对窗口时长 |

## 5. 协议与线上格式变更汇总

评审时只需要看这一节就能确认所有跨端约定。

**广播（新，仅键盘 dongle 槽位使用）：**

| 场景 | 类型 | 内容 |
| --- | --- | --- |
| seeking（未绑定） | `ConnectableScannableUndirected`，不可发现 | `Flags(BR_EDR_NOT_SUPPORTED)` + `MSD{0xe118, [0xD0, RYNK_PROTO_MAJOR]}` |
| 已绑定重连 | `ConnectableNonscannableDirected` | 目标 = dongle 地址（复用 split 先例） |

**GATT（键盘侧，`RynkGattService` 内）：**

| 项 | 变更 |
| --- | --- |
| `dongle_ctrl` 特征 | 新增：`RYNK_DONGLE_CTRL_CHAR_UUID`，1 字节，notify，encrypted；`0x01` = OpenPairingWindow |

**Rynk 协议（rmk-types）：**

| 项 | 变更 | 兼容性 |
| --- | --- | --- |
| `0x0901–0x0904` + `0x8901` | 新增 dongle 命令段（§4.7） | minor bump：老键盘对 `0x09xx` 答 `UnknownCmd`，这正是探测流程的分支依据 |
| `RynkError::NoTarget` | 新变体 | 只可能出现在"新 dongle + 多键盘"对话中，此时上位机必然已支持 dongle；旧上位机接触不到 |
| `decode_header` helper | 新增：对 COBS 编码帧做前缀解码取 3 字节头，不破坏原帧 | 纯新增 |
| `RYNK_DONGLE_CTRL_CHAR_UUID` | 新 UUID 常量 | 纯新增 |

命令表的 snapshot 测试（`rmk-types/src/protocol/rynk/snapshots/`）自动把这些变更钉进 golden 文件。

## 6. 代码改动清单

| 位置 | 类型 | 内容 | 规模 |
| --- | --- | --- | --- |
| `rmk/src/dongle/mod.rs` | 新增 | `Dongle` runnable：槽位表、认领、配对窗口 | ~250 行 |
| `rmk/src/dongle/link.rs` | 新增 | 链路状态机：connect/secure/handshake/serve | ~300 行 |
| `rmk/src/dongle/merge.rs` | 新增 | HID 合并器 + 单元测试 | ~150 行 |
| `rmk/src/dongle/router.rs` | 新增 | `DongleRouter` CDC 会话 + 单元测试 | ~300 行 |
| `rmk/Cargo.toml` | 修改 | feature `dongle` | 数行 |
| `rmk/src/lib.rs` | 修改 | `pub mod dongle` | 数行 |
| `rmk/src/ble/mod.rs` | 修改 | `advertise()` dongle 分支；`CONNECTIONS_MAX`；auth notify 任务 | ~60 行 |
| `rmk/src/ble/profile.rs` | 修改 | `DONGLE_PROFILE`、槽位容量 +1 | ~15 行 |
| `rmk/src/ble/ble_server.rs` | 修改 | `dongle_ctrl` 特征 | ~5 行 |
| `rmk/src/keyboard.rs` | 修改 | `SwitchToDongle` 键处理 | ~30 行 |
| `rmk/src/channel.rs` | 修改 | `DONGLE_AUTH_SIGNAL` | 数行 |
| `rmk/src/usb/mod.rs`、`usb/rynk.rs` | 修改 | gate 放宽为 `any(rynk, dongle)`；`run_host_usb` 经 `RynkUsbService` trait 泛化，`UsbTransport` 增加 `with_dongle_router` | ~40 行 |
| `rmk/src/storage/mod.rs` | 修改 | `DongleSlotMeta` 键 + 数据变体；`new_storage_for_dongle` | ~40 行 |
| `rmk-types` protocol/rynk | 修改 ×5 + 新增 1 | 命令段、payload、error、UUID、`decode_header` | ~200 行 |
| `rmk-config/src/lib.rs` | 修改 | 3 个 `[rmk]` 字段 | ~15 行 |
| `examples/use_rust/nrf52840_dongle` | 新增 | dongle 固件例子 | ~250 行 |

## 7. 实施切分

每个 PR 独立可验证，依赖只向前：

| # | 内容 | 验证方式 |
| --- | --- | --- |
| 1 | rmk-types：命令段 / payload / `NoTarget` / UUID / `decode_header` | 单元 + snapshot 测试 |
| 2 | 键盘侧：槽位、键码、广播分支、`dongle_ctrl`（随 `rynk`+BLE 编入） | nRF Connect 手机扫描验证三种广播形态与特征表；`vial` 构建体积/行为零变化；既有 BLE 回归 |
| 3 | dongle 核心：模块骨架 + link + merge + USB 输出（先只做打字，硬编码 bond 跳过配对） | 双 DK 手测打字；merge 单测 |
| 4 | 配对：窗口、扫描选优、槽位淘汰、版本门 | 双 DK 走 §2.5 配对行为表 |
| 5 | 配置：router + 0x09xx + 直通 | router 单测 + Rynk 上位机实测（单/多键盘） |
| 6 | 例子 + 用户文档 + 英文版设计文档 | CI 例子构建 |

## 8. 测试计划

**单元测试**（跑在既有 host 测试行里）：

- `merge.rs`：并集 / 溢出截断 / 断链清理重发——纯函数直接断言。
- `router.rs`：复用 `RynkService` 测试的 `ChunkRead` / `VecWrite` 模式（`rmk/src/host/rynk/mod.rs` 测试节）：0x09xx 自答、单槽隐式 target、多槽 `NoTarget`、目标断链 `NotReady`、直通帧字节级原样、kb→host 帧边界重组不与自答帧交错。
- `decode_header`：整帧 / 分块到达 / 头部跨 COBS code byte。
- 命令表 snapshot：现有机制自动覆盖 0x09xx 段。

**手测矩阵**（2× nRF52840 DK + 1 台键盘直连对照，逐行核对 §2.5 的四张行为表）：

| 场景 | 覆盖 |
| --- | --- |
| 首配：长按 → 插 dongle | 上电窗口、RSSI 选优、版本门 |
| 拔插 dongle | bond 持久化、自动重连 |
| 加第二把：已连接键盘长按 | 授权窗口、双键盘合并打字 |
| 键盘切走 / 切回 profile | dongle 不打扰、自动重连 |
| 改绑另一个 dongle 后回连 | 加密失败清槽位 |
| 打字中拔键盘电池 | 防卡键 |
| Rynk 工具直连 vs 经 dongle（单槽） | 透明直通、全量读写键位表 |
| Rynk 工具经 dongle（双槽） | NoTarget、SelectTarget、target 断链通知 |
| 三模切换 USB / BLE / dongle | 键码通路、preferred 交互 |

**CI**：`dongle` 加入一行 `cargo nextest` feature 组合与例子构建（键盘侧随既有 `rynk` 行覆盖）；`scripts/test_all.sh` 相应加行。

## 9. 风险与开放问题

| 项 | 说明 | 处置 |
| --- | --- | --- |
| trouble-host central 侧 LESC 配对 | 代码存在（`security_manager/pairing/central.rs`、`Connection::request_security`），但 RMK 从未用过 central 侧 SMP（split 链路不加密） | PR 3 前用最小 demo 先行验证；有问题给 trouble 提 PR（RMK 已有 patch 先例） |
| 定向广播兼容性 | 个别 controller 对 directed adv 的行为差异 | 回退方案：带 `BONDED` kind 的不可发现 undirected 广播，dongle 仍按 accept list 连；只改键盘一处分支 |
| 合并溢出 | 两把键盘合计 >6 键时溢出键丢弃 | 记录在用户文档；NKRO 描述符是后续独立改动 |
| 键盘电池状态 | 不进 `DongleSlot`：dongle 只能从 BAS 拿到 level，没有充电状态 | 上位机对配置目标透传 `GetBatteryStatus`，拿键盘自己的 `BatteryStatus` |
| Dongle 自身省电 | USB suspend 时是否停扫描 | 非目标，挂起；dongle 由 USB 供电，动机弱 |
| `dongle_pairing_window_secs` 的安全/易用平衡 | 30 s 是初值 | 可配置项，实测后调整默认值 |
