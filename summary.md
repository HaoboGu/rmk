# RMK 変更内容まとめ（IQS5xx 対応）

## 概要
- Azoteq IQS5xx トラックパッドを RMK の input_device フレームワークに統合。
- I2C 初期化・IRQ バインド・設定マクロ・ドキュメント・サンプルを追加。

## 主要な追加/変更点

### 1) 入力デバイス実装
- `rmk/src/input_device/iqs5xx.rs` を追加。
  - `Iqs5xxDevice`（イベント読み取り）と `Iqs5xxProcessor`（HID 変換）を実装。
  - クリック/二本指タップ/スクロール/移動を `MouseReport` に変換。

### 2) 設定（rmk-config）
- `Iqs5xxConfig` を追加。
  - 例: `enable_single_tap`, `press_and_hold_time_ms`, `invert_x/y`, `poll_interval_ms` など。
  - `scroll_divisor`, `natural_scroll_x/y` を追加。
- `I2cConfig` に `frequency: Option<u32>` を追加。

### 3) マクロ生成（rmk-macro）
- `input_device/iqs5xx.rs` 追加。
  - I2C インスタンス生成（`I2c::new_async`）と IRQ バインドの自動生成。
  - Split peripheral 側でも I2C IRQ を生成。

### 4) ドキュメント
- `docs/.../input_device/iqs5xx.md` を追加。
- `docs/.../input_device/index.md` に IQS5xx を追加。
- RP2040 のみ対応である旨を明記。

### 5) サンプル
- `examples/use_config/rp2040_iqs5xx` を追加。
  - `keyboard.toml` に IQS5xx 入力デバイス例を記載。

## 対応プラットフォーム
- RP2040 のみ対応（RP2350 対応は drop）。

## メモ
- RMK 側は `rmk-driver-azoteq-iqs5xx` を利用し、循環依存を避けるため feature を分離。
- HID は MouseReport と Custom イベント中心で処理（Zoom 未対応）。
