//! Keymap endpoints — keys, default layer, encoders.

use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::protocol::rynk::{Cmd, GetEncoderRequest, KeyPosition, RynkResult, SetEncoderRequest, SetKeyRequest};

use crate::transport::{Transport, TransportError};

/// Read one key's action.
pub async fn get_key<T: Transport>(t: &mut T, layer: u8, row: u8, col: u8) -> Result<KeyAction, TransportError> {
    let pos = KeyPosition { layer, row, col };
    t.request::<KeyPosition, KeyAction>(Cmd::GetKeyAction, &pos).await
}

/// Write one key's action and persist it to flash. Returns the device's
/// validation result.
pub async fn set_key<T: Transport>(
    t: &mut T,
    layer: u8,
    row: u8,
    col: u8,
    action: KeyAction,
) -> Result<RynkResult, TransportError> {
    let req = SetKeyRequest {
        position: KeyPosition { layer, row, col },
        action,
    };
    t.request::<SetKeyRequest, RynkResult>(Cmd::SetKeyAction, &req).await
}

/// Read the currently selected default layer index.
pub async fn get_default_layer<T: Transport>(t: &mut T) -> Result<u8, TransportError> {
    t.request::<(), u8>(Cmd::GetDefaultLayer, &()).await
}

/// Set the default layer.
pub async fn set_default_layer<T: Transport>(t: &mut T, layer: u8) -> Result<RynkResult, TransportError> {
    t.request::<u8, RynkResult>(Cmd::SetDefaultLayer, &layer).await
}

/// Read both rotation actions for one encoder on one layer.
pub async fn get_encoder<T: Transport>(t: &mut T, encoder_id: u8, layer: u8) -> Result<EncoderAction, TransportError> {
    let req = GetEncoderRequest { encoder_id, layer };
    t.request::<GetEncoderRequest, EncoderAction>(Cmd::GetEncoderAction, &req)
        .await
}

/// Set both rotation actions for one encoder on one layer.
pub async fn set_encoder<T: Transport>(
    t: &mut T,
    encoder_id: u8,
    layer: u8,
    action: EncoderAction,
) -> Result<RynkResult, TransportError> {
    let req = SetEncoderRequest {
        encoder_id,
        layer,
        action,
    };
    t.request::<SetEncoderRequest, RynkResult>(Cmd::SetEncoderAction, &req)
        .await
}
