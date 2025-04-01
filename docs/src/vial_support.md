# Vial support

RMK uses [vial](https://get.vial.today/) as the default keymap editor. By using vial, you can change your keymapping at real-time, no more programming/flashing is needed. 

<div class="warning">

To persistently save your keymap data, RMK will use the **last two sectors** of your microcontroller's internal flash. See [storage](./storage.md). If you don't have enough flash for saving keymaps, changing in vial will lose after keyboard reboot.

</div>

## Port vial

To use vial in RMK, a keyboard definition file named `vial.json` is necessary. Vial has a very detailed documentation for how to generate this JSON file: <https://get.vial.today/docs/porting-to-via.html>. One note for generating `vial.json` is that you have to use same layout definition of internal keymap of RMK, defined in `src/keymap.rs` or `keyboard.toml`. 

In `vial.json`, you define the keyboard layout, specifying which keys are in which position. In `src/keymap.rs` or `keyboard.toml`, you define the keymap, meaning which symbol will be printed by the press of which button. 

This is the default keymap, which you can change using [the vial app (or the web app)](https://get.vial.today). Unless you set `clear_storage = true` (see [storage]), these changes will persist when you reset your keyboard.

After getting your `vial.json`, just place it at the root of RMK firmware project, and that's all. RMK will do all the rest work for you.