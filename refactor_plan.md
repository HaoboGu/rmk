# RMK rmk-macro 架构优化方案

## 背景

rmk-macro 是 RMK 键盘固件的过程宏代码生成 crate。核心问题：
- `behavior.rs`（478行）混合 6 种行为类型
- `action_parser.rs`（520行）的 `parse_key()` 有 23 个分支，存在大量重复模式
- `input_device/mod.rs` 和 `split/peripheral.rs` 有大量重复的设备初始化模式
- `orchestrator.rs`（405行）内嵌了本应独立的函数

## 不变的部分

- 所有 expand_* 函数的最终 TokenStream 输出不变
- 对外 API（`#[rmk_keyboard]`, `#[rmk_central]`, `#[rmk_peripheral]` 等）不变
- crate 边界不变

## 工作流程

**每个步骤只重构一个函数（一个小点）。重构完成后，等待用户手动确认再进行下一步。**

---

## Step 0: 保存计划 ✅

将当前计划保存到项目根目录的 `refactor_plan.md` 中。

---

## Phase 1: 增加 expand 测试（重构前必须完成） ✅

在 `rmk-macro/src/codegen/` 各模块中添加 `#[cfg(test)] mod tests`，为核心 expand 函数建立回归测试基线。

**测试策略**: 使用 `quote!` 构造输入，通过 `prettyplease` 格式化后做字符串比较。

**已完成**: 共 45 个测试全部通过 (`cargo test --lib codegen`)

### 1.1 behavior 测试 — `behavior.rs` ✅ (15 个正例)
- `test_expand_tri_layer_some/none` — Some/None 两种情况 (prettyplease 精确比较)
- `test_expand_one_shot_with_timeout/without_timeout/none` — 3 种情况
- `test_expand_combos_some/none` — combo 配置展开
- `test_expand_macros_some/none` — macro 配置展开
- `test_expand_forks_some/none` — fork 配置展开
- `test_expand_morse_none/with_morse_actions/with_tap_hold_actions/with_individual_tap_hold` — 4 种 morse 变体

### 1.2 action_parser 测试 — `action_parser.rs` ✅ (23 正例 + 4 负例)
正例：
- `test_parse_key_transparent/no/hid_keycode` — 基础按键
- `test_parse_key_mo/osl/tt/tg/to/df` — 6 种 layer 操作
- `test_parse_key_wm/osm/lm/mt` — 4 种修饰键
- `test_parse_key_lt/lt_with_profile/th/th_with_profile` — tap-hold（含 profile）
- `test_parse_key_user/user_alt_format/macro/macro_alt_format/td/shifted` — 特殊按键

负例（`#[should_panic]`）：
- `test_parse_key_wm_invalid_args` — WM 参数数量错误
- `test_parse_key_user_out_of_range` — User(32) 超出范围
- `test_parse_key_profile_not_found` — LT 引用不存在的 profile（profiles=None）
- `test_parse_key_th_profile_not_found` — TH 引用不存在的 profile（profiles 有值但 key 不匹配）

### 1.3 behavior 负例测试 — `behavior.rs` ✅ (3 个负例)
- `test_expand_morses_conflict_actions_and_tap` — morse_actions 与 tap 同时存在应 panic
- `test_expand_morses_conflict_tap_actions_and_tap` — tap_actions 与 tap 同时存在应 panic
- `test_expand_forks_missing_match` — fork 缺少 match_any 和 match_none 应 panic

---

## Phase 2: 重构 behavior 代码生成

### 2.1 轻量简化 profiles 传递

**现状**: `&Option<HashMap<String, MorseProfile>>` 类型冗长，提取逻辑在 behavior.rs 和 layout.rs 各写一次。

**方案**: 用 type alias + 辅助函数简化，不引入新抽象：

```rust
// action_parser.rs
pub(crate) type MorseProfiles = Option<HashMap<String, MorseProfile>>;

pub(crate) fn get_morse_profiles(config: &KeyboardTomlConfig) -> MorseProfiles {
    config.get_behavior_config()
        .unwrap()
        .morse
        .and_then(|m| m.profiles)
}
```

