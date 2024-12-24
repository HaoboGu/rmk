//! Input device for RMK
//!

use core::future::Future;

pub mod rotary_encoder;

/// The trait for input devices.
///
/// This trait MUST be used with `impl_input_device` macro, which will automatically create a static embassy task for the input device.
/// The following is an example for the usage of this trait:
/// ```rust
/// use core::futures::Future;
/// struct MyInputDevice;
/// impl InputDevice for MyInputDevice {
///     fn run(&mut self) -> impl Future<Output = ()> {
///         // Implementation
///     }
/// }
/// impl_input_device!(MyInputDevice, my_input_device_task);
/// ```
/// The expanded code will be:
/// ```rust
/// use core::futures::Future;
/// struct MyInputDevice;
/// impl InputDevice for MyInputDevice {
///     fn run(&mut self) -> impl Future<Output = ()> {
///         // Implementation
///     }
/// }
/// #[embassy_executor::task]
/// async fn my_input_device_task(device: MyInputDevice) -> ! {
///    device.run().await
///  }
/// ```
/// Then, you can spawn `my_input_device_task` as an embassy task:
/// ```rust
/// let input_device = MyInputDevice{};
/// spawner.spawn(my_input_device_task(input_device));
/// ```
pub trait InputDevice {
    fn run(&mut self) -> impl Future<Output = ()>;
}

#[macro_export]
macro_rules! impl_input_device {
    ($device:ty, $task_name:ident) => {
        #[::embassy_executor::task]
        pub async fn $task_name(mut device: $device) -> ! {
            device.run().await;
            loop {
                // If the device.run() completes, we enter an infinite loop to satisfy
                // the `-> !` return type requirement for embassy tasks
                ::embassy_time::Timer::after_secs(1).await;
            }
        }
    };
}
