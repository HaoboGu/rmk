# SimKeyboard 宏观端到端测试 API 计划

## Summary

- `rmk::sim::SimKeyboard` 是 host-side 的完整模拟键盘：除物理输入和 USB/BLE transport I/O 被虚拟化外，创建方式、运行路径、按键处理、Vial host service、Rynk protocol dispatch 都尽量走真实固件路径。
- 所有会模拟用户输入、旋钮输入、host 协议修改键盘状态、或观察 HID 输出的集成测试，都使用 `SimKeyboard` / `SimHost` 端到端 API。
- 测试作者只表达三件事：键盘如何配置、外界对键盘做了什么、键盘应该输出什么。

## Public API Shape

```rust
let mut keyboard = SimKeyboard::create(keymap).await;

let mut keyboard = SimKeyboard::builder(keymap)
    .behavior(behavior_config)
    .positional(positional_config)
    .encoders(encoder_map)
    .vial() // or .rynk(), depending on the enabled protocol feature
    .build()
    .await;

keyboard
    .press(row, col)
    .delay(100)
    .release(row, col)
    .expect_keys([HidKeyCode::A])
    .run()
    .await;
```

`SimKeyboard` 使用 timeline 模型：`press/release/delay/expect_*` 都是同步方法，只向模拟时间线追加步骤；`.run().await` 启动真实 `Keyboard::run()`、可选 host service，并执行整条时间线。

## Input API

```rust
keyboard.press(row, col);
keyboard.release(row, col);
keyboard.tap(row, col, hold_ms);
keyboard.delay(ms);
keyboard.delay_ms(ms);

keyboard.rotary_cw(id);
keyboard.rotary_ccw(id);

keyboard.event(KeyboardEvent::key(row, col, true));
```

## Assertion API

```rust
keyboard.expect_keys([HidKeyCode::A, HidKeyCode::B]);
keyboard.expect_keycodes([0x04, 0x05]);
keyboard.expect_mods(KC_LSHIFT);
keyboard.expect_keyboard_report(report);
keyboard.expect_empty();
keyboard.expect_no_report(ms);
keyboard.expect_report(Report::KeyboardReport(report));
```

## Host API

```rust
let host = SimHost::usb();

host.vial(&mut keyboard)
    .set_key(layer, row, col, k!(B))
    .expect_ok();

host.vial(&mut keyboard)
    .get_protocol_version()
    .expect(expected_reply);

host.rynk(&mut keyboard)
    .set_key(layer, row, col, k!(B))
    .expect_ok();

host.rynk(&mut keyboard)
    .request(Cmd::GetVersion, ())
    .expect(ProtocolVersion::CURRENT);
```

Host operations append protocol steps into the target `SimKeyboard` timeline. Vial runs through the real host service and static request/reply channels; Rynk runs through the real transport-agnostic `RynkService::dispatch`. Protocol mutations and later physical key events are tested in one end-to-end scenario.

## Migration Rules

- `rmk/tests` must not use raw keyboard construction, legacy sequence helpers/macros, direct HID/protocol channels, or lower-level simulator task helpers. Tests should directly script `SimKeyboard` timelines with `press/release/delay/expect_*` and finish with `.run().await`.
- Shared fixtures in `rmk/tests/common` return `SimKeyboard` or `SimKeyboardBuilder`; they do not expose raw `Keyboard<'static>` construction.
- Pure serialization, wire-format, conversion, and tiny internal unit tests may remain as unit tests outside this simulator DSL.
- Any test that models user input, encoder input, host protocol changing keyboard state, or HID output must use `SimKeyboard`.

## Verification

- `sh scripts/check_sim_tests.sh`
- `cargo nextest run --manifest-path rmk/Cargo.toml --no-default-features`
- `cargo nextest run --manifest-path rmk/Cargo.toml --no-default-features --features "std,rynk"`
- `cargo nextest run --manifest-path rmk/Cargo.toml --no-default-features --features "split,vial,storage,async_matrix,_ble"`
- `cargo check --manifest-path rmk/Cargo.toml --no-default-features --features "std,_ble,_no_usb"`
- `sh scripts/test_all.sh`