改动后：
- `behavior.rs`: `let profiles = get_morse_profiles(config);`
- `layout.rs`: `let profiles = get_morse_profiles(config);`
- 所有函数签名: `&Option<HashMap<String, MorseProfile>>` → `&MorseProfiles`

**影响文件**: `action_parser.rs`, `behavior.rs`, `layout.rs`

### 2.2 重构 `parse_key` 内部 — 提取通用解析模式

**问题**: `parse_key()` 的 23 个分支中有大量重复模式：
- 7 个数字参数分支（MO/OSL/TT/TG/TO/DF/TD）用相同的 `get_number` + `quote!` 模式
- 3 个枚举变体查找（KeyboardAction/LightAction/SpecialKey）用相同的 `.find()` 模式

**方案**: 提取 2 个辅助函数，不做 trim 等语义变更（语义等价重构）：

```rust
/// 统一处理 MO(1), OSL(2), TT(3), TG(4), TO(5), DF(6), TD(7), Macro(0)
fn parse_numeric_action(s: &str, prefix: &str, macro_name: &str) -> Option<TokenStream2>

/// 统一处理 KeyboardAction/LightAction/SpecialKey 枚举变体查找
fn try_parse_enum_variant(key_lower: &str) -> Option<TokenStream2>
```

重构后 `parse_key` 结构（分支顺序必须保持现状）：
```rust
pub(crate) fn parse_key(key: String, profiles: &MorseProfiles) -> TokenStream2 {
    // 注意：不添加 trim()，保持与现有行为完全一致
    // 1. 简单 token（Transparent, No）
    // 2. 数字参数类 — 8 种统一处理
    for (prefix, macro_name) in NUMERIC_ACTIONS {
        if let Some(ts) = parse_numeric_action(&key, prefix, macro_name) { return ts; }
    }
    // 3. 修饰键类（WM, OSM, LM）
    // 4. Tap-Hold 类（LT, MT, TH — 使用 profiles）
    // 5. 特殊类（Shifted, User）
    // 6. 枚举变体查找
    if let Some(ts) = try_parse_enum_variant(&key.to_lowercase()) { return ts; }
    // 7. 默认：HID keycode
}
```

**注意**: 分支顺序必须与现有代码保持一致，不可重排。

**影响文件**: `action_parser.rs`

### 2.3 重构 `expand_morses` — 分离 3 种 morse 配置变体

**问题**: `expand_morses()` (L207-288) 有 80 行，3 个嵌套分支处理 3 种不同的 morse 配置格式，互斥校验逻辑混在展开逻辑中。

**方案**: 将 3 种变体分离为独立函数：

```rust
/// morse_actions 格式: [{pattern = "10", action = "A"}, ...]
fn expand_morse_from_actions(...) -> TokenStream2

/// tap_actions/hold_actions 格式
fn expand_morse_from_tap_hold_actions(...) -> TokenStream2

/// 单独 tap/hold 格式
fn expand_morse_from_individual(...) -> TokenStream2
```

`expand_morses` 变为简洁的分发函数。

**影响文件**: `behavior.rs`

### 2.4 拆分 behavior.rs 为子模块目录

在 2.1-2.3 完成后，按行为类型拆分文件：

```
rmk-macro/src/codegen/behavior/
    mod.rs        — expand_behavior_config() 主入口
    combo.rs      — expand_combos()
    fork.rs       — expand_forks() + StateBitsMacro + parse_state_combination()
    macros.rs     — expand_macros()
    morse.rs      — expand_morse() + expand_morses() + 3 个变体函数
    one_shot.rs   — expand_one_shot()
    tri_layer.rs  — expand_tri_layer()
```

**影响文件**: `behavior.rs` → 拆分为 7 个文件，`mod.rs` 更新模块声明

---

## Phase 3: 重构 input device 代码生成

### 3.1 提取 `collect_initializers` 辅助函数

**问题**: 以下模式在 `input_device/mod.rs` 中重复 5 次，在 `split/peripheral.rs` 中重复 3 次（共 8 处，~80 行）：

```rust
for initializer in device_initializers {
    initialization.extend(initializer.initializer);
    let device_name = initializer.var_name;
    devices.push(quote! { #device_name });
}
```

**方案**: 在 `input_device/mod.rs` 中添加：

