# Device

There are two types of device in RMK:

- input device: an external device which finally generates a HID(keyboard/mouse/media) report, such as encoder, joystick, touchpad, etc.

- output device: an external device which is triggered by RMK, to perform some functionalities, such as LED, RGB, screen, motor, etc

## Input device

Here is a simple(but not exhaustive) list of input devices:

- Keyboard itself
- Rotary encoder
- Touchpad
- Trackball
- Joystick

Except keyboard and rotary encoder, the others protocol/implementation depend on the actual device. A driver like interface is what we need.

### Rotary encoder

The encoder list is represented separately in vial, different from normal matrix. But layers still have effect on encoder. The behavior of rotary encoder could be changed by vial.

