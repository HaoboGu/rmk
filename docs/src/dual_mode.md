# Developing :USB & BLE dual mode

BLE & USB should both be used for microcontrollers like nrf52840, with manually or automatically switching.

The user side operation logic should be like:

![usb_ble_switch](diagrams/usb_ble_switching.drawio.svg)

In one word, USB has higher priority to BLE. If there is a USB cable connected, the keyboard should never use BLE. If the USB cable is removed, then the keyboard should behavior like restarting, do BLE advertising and then connect to known host.



- plug -> unplug
    - 会有一个 Power usb removed
    - 会有一个Device disabled出现在 MyDeviceHandler
case2
  - Device suspended -> Power usb removed  -> Bus reset -> Device resumed -> Device disabled
  - 

- unplug -> plug
    - Power usb detected -> Power usb ready
    - Device enabled -> Device suspended -> Bus reset -> Device resumed -> Device configured
    - 

- start when plug
  - Power usb ready
  - Device enabled -> Device suspended

- 

h7:

- start when unplug
  - Device enabled -> Device suspended

- start when plug
  - Device enabled -> Device suspended -> Bus reset -> USB address set -> Device configured

- unplug -> plug
  - Device suspended -> Bus reset -> USB address set -> Device configured

- plug -> unplug
  - Device suspended -> Bus reset -> Device suspended
