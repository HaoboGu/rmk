# Joysticks

A joystick is an analog input device that can be used for mouse control and other functions. Currently, only NRF series chips are supported.

::: warning

1. You need to use a debug probe to find your parameters now.
2. Only Nrf is supported now.

:::

TODO:

- [ ] a more intuitive way to configure the joystick
- [ ] more functions besides mouse

## `toml` configuration

```toml
[[input_device.joystick]]
name = "default"
pin_x = "P0_31"
pin_y = "P0_29"
pin_z = "_"
transform = [[80, 0], [0, 80]]
bias = [29130, 29365]
resolution = 6
# func = "mouse | n-direction key" # TODO: only mouse is supported now
```

### Parameters:

- `name`: Unique name for the joystick. If you have multiple joysticks, they need different names
- `pin_x`: Pin for X-axis
- `pin_y`: Pin for Y-axis
- `pin_z`: Pin for Z-axis
- `transform`: Transformation matrix for the joystick
- `bias`: Bias value for each axis
- `resolution`: Resolution for each axis

::: note
`_` indicates that the axis does not exist. `_` is only allowed for:

1. Both y and z axes
2. Only z axis

For example: `pin_x = "_"` `pin_y = "P0_29"` `pin_z = "P0_30"` is not allowed
:::

::: tip
The transform might be not very intuitive, please read the document below for more information.
:::

#### How it works


1. Device reads values from each axis
2. Adds the `bias` value to each axis to make the value close to 0 when the joystick is released
3. About the `transform` matrix:
   1. New x-axis value = (axis_x + bias[0]) / transform[0][0] + (axis_y + bias[1]) / transform[0][1] + (axis_z + bias[2]) / transform[0][2]
   2. New y-axis value = (axis_x + bias[0]) / transform[1][0] + (axis_y + bias[1]) / transform[1][1] + (axis_z + bias[2]) / transform[1][2]
   3. New z-axis value = (axis_x + bias[0]) / transform[2][0] + (axis_y + bias[1]) / transform[2][1] + (axis_z + bias[2]) / transform[2][2]

   If `transform[new_axis][old_axis]` is 0, that old axis value is ignored.

   Since the value range read by the ADC device is usually much larger than the mouse report range of -256~255, `transform` is designed as a divisor.

4. Each axis value is adjusted to the largest integer multiple of `resolution` that is less than its original value to reduce noise from ADC device readings.

#### How to find configuration for your hardware quickly

1. First set `bias` to 0, `resolution` to 1, and `transform` to `[[1, 0, 0], [0, 1, 0], [0, 0, 1]]` (matrix dimension depends on the number of axes)

2. Find the optimal `bias` value:
   - Use a debug probe to find the output `JoystickProcessor::generate_report: record = [axis_x, axis_y, axis_z]` in debug information
   - Observe these values to find the `bias` value that makes each axis closest to 0 when the joystick is released

3. If the mouse moves too fast, gradually increase the `transform` value until you find the right sensitivity

4. If the mouse jitters, gradually increase the `resolution` value until the jitter disappears


## `rust` configuration

Because the `joystick` and `battery` use the same ADC peripheral, they actually use the same `NrfAdc` `input_device`.

If the `light_sleep` is not `None`, the `NrfAdc` will enter light sleep mode when no event is generated after 1200ms, and the polling interval will be reduced to the value assigned.

```rust
let saadc_config = saadc::Config::default();
let adc = saadc::SAADC::new(p.SAADC, Irqs, saadc_config,
    [
        saadc::ChannelConfig::SingleEnded(saadc::VddhDiv5Input.degrade_saadc()),
        saadc::ChannelConfig::SingleEnded(p.P0_31.degrade_saadc()),
        saadc::ChannelConfig::SingleEnded(p.P0_29.degrade_saadc())
    ],
);
saadc.calibrate().await;
let mut adc_dev = NrfAdc::new(adc, [AnalogEventType::Battery, AnalogEventType::Joystick(2)], 20 /* polling interval */, Some(350)/* light sleep interval */);
let mut batt_proc = BatteryProcessor::new(1, 5, &keymap);
let mut joy_proc = JoystickProcessor::new([[80, 0], [0, 80]], [29130, 29365], 6, &keymap);
...
run_devices! (
    (matrix, adc_dev) => EVENT_CHANNEL,
),
run_processor_chain! {
    EVENT_CHANNEL => [joy_proc, batt_proc],
}
...
```
