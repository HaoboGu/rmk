rmk に Azoteq TPS65 トラックパッドのドライバを追加したい。


- TPS65の詳細
  - https://www.digikey.jp/ja/products/detail/azoteq-pty-ltd/TPS65-201A-S/7164942
  - https://holykeebs.com/products/touchpad-module

TPS65 は iqs5xx のチップを積んでおり、以下のドライバのサンプルが存在する。

参考実装として

- rust 独自実装のもの
  - https://www.reddit.com/r/ErgoMechKeyboards/comments/10hbco2/added_a_tps65_touchpad_to_eskarp/?tl=ja
  - https://github.com/legokichi/iqs5xx
- zmk のもの
  - https://www.reddit.com/r/ErgoMechKeyboards/comments/1mm73zi/zmk_driver_for_azoteq_trackpads/
  - https://github.com/legokichi/zmk-driver-azoteq-iqs5xx
  - https://github.com/legokichi/zmk-keyboard-iqs5xx-dev

を git submodule で用意している。


./rmk-driver-azoteq-iqs5xx フォルダを作り、そこに rmk 用の iqs5xx ドライバを実装せよ。

## 実装計画
1. 既存実装の把握: `iqs5xx` と `zmk-driver-azoteq-iqs5xx` の初期化手順・レポートフォーマット・ジェスチャー処理を整理する。
2. RMK 側の入力デバイス設計: `rmk/rmk/src/input_device/pmw3610.rs` を参考に、`InputDevice` + `InputProcessor` の責務を分離する。
3. 新規クレート設計: `./rmk-driver-azoteq-iqs5xx` に `no_std` クレートを作成し、I2C + RDY/RST ピン + 非同期待機を扱える API にする。
4. 変換ロジック実装: IQS5xx のレポート/ジェスチャーから RMK の `Event`（Joystick/AxisEventStream）を生成し、`MouseReport` への変換も用意する。
5. RMK との統合: `rmk`/`rmk-config`/`rmk-macro` に設定項目と初期化コードを追加し、`keyboard.toml` から設定可能にする。
6. 動作確認: `cargo build --release --bin central/peripheral` または `cargo make build` でビルド確認し、最低限のログ出力で初期化と入力イベントが流れることを確認する。

## 実装指示書
- `./rmk-driver-azoteq-iqs5xx` を新規作成し、`Cargo.toml`/`src/lib.rs` を用意する（`#![no_std]`、`embedded-hal-async` と `embassy-time` を利用）。
- IQS5xx のレジスタ/レポート定義は `iqs5xx` の実装を参照し、必要最小限を新クレート内に移植する（`no_main` は使わない）。
- `Iqs5xxConfig` を定義し、I2C アドレス、座標スケール、軸反転、タップ/スクロール設定など最低限の調整項目を持たせる。
- `Iqs5xxDevice` を実装して `InputDevice` を満たす（RDY ピン待機 → レポート取得 → `Event::Joystick` か `Event::AxisEventStream` を返す）。
- `Iqs5xxProcessor` を実装して `InputProcessor` を満たす（移動は `MouseReport` の `x/y`、二本指スクロールは `wheel/pan` を想定）。
- RMK 統合:
  - `rmk/rmk/src/input_device/mod.rs` にモジュールを追加。
  - `rmk/rmk-config/src/lib.rs` の `InputDeviceConfig` に `iqs5xx` 設定を追加。
  - `rmk/rmk-macro/src/input_device` に展開ロジックを追加し、RP2040/RP2350 の I2C と GPIO 初期化を生成。
- `keyboard.toml` に設定例を追加する（必要なら README にも簡潔な使用例を追記）。
- 生成物 (`pico2wh-*.uf2` 等) は追加しない。

