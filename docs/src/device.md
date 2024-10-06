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

## Input device

The keyboard itself is an input device. To use more types of input device, a good abstraction layer is needed.

The first thing that needs to be determined, is what report should input device emit? And how it can be mapped to the keymap/vial? 

