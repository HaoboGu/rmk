use nrf_softdevice::{
    ble::{
        gatt_server::{
            builder::ServiceBuilder,
            characteristic::{Attribute, Metadata, Properties},
            CharacteristicHandles, RegisterError,
        },
        Uuid,
    },
    Softdevice,
};

use super::spec::{BleCharacteristics, BleSpecification};

pub struct DeviceInformationService {}

impl DeviceInformationService {
    pub fn new(
        sd: &mut Softdevice,
        pnp_id: &PnPID,
        info: DeviceInformation,
    ) -> Result<Self, RegisterError> {
        let mut sb = ServiceBuilder::new(sd, BleSpecification::DeviceInformation.uuid())?;

        Self::add_pnp_characteristic(&mut sb, pnp_id)?;
        Self::add_opt_str_characteristic(
            &mut sb,
            BleCharacteristics::ManufacturerName.uuid(),
            info.manufacturer_name,
        )?;
        Self::add_opt_str_characteristic(
            &mut sb,
            BleCharacteristics::ModelNumber.uuid(),
            info.model_number,
        )?;
        Self::add_opt_str_characteristic(
            &mut sb,
            BleCharacteristics::SerialNumber.uuid(),
            info.serial_number,
        )?;
        Self::add_opt_str_characteristic(
            &mut sb,
            BleCharacteristics::HardwareRevision.uuid(),
            info.hw_rev,
        )?;
        Self::add_opt_str_characteristic(
            &mut sb,
            BleCharacteristics::FirmwareRevision.uuid(),
            info.fw_rev,
        )?;
        Self::add_opt_str_characteristic(
            &mut sb,
            BleCharacteristics::SoftwareRevision.uuid(),
            info.sw_rev,
        )?;

        let _service_handle = sb.build();

        Ok(DeviceInformationService {})
    }

    fn add_opt_str_characteristic(
        sb: &mut ServiceBuilder,
        uuid: Uuid,
        val: Option<&'static str>,
    ) -> Result<Option<CharacteristicHandles>, RegisterError> {
        if let Some(val) = val {
            let attr = Attribute::new(val);
            let md = Metadata::new(Properties::new().read());
            Ok(Some(sb.add_characteristic(uuid, attr, md)?.build()))
        } else {
            Ok(None)
        }
    }

    fn add_pnp_characteristic(
        sb: &mut ServiceBuilder,
        pnp_id: &PnPID,
    ) -> Result<CharacteristicHandles, RegisterError> {
        // SAFETY: `PnPID` is `repr(C, packed)` so viewing it as an immutable slice of bytes is safe.
        let val = unsafe {
            core::slice::from_raw_parts(
                pnp_id as *const _ as *const u8,
                core::mem::size_of::<PnPID>(),
            )
        };

        let attr = Attribute::new(val);
        let md = Metadata::new(Properties::new().read());
        Ok(sb
            .add_characteristic(BleCharacteristics::PnpId.uuid(), attr, md)?
            .build())
    }
}
