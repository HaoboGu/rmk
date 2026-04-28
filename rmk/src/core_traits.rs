/// The trait for runnable input devices and processors.
///
/// For some input devices or processors, they should keep running in a separate task.
/// This trait is used to run them in a separate task.
pub trait Runnable {
    async fn run(&mut self) -> !;
}