```rust
pub(crate) fn collect_initializers(
    initializers: Vec<Initializer>,
    initialization: &mut TokenStream,
    names: &mut Vec<TokenStream>,
) {
    for init in initializers {
        initialization.extend(init.initializer);
        let name = init.var_name;
        names.push(quote! { #name });
    }
}
```

**影响文件**: `input_device/mod.rs`, `split/peripheral.rs`

### 3.2 提取 `get_central_input_device` 统一配置提取

**问题**: 从 BoardConfig 提取中央板设备配置的 `match &board { UniBody => ..., Split => ... }` 模式重复 4 次（ADC/Encoder/PMW3610/PMW33xx）。

**方案**:

```rust
fn get_central_input_device(board: &BoardConfig) -> InputDeviceConfig {
    match board {
        BoardConfig::UniBody(UniBodyConfig { input_device, .. }) => input_device.clone(),
        BoardConfig::Split(split) => split.central.input_device
            .clone()
            .unwrap_or_default(),
    }
}
```

**影响文件**: `input_device/mod.rs`

### 3.3 利用辅助函数重构 `expand_input_device_config`

利用 3.1-3.2 的辅助函数简化主函数。电池配置逻辑保持原位不动（central 有 warning + fallback 逻辑，peripheral 没有，两者差异大，不适合强行统一）。

**影响文件**: `input_device/mod.rs`

### 3.4 利用 `collect_initializers` 重构 `expand_peripheral_input_device_config`

利用 3.1 的 `collect_initializers` 简化 peripheral 版本中的重复收集代码。

**影响文件**: `split/peripheral.rs`

---

## Phase 4: Orchestrator 函数外迁

### 4.1 提取 `keymap_storage.rs`

将 `expand_keymap_and_storage()` + `expand_key_info()` + `expand_key_info_row()` (~94行) 从 orchestrator.rs 移到新建的 `codegen/keymap_storage.rs`。orchestrator.rs L235 已有 `// TODO: move this function to a separate folder` 注释。

**影响文件**: `orchestrator.rs`, 新建 `codegen/keymap_storage.rs`, `codegen/mod.rs`

### 4.2 提取 `matrix_keyboard.rs`

将 `expand_matrix_and_keyboard_init()` + `get_debouncer_type()` (~68行) 从 orchestrator.rs 移到新建的 `codegen/matrix_keyboard.rs`。

**影响文件**: `orchestrator.rs`, 新建 `codegen/matrix_keyboard.rs`, `codegen/mod.rs`, `split/peripheral.rs`（L23 导入 `get_debouncer_type`，L252/L277 调用处需更新导入路径）

---

## Future Plan（记录，暂不实施）

- **rmk-config lib.rs 拆分**: 48 个类型按逻辑分组移入子模块
- **错误处理改善**: `panic!` → `compile_error!()` / `Err(D::Error::custom(...))`
- **增强测试**: 为 rmk-config 添加 TOML 解析测试
- **chip_init 简化**: 按芯片系列抽取独立初始化函数
- **Const Generic 参数简化**: 等 Rust `generic_const_exprs` 稳定后考虑

---

## 验证方式

每个步骤完成后：
1. `cargo test` — 确保所有测试通过
2. `cargo build` — 确保编译通过
3. 使用 `/test-changes` skill 进行完整测试

## 关键文件清单

| 文件 | 行数 | 涉及阶段 |
|------|------|----------|
| `rmk-macro/src/codegen/action_parser.rs` | 520 | Phase 1, 2.1, 2.2 |
| `rmk-macro/src/codegen/behavior.rs` | 478 | Phase 1, 2.1, 2.3, 2.4 |
| `rmk-macro/src/codegen/layout.rs` | 100 | Phase 2.1 |
| `rmk-macro/src/codegen/keymap_macro.rs` | 268 | Phase 2.1 |
| `rmk-macro/src/codegen/input_device/mod.rs` | 245 | Phase 3.1-3.3 |
| `rmk-macro/src/codegen/split/peripheral.rs` | 571 | Phase 3.1, 3.4 |
| `rmk-macro/src/codegen/orchestrator.rs` | 405 | Phase 4.1, 4.2 |
| `rmk-macro/src/codegen/mod.rs` | 18 | Phase 2.4, 4.1, 4.2 |
