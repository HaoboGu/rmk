# Device

There are two types of device in RMK:

- input device: an external device which finally generates a HID(keyboard/mouse/media) report, such as encoder, joystick, touchpad, etc.

- output device: an external device which is triggered by RMK, to perform some functionalities, such as LED, RGB, screen, motor, etc


## Current tasks

- `keyboard_task`
- communication task
  - `communication_task`
  - `ble_communication_task`
- communication background task
  - `gatt_server::run()`
  - `usb_device.run()`
- storage_task
- vial_task
- led_hid_task