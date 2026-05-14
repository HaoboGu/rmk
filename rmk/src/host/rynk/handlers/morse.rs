//! Morse handlers.

use rmk_types::morse::Morse;
use rmk_types::protocol::rynk::{RynkError, SetMorseRequest};

use super::super::RynkService;

impl<'a> RynkService<'a> {
    pub(crate) async fn handle_get_morse(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (idx, _) = postcard::take_from_bytes::<u8>(payload).map_err(|_| RynkError::InvalidRequest)?;
        // Out-of-range slots return the empty default so the wire envelope
        // stays uniform across hits and misses.
        let morse = self.ctx.get_morse(idx).unwrap_or_else(|| Morse {
            profile: rmk_types::morse::MorseProfile::const_default(),
            actions: heapless::LinearMap::new(),
        });
        Self::write_response(&morse, payload)
    }

    pub(crate) async fn handle_set_morse(&self, payload: &mut [u8]) -> Result<usize, RynkError> {
        let (r, _) = postcard::take_from_bytes::<SetMorseRequest>(payload).map_err(|_| RynkError::InvalidRequest)?;
        self.ctx
            .update_morse(r.index, |m| {
                *m = r.config;
            })
            .await;
        Self::write_response(&(), payload)
    }

    #[cfg(feature = "bulk_transfer")]
    pub(crate) async fn handle_get_morse_bulk(&self, _payload: &mut [u8]) -> Result<usize, RynkError> {
        Err(RynkError::Internal)
    }

    #[cfg(feature = "bulk_transfer")]
    pub(crate) async fn handle_set_morse_bulk(&self, _payload: &mut [u8]) -> Result<usize, RynkError> {
        Err(RynkError::Internal)
    }
}
