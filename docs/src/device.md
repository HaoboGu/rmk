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

Here is a simple(but not exhaustive) list of input devices:

- Keyboard itself
- Rotary encoder
- Touchpad
- Trackball
- Joystick

Except keyboard and rotary encoder, the others depend on the actual type of the device, that means there's no universal device for them. A driver like interface is what we need. 

### Draft 1

`run()` method in the input device will be called, and the device sends reports to a global channel.

This is a simplest model, which can be used for all types of input devices. But it lacks the way to interacts with keymap & other keys. For example, this types of input device can not do multi-layer action, and it cannot be updated by vial.

```rust
/// Trait for all types of input device, which emit the HID report
pub trait InputDevice {
    /// Run the input device and send the report to a global report channel
    async fn run(&mut self);
}
```

重新考虑input device和各个设备的关系:
1. 对于各种输入设备,每种设备分别是一个trait,这个trait,包含了该设备的行为
2. 对于InputDevice,重要的是把各种设备的输出统一到一起,这样可以在同一个地方处理,方便分层.如果按照这个思路去思考InputDevice,那其实只需要一个全局channel,即可.
3. 或者,有没有什么办法能够更好地表示一下这个全局channel?
4. 对于用户,最好在接口里面是一个builder,有什么就添加什么,没有就算了.现在的join的方式不是很好.
  ```rust
  // 这里的Keyboard,相当于是一个资源的集合体.
  let mut keyboard = Keyboard::new(xxxxxx);
  keyboard.add_rotary_encoder();
  keyboard.add_slave();
  keyboard.set_slave();
  keyboard.add_master();
  keyboard.addxxx();
  keyboard.run().await;
  // Or

  run_rmk(keyboard);
  ```
  - Keyboard.add_rotary_encoder()
  - Keyboard.add_touchpad()
  - Keyboard.init() 这里应该是初始化所有的信息.也就是当前 run_rmk() 的输入
  - 存在一个问题是,所有的generics内容没有办法通过条件编译的方式来选择,那么对于某些没有usb/storage的chip来说,还得用户填写类型.
5. 还有一个方式是,仿照rumcake,每种外设设计一个proc-macro,然后每个外设都是独立的任务,这些任务share代码.

  ```rust
  rotary_encoder!(0, PA1, PA2);
  rotary_encoder!(1, PA3, PA4);
  ```
6. 如果还是比较麻烦,考虑重构整个lib,重构的点:
   1. 把Matrix和Keyboard彻底解耦开,Matrix只做扫描,然后异步发送键位到channel
   2. 各种input device,也是只做检测,然后发送metadata,比如rotary encoder就只发送cw/ccw
   3. 这些所有的事件,可以放在同一个channel中,做一个enum Message;也可以放在不同的channel中用select选择
   4. 所有的按键处理和事件处理，放在一个大的Keyboard task中.那么这个task就不只是包含keymap,也包含rotary encoder map
   5. 这样的话,每个device,都可以是一个单独的embassy_task,然后,通过channel的方式发送.也不再需要限制个数,用宏的方式,有多少外设就声明多少任务.
   6. 核心就变成了,如何在keyboard task里面把所有这些分门别类处理好.
   7. 
7. 无论哪种实现,需要解决如下几个问题:
   1. rotary encoder这类设备,需要读取到当前激活的层的状态.这个状态现在是放在keymap中的,如果拿一个&RefCell,那么device还需要COL/ROW这些const generics定义,显然是不合适的
   2. 

### Draft 2

All input devices are integrated in the `Keyboard`, with dynamic dispatch.

### Rotary encoder

The rotary encoder is different, 

For encoder, vial has separate processing, it's a unique device and has unique representation in vial.

