use core::marker::PhantomData;
#[derive(Clone, Copy, Debug, Default)]
pub struct BleBatteryConfig<'a> {
    _marker: PhantomData<&'a ()>,
}
