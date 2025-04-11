# Special Keys
RMK maps all [keys](https://docs.rs/rmk/latest/rmk/keycode/index.html) QMK does. However, at the time of writing, not all features are supported.

The following keys are supported (some further keys might work, but are not documented).

## Repeat/Again key
[Similar to QMK](https://docs.qmk.fm/features/repeat_key) pressing this key repeats the last key pressed.
Note that QMK binds this function to `Kc_RepeatKey`, while RMK binds it to `Kc_Again`.
This ensures a better compatibility with Vial, which features the `Again` key as a dedicated key (unlike the `RepeatKey`, which doesn't exist in Vial).
Although some old keyboards might have a key for `Again`, it is not used in modern operating systems anymore.

In QMK an `AlternativeRepeatKey` is supported. This functionalaty is not implemented in RMK.