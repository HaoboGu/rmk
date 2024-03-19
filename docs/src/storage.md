# Storage

TODO: Update storage documentation

Storage feature is used by saving keymap edits to internal flash. By default, it uses **last 2 sectors** of your microcontroller's internal flash. So you have to ensure that you have enough flash space for storage feature if you pass the storage argument to RMK. If there is not enough space, passing `None` is acceptable.

If you're using nrf528xx + BLE, this feature is automatically enabled because it's required to saving BLE bond info. 

Future work: 

- [ ] make it configurable that how many sectors to be used(but at least 2)
- [ ] add storage to RMK feature gate, disable all related stuffs if the feature is not enabled. This could save a lot of flash
- [ ] Save more configurations to storage
