#[cfg(feature = "vial")]
pub(crate) mod vial;

#[cfg(feature = "vial")]
pub(crate) use vial::{
    BleVialTransport as BleHostTransport, HostGattWriteResult, VialGattService as HostGattService, handle_gatt_write,
};
