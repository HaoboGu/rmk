# Split keyboard

<div class="warning">
This feature is currently not implemented, this document is a design writeup
</div>

## Design

### Usage design

Defining a split keyboard should be as simple as a normal keyboard. The split-keyboard's type and matrix should be defined in the `keyboard.toml`.

TODO: should we have left.toml & right.toml, or just use one keyboard.toml?

```toml
# Split keyboard definition(draft)
[matrix]
# Total rows & cols
# rows for left & right should be identical(consider introduce dummy pin in the future? to save the used pin)
rows = 4
# cols = left cols + right cols
cols = 7
layers = 2

[split]
split = true
main = "left/right"

# Left & right connection
connection = "i2c"/"ble"/"uart"?

# Pin assignment for left & right
[split.left]
input_pins = ["P1_00", "P1_01", "P1_02", "P1_07"]
output_pins = ["P1_05", "P1_06", "P1_03", "P1_04"]

# If the connection type is i2c
i2c_instance = ""
# If the connection type is ble
ble_addr = ""
# If the connection type is uart
uart_instance = ""

[split.right]
input_pins = ["P1_00", "P1_01", "P1_02", "P1_07"]
output_pins = ["P1_05", "P1_06", "P1_03"]

# If the connection type is i2c
i2c_instance = ""
# If the connection type is ble
ble_addr = ""
# If the connection type is uart
uart_instance = ""

# Other fields are same

```

### Communication between left & right

When the left & right talk to each other, the **debounced key states** are sent. The main(can be either left or right) receives the key states, converts them to actual keycode and then sends keycodes to the host.

That means the main side should have a full keymap stored in the storage/ram. The other side just do matrix scanning, debouncing and sending key states over i2c/uart/ble.

### How to establish the connection?

According to the connection type, some more info should be added. For example, if i2c is used, then the i2c instance of both left/right should be set in `keyboard.toml`.

If the communication is over BLE, a pairing step has to be done first, to establish the connection between right & left. In this case, the random addr of right and left should be set in `keyboard.toml`, to make sure that left & right can be paired.


### Types of split keyboard

There are several types of split keyboard that RMK should support:

1. fully wired: the left and right are connected with a cable, and the host is connected to left/right with an usb cable
2. fully wireless: the left and right are connected using BLE, and the host is connected using BLE as well
3. dongle like: there is a "central" device aka dongle, which connected to both left and right using BLE, and the dongle is connected to host by USB. Note that the dongle can be one of left/right side of the keyboard.
4. partial wireless: the left and right are connected with a cable, and the host is connected using BLE

The following is a simple table for those four types of split keyboard

| left/right connection | wired | wireless |
| ----------- | ----------- | ------------ |
| USB to host | fully wired | dongle like |
| BLE to host | partial wireless| fully wireless|
