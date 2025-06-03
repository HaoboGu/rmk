# Keyboard and matrix

## `[keyboard]`

`[keyboard]` section contains basic information of the keyboard, such as keyboard's name, chip, etc:

```toml
[keyboard]
name = "RMK Keyboard"
vendor_id = 0x4c4b
product_id = 0x4643
manufacturer = "RMK"
chip = "stm32h7b0vb"
board = "nice!nano" # optional, cannot be used with `chip`
# If your chip doesn't have a functional USB peripheral, for example, nRF52832/esp32c3(esp32c3 has only USB serial, not full functional USB), set `usb_enable` to false
usb_enable = true
```

### Supported `chip` and `board`

#### chip

- nrf52840
- nrf52833
- nrf52832
- nrf52811
- nrf52810
- esp32c3
- esp32c6
- esp32s3
- rp2040
- ALL stm32s supported by [embassy-stm32](https://github.com/embassy-rs/embassy/blob/main/embassy-stm32/Cargo.toml) with USB

#### board

- `nice!nano`
- `nice!nano_v2`
- `pi_pico_w`   
- `XIAO BLE`

## `[matrix]`

`[matrix]` section defines the physical [key matrix](https://docs.qmk.fm/how_a_matrix_works) IO information of the keyboard, aka input/output pins.

::: warning

For split keyboard, this section should be just ignored, the matrix IO pins for split keyboard are defined in `[split]` section.

:::

### Key matrix

For the normal key matrix, in order to identify the IO pins take a look at your keyboard's schematic: The pin going to the [diode](https://en.wikipedia.org/wiki/Diode) (called anode) is an output pin, the pin coming out (called cathode) is an input pin:

```
output_pin =>   >|   => input_pin
                 ↑
              diode(be aware of it's direction)
```

::: warning

Per default RMK assumes that your pins are <b>col2row</b>, meaning that the output pins (anodes) represent the columns and the input pins (cathodes) represent the rows. If your schemata shows the opposite you need to <a href="https://rmk.rs/docs/user_guide/faq.html#my-matrix-is-row2col-the-matrix-doesn-t-work"> change the configuration to `row2col`</a>

:::

IO pins are represented with an array of string, the string value should be the **GPIO peripheral name** of the chip. For example, if you're using stm32h750xb, you can go to <https://docs.embassy.dev/embassy-stm32/git/stm32h750xb/peripherals/index.html> to get the valid GPIO peripheral name:

![gpio_peripheral_name](/images/gpio_peripheral_name.png)

The GPIO peripheral name varies for different chips. For example, RP2040 has `PIN_0`, nRF52840 has `P0_00` and stm32 has `PA0`. So it's recommended to check the embassy's doc for your chip to get the valid GPIO name first.

Here is an example toml of `[matrix]` section for stm32:

```toml
[matrix]
# Input and output pins are mandatory
input_pins = ["PD4", "PD5", "PD6", "PD3"]
output_pins = ["PD7", "PD8", "PD9"]
# WARNING: Currently row2col/col2row is set in RMK's feature gate, row2col config here is valid ONLY when you're using cloud compilation
# Checkout documentation here: https://rmk.rs/docs/user_guide/faq.html#my-matrix-is-row2col-the-matrix-doesn-t-work
# row2col = true
```

### Direct pins

If your keys are directly connected to the microcontroller pins, set `matrix_type` to `direct_pin`. (The default value for `matrix_type` is `normal`)

`direct_pins` is a two-dimensional array that represents the physical layout of your keys.

If your pin requires a pull-up resistor and the button press pulls the pin low, set `direct_pin_low_active` to true. Conversely, set it to false if your pin requires a pull-down resistor and the button press pulls the pin high.

Here is an example for rp2040.

```toml
matrix_type = "direct_pin"
direct_pins = [
    ["PIN_0", "PIN_1", "PIN_2"],
    ["PIN_3", "_", "PIN_5"]
]
# `direct_pin_low_active` is optional. Default to `true`.
direct_pin_low_active = true
```
