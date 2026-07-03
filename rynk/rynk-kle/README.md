# rynk-kle

Convert a physical keyboard layout between [KLE](http://www.keyboard-layout-editor.com/)/[Vial](https://get.vial.today/) JSON and RMK/Rynk's `[layout]` section — as a library. The `rmkit layout` CLI wraps it; the `wasm` feature exposes the same pipeline to JavaScript.

- **Forward** — `convert_kle(&serde_json::Value)`: a raw KLE JSON export or a `vial.json` (same KLE blob wrapped in `layouts.keymap`) becomes a `Generated { display_toml, inner_layout_toml, warnings }`. Key positions, cap sizes, split gaps, rotation, ISO/L-shaped caps, encoders, and VIA layout options are converted to `map` tokens plus `[layout.shapes]` / `[[layout.variant]]` entries. KLE carries no keycodes, so no `[keymap]` is emitted.
- **Reverse** — `to_kle::keyboard_toml_to_vial(&str)`: a `keyboard.toml`'s `[layout]` back into a minimal `vial.json` (default variant, encoders as Vial CW/CCW switch pairs).
- **Decode** — `decode_layout(&str)`: any `[layout]` TOML into `layout::LayoutInfo` (re-exported from `rynk`), via the real wire path — `rmk-config` builds the same compressed blob the firmware serves over `GetLayout`, then it is inflated and postcard-decoded with the host types. What you get is exactly what a Rynk host sees.

Every generated `[layout]` round-trips through `rmk_config::layout_blob_from_toml` — the same builder the firmware uses — and the fixture suite verifies the rendered layout is preserved through `vial.json → [layout] → vial.json`.

## Web

```sh
wasm-pack build --target web --features wasm
```

exports string-in / plain-object-out bindings: `convert_kle(json)`, `keyboard_toml_to_vial(toml)`, and `decode_layout(toml)` (a `LayoutInfo` object for drawing a preview).
