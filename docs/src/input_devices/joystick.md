# Joysticks

<div class="warning">
    Notice:

    1. You need to use a debug probe to find your parameters now.
    2. Only Nrf is supported now.
</div>

TODO:
- [ ] a more intuitive way to configure the joystick via `rmk-gui`
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
- `name`: the unique name of the joystick. If you have multiple joysticks, you should give them different names
- `pin_x`: the pin of the x-axis
- `pin_y`: the pin of the y-axis
- `pin_z`: the pin of the z-axis
- `transform`: the transformation matrix of the joystick
- `bias`: the bias of each axis
- `resolution`: the resolution of each axis

> #### Axis:
>
> The `_` stands for the axis is not existed.
>
> `_` is only allowed for:
> 1. both y and z axis
> 2. only z axis.
>
> e.g. `pin_x = "_"` `pin_y = "P0_29"` `pin_z = "P0_30"` is not allowed

#### How it works

<div class="warning">
    Notice:

    the transform might work not so intuitively,
    please read the document below for more information.
</div>

1. The device read axes
2. Add `bias` on each axis to make them into `0` when the joystick is released because the number returned by the ADC device is `u16`.
3. About the `transform`
    1. new `x-axis`'s value = (`axis_x` + bias\[0\]) / transform\[0\]\[0\] + (`axis_y` + bias\[1\]) / transform\[0\]\[1\] + (`axis_z` + bias\[2\]) / transform\[0\]\[2\]
    2. new `y-axis`'s value = (`axis_x` + bias\[0\]) / transform\[1\]\[0\] + (`axis_y` + bias\[1\]) / transform\[1\]\[1\] + (`axis_z` + bias\[2\]) / transform\[1\]\[2\]
    3. new `z-axis`'s value = (`axis_x` + bias\[0\]) / transform\[2\]\[0\] + (`axis_y` + bias\[1\]) / transform\[2\]\[1\] + (`axis_z` + bias\[2\]) / transform\[2\]\[2\]

    If `transform[new axis][old axis]` is `0`, the old axis value will be ignored.

    For most situation the value boundary read by the ADC device is far larger than `-256~255` which is the range of the mouse report, that is why the `transform` is designed to be the divisor.
4. Each axis will be changed into the maximum integer multiples of `resolution` smaller than its original value.

   Because the values read by the ADC device may have noises.

#### How to find configuration for your circuit quickly
1. Firstly, set the `bias` to `0`, `resolution` to `1` and `transform` to `[[1, 0, 0], [0, 1, 0], [0, 0, 1]]` (the identity 2-d array's dimension depends on how many axes the joystick has).

2. Find the best `bias`,

   Using the debug probe, in the debug information, there is the output `JoystickProcessor::generate_report: record = [axis_x, axis_y, axis_z]`
   Observe the value, and you can get the `bias` which make each axis as close to `0` as possible.

3. Try to use the joystick, if you notice the mouse moves too fast, you can adjust the `transform` larger till you fond.
4. If your mouse is jitter, you should adjust the `resolution` larger till you fond.

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