## rmk-driver-azoteq-iqs5xx 実装内容（現状）
- 新規クレート `rmk-driver-azoteq-iqs5xx` を追加（`#![no_std]`、edition 2024）。
- `src/registers.rs` に IQS5xx の主要レジスタ/ビット定義を移植。
- `src/lib.rs` に非同期 I2C ドライバを実装（`embedded-hal-async` + `embassy-time`）。
- `Iqs5xxConfig` を追加（I2C アドレス、リセット/待機時間、ジェスチャー有効化、軸反転/XY 入替、感度系など）。
- `Report`/`Touch`/`Event` を定義し、レポート→イベント変換を実装。
- `rmk` feature で RMK 連携モジュール `rmk_support` を用意:
  - `Iqs5xxDevice` が `InputDevice` を実装（RDY 監視 + レポート取得 + タップ/スクロール/移動を `Event` 化）
  - `Iqs5xxProcessor` が `InputProcessor` を実装（移動を `MouseReport` に、スクロールは `wheel/pan` に変換）
  - タップ/ホールドは `Event::Custom` にエンコードしてボタン操作へ変換

## 未対応/今後の課題
- `rmk` 側（`rmk-config`/`rmk-macro`）への統合は未実施。
- `keyboard.toml` 設定からの自動生成は未対応。
- 実機での初期化/イベント動作確認は未実施。

## これまでの作業（実施済み）
- `rmk-driver-azoteq-iqs5xx` クレートを作成し、非同期 I2C ドライバ + レポート/イベント変換を実装。
- `rmk` feature で RMK 連携モジュール（`InputDevice`/`InputProcessor`）を追加。
- `Cargo.toml` の依存を整理（`usbd-hid` を 0.9 に更新、`defmt` feature を追加）。
- `README.md` に使用例を追記し、examples を列挙。
- `examples/raw_driver.rs` と `examples/rmk_integration.rs` を追加（no_std + panic handler）。
- `examples/embassy_rp_pico2w.rs` を追加（embassy-rp + I2C 初期化の実機向けサンプル）。
- `cargo check` / `cargo check --examples` / `cargo fmt` / `cargo clippy --lib --examples` を実行。
- RMK 側のIQS5xx統合を実装し、`rmk` サブモジュールでブランチ `iqs5xx-input-device` を作成してコミット。

## 次にやること候補
- `examples/embassy_rp_pico2w.rs` の SDA/SCL/RDY/RST ピン割り当てを実配線に合わせて調整。
- RMK 側の `run_devices!` に接続する具体例を追加。

## RMKにIQS5xxを使うサンプルコード（実装案）

目的: `keyboard.toml` で `iqs5xx` を設定したときの最小動作例を RMK 側の examples に追加する。

### 追加場所案
- `rmk/examples/use_config/rp2350/` もしくは `rmk/examples/use_config/rp2040/` にサンプルを追加
  - `keyboard.toml` に `[[input_device.iqs5xx]]` を書き、`rmk` マクロの自動生成で動かす例
- もしくは `rmk/examples/use_rust/rp2350/` に「手動追加」版を追加
  - `Iqs5xxDevice`/`Iqs5xxProcessor` をコードで組み込む例

### サンプル内容（config版）
- `keyboard.toml` に以下を追加
  - `[[input_device.iqs5xx]]` or `[[split.central.input_device.iqs5xx]]`
  - `i2c.instance/sda/scl/address/frequency`
  - `rdy/rst`
  - `poll_interval_ms`, `enable_scroll`, `scroll_divisor`
- 既存の `src/main.rs` はそのまま（`#[rmk_*]` マクロで自動初期化）
- 動作確認: `cargo build --release --bin central` (splitの場合)

### サンプル内容（手動版）
- `Iqs5xxDevice::new` と `Iqs5xxProcessor::new` を作成
- `run_devices!` に trackpad を追加
- `run_processor_chain!` に processor を追加

### ドキュメント更新
- `rmk/docs/.../configuration/input_device/iqs5xx.md` に examples のパスを追記
- もし config 版なら `input_device/index.md` からも参照を追加

## RMK 側修正の方針（やることと変更箇所）

目的: `keyboard.toml` から IQS5xx を宣言し、RMK の自動生成コードで I2C + RDY/RST を初期化して `InputDevice`/`InputProcessor` を組み込めるようにする。

