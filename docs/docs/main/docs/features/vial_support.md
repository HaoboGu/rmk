# Vial support

RMK uses [vial](https://get.vial.today/) as the default keymap editor. By using vial, you can change your keymapping at real-time, no more programming/flashing is needed.

::: warning

To persistently save your keymap data, RMK will use your microcontroller's internal flash as the storage. See [storage](./configuration/storage.md). If you don't have enough flash for saving keymaps, changing in vial will lose after keyboard reboot.

:::

## Port vial

To use vial in RMK, a keyboard definition file named `vial.json` is necessary. Vial has a very detailed documentation for how to generate this JSON file: <https://get.vial.today/docs/porting-to-via.html>. One note for generating `vial.json` is that you have to use same layout definition of internal keymap of RMK, defined in `src/keymap.rs` or `keyboard.toml`.

In `vial.json`, you define the keyboard layout, specifying which keys are in which position. In `src/keymap.rs` or `keyboard.toml`, you define the keymap, meaning which symbol will be printed by the press of which button.

This is the default keymap, which you can change using [the vial app (or the web app)](https://get.vial.today). Unless you set `clear_storage = true` (see [storage](./configuration/storage.md)), these changes will persist when you reset your keyboard.

After getting your `vial.json`, just place it at the root of RMK firmware project, and that's all. RMK will do all the rest work for you.

## Disable vial

Note vial support requires extra Flash/RAM space. You can also disable vial support to reduce the binary size and RAM consumption. If you're using `keyboard.toml`, you can disable vial by setting `vial_enabled` under `[rmk]` section and disable `vial` feature(by disabling default features) in `Cargo.toml` to fully disable vial:

```toml
# In keyboard.toml:
[rmk]
vial_enabled = false

# In Cargo.toml
rmk = { version = "...", default-features = false, features = ["col2row"] }
```
