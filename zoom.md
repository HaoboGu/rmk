# Zoom対応 実装案（IQS5xx）

## 目的
IQS5xx のピンチ（Zoom In/Out）イベントを RMK に取り込み、ホストへズーム操作を送信できるようにする。

## 前提・現状
- IQS5xx ドライバは `Event::Zoom(i16)` を生成済み（`rmk-driver-azoteq-iqs5xx`）。
- RMK 側の `Iqs5xxDevice` は `Zoom` を無視している。
- RMK の HID 送信は `KeyboardReport / MouseReport / MediaKeyboardReport / SystemControlReport` をサポート。

## 方式の候補

### A. Consumer Page (Zoom In/Out) 方式（推奨）
OS側の「Zoom In / Zoom Out」用途の Consumer usage を送る。
- メリット: キーボード状態と干渉しにくい（Ctrl+Wheel 方式より安全）
- デメリット: OS/アプリでの解釈差があり得る

### B. Ctrl + Wheel 方式
Zoom を `KeyboardReport` の Ctrl 押下 + `MouseReport` のホイールで実現。
- メリット: ブラウザでの一般的な挙動に一致しやすい
- デメリット: 既存のキー保持状態と干渉しやすく、実装が複雑

## 実装方針（A案ベース）

### 1) 入力イベント→Custom イベント化
対象: `rmk/rmk/src/input_device/iqs5xx.rs`
- `CUSTOM_TAG_ZOOM` を追加（`Event::Custom` にタグを積む）
- `IqsEvent::Zoom(delta)` を受け取ったら Custom イベント化
  - `i16` を 2byte（LE）で載せる

### 2) Processor 側でズームイベントを処理
対象: `rmk/rmk/src/input_device/iqs5xx.rs`
- `Iqs5xxProcessor` にズーム用の蓄積値と閾値を追加
  - 例: `zoom_acc: i16`, `zoom_divisor: i16`
- `CUSTOM_TAG_ZOOM` の値を蓄積し、閾値超過でズーム 1 ステップ
  - 正方向 → Zoom In
  - 負方向 → Zoom Out
- MediaKeyboardReport を送る
  - 「押す→離す」なので `usage_id` を送った後に `usage_id = 0` を送信

### 3) 設定の追加
対象: `rmk/rmk-config/src/lib.rs` と `rmk/rmk-macro`
- `Iqs5xxConfig` に追加:
  - `zoom_mode: Option<String>`（例: `"consumer" | "off"`）
  - `zoom_divisor: u16`（既定値 32 など）
- `Iqs5xxProcessorConfig` に `zoom_divisor` を追加
- マクロで config を Processor に渡す

### 4) HID usage の扱い
対象: `rmk/rmk-types` or `rmk/rmk`（どちらに寄せるか決める）
選択肢:
- 既存の `MediaKeyboardReport { usage_id: u16 }` を直接使う
  - `usage_id` の値は HID Usage Table の Consumer Page を参照して確定する
- あるいは `ConsumerKey` / `KeyCode` に `AcZoomIn/AcZoomOut` を追加して型安全にする

### 5) docs / keyboard.toml 例
対象: `rmk/docs/...` と `keyboard.toml`
- `zoom_mode` / `zoom_divisor` の記述例を追記

## 変更予定ファイル（A案）
- `rmk/rmk/src/input_device/iqs5xx.rs`
- `rmk/rmk-config/src/lib.rs`
- `rmk/rmk-macro/src/input_device/iqs5xx.rs`
- `rmk/docs/docs/main/docs/features/input_device/iqs5xx.md`
- `keyboard.toml`（サンプル更新）
- （必要なら）`rmk/rmk-types/src/keycode.rs`

## 未決事項
- Consumer usage の具体値（Zoom In/Out/Reset）
- `zoom_mode` の既定値を `off` にするか `consumer` にするか
  - 安全寄りなら `off` 既定
  - 使いやすさ優先なら `consumer` 既定

## 次の確認ポイント
1. 実装方式の決定（A or B）
2. usage_id の確定
3. `zoom_mode` の仕様確定（文字列 or enum）
