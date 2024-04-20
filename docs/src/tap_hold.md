# Tap/Hold?

In current implementation, basic keycode uses 1 byte, each key in the eeprom occupies 2 bytes. So, tap/hold feature for any BASIC keycode is not possible -- You must have an identifier to distinguish tap/hold key from other keys.

QMK implements it by limiting hold actions -- only modifier/layer options can be hold actions. Then, you can use the higher 8 bits for tap/hold identifier(with layer/modifier), and lower 8 bits for basic keycode.

QMK supports only the following tap/hold keys:

- `LT(layer, kc)`: hold to activate layer, tap to send kc
- `MT(mod, kc)`: hold to activate mod, tap to send kc

Hence, in the QMK implementation, 4bit for tap/hold identifier, 4bits for layer_num/modifier, and 8bits for basic keycode.

Is there any other way to implement tap/hold feature for any BASIC keycode?

modifier: 8
layer: 8
at least 5 bits for tap/hold identifier + modifier/layer. 11 bits free for representing keycode