### 1) rmk-config: 設定項目の追加
- 変更ファイル: `rmk/rmk-config/src/lib.rs`
- 追加内容:
  - `InputDeviceConfig` に `iqs5xx: Option<Vec<Iqs5xxConfig>>` を追加
  - `Iqs5xxConfig` 構造体を新設（例: `name`, `i2c`(SDA/SCL + I2C番号), `rdy`, `rst`, `addr`, `invert_x`, `invert_y`, `swap_xy`, `enable_scroll`, `enable_two_finger_tap`, `enable_press_and_hold`, `press_and_hold_time_ms`, `bottom_beta`, `stationary_threshold`, `poll_interval_ms` など）

### 2) rmk-macro: 初期化コード生成
- 変更ファイル: `rmk/rmk-macro/src/input_device/mod.rs`
  - `iqs5xx` 展開関数を呼び出すように追加
- 新規追加: `rmk/rmk-macro/src/input_device/iqs5xx.rs`
  - `expand_iqs5xx_device(...)` を実装
  - `ChipSeries::Rp2040|Rp2350` を対象に I2C + RDY/RST の初期化コードを生成
  - `rmk-driver-azoteq-iqs5xx::rmk_support::{Iqs5xxDevice, Iqs5xxProcessor}` を生成コードで使う
  - `I2c::new_async(...)` + `Input::new(...)` + `Output::new(...)` を組み立て、`poll_interval_ms` などの設定を反映

### 3) rmk本体: input_device モジュールの拡張
- 変更ファイル: `rmk/rmk/src/input_device/mod.rs`
  - `pub mod iqs5xx;` を追加（必要なら `rmk-driver-azoteq-iqs5xx` を再エクスポートする薄いモジュールを作る）
  - もし再エクスポート方針なら `rmk/rmk/src/input_device/iqs5xx.rs` を追加し、`pub use rmk_driver_azoteq_iqs5xx::rmk_support::*;` を置く

### 4) rmk crate の依存追加
- 変更ファイル: `rmk/rmk/Cargo.toml`
- 追加内容:
  - `rmk-driver-azoteq-iqs5xx = { path = "../../rmk-driver-azoteq-iqs5xx", default-features = false, features = ["rmk"] }`
  - `rmk` の feature として必要なら `iqs5xx` を追加し、`rmk-driver-azoteq-iqs5xx` を feature で切替可能にする

### 5) 設定例の追加
- 変更ファイル: `keyboard.toml`
  - `input_device.iqs5xx` の記述例を追加
  - ピン名は本ボードの命名規則に合わせる
- 必要なら `README.md` に利用手順を追記

### 6) 動作確認
- `cargo make build` または `cargo build --release --bin central/peripheral`
- 初期化ログとイベント発火（スクロール/タップ/移動）を確認

## keyboard.toml 仕様案（IQS5xx）

```toml
[input_device]

[[input_device.iqs5xx]]
# 任意の識別名（マクロ生成時の変数名に使う）
name = "trackpad"

# I2C 設定
# - i2c は RMK の既存パターンに合わせる（I2C番号とSDA/SCLピン）
[input_device.iqs5xx.i2c]
instance = "I2C0"        # 既存の I2cConfig と同じキー名
sda = "PIN_0"
scl = "PIN_1"
address = 0x74           # 既存の I2cConfig と同じキー名
frequency = 400000       # 必要に応じて追加 (Hz)

# RDY/RST ピン
rdy = "PIN_3"            # 入力（プル設定はマクロ側で Pull::Down 推奨）
rst = "PIN_2"            # 出力（初期 High）

# ジェスチャー有効/無効
enable_single_tap = true
enable_press_and_hold = true
press_and_hold_time_ms = 250
enable_two_finger_tap = true
enable_scroll = true

# 軸設定
invert_x = false
invert_y = false
swap_xy = false

# 感度/フィルタ系
bottom_beta = 5
stationary_threshold = 5

# 読み取り間隔 (ms)
poll_interval_ms = 5

# スクロール挙動（Processor 側設定）
scroll_divisor = 32
natural_scroll_x = false
natural_scroll_y = false
```

メモ:
- `i2c` は `rmk-config` の既存 `I2cConfig` (`instance/sda/scl/address`) に合わせる。
- `frequency` は拡張項目として追加する想定。
- `scroll_*` は `Iqs5xxProcessorConfig` 側の設定（rmk-config で持つ）。
- RP2040/RP2350 以外は当面非対応。
