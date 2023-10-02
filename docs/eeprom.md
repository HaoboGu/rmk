# EEPROM

EEPROM is used to store persistent configurations of the keyboard. Many MCUs doesn't have internal EEPROMs, it's common to use MCUs' flash to emulate EEPROM in keyboard firmwares.

RMK's `eeprom` module provides a eeprom implementation with wear-leveling algorithm. It's based on `embedded-storage` crate, so any storages which implement `embedded-storage::Storage` can be used as the back-end of RMK's eeprom.

## Reference

- https://docs.qmk.fm/#/eeprom_driver

- https://docs.qmk.fm/#/feature_eeprom

## Implementation

To reduce wearing of the flash, data is stored as a "record", which will never be updated. Instead, newer record is appended at the beginning of the free space of the flash. When reading the data, only the newest record is valid.

If the flash space for EEPROM is full, a cleaning operation will be executed, all invalid records are freed.

### Multi-byte encoding of EEPROM

AAAAAAAA BBBBBBBB CCCCCCCC DDDDDDDD
|--- address ---| |----- data ----|
