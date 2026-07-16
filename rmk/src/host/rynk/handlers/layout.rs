//! Layout handler — serves the opaque blob over the built-in `GetLayout`.

use heapless::Vec;
use rmk_types::protocol::rynk::command::GetLayout;
use rmk_types::protocol::rynk::{LayoutChunk, RYNK_BLE_CHUNK_SIZE, RynkError};

use super::super::RynkService;
use super::Handle;

impl Handle<GetLayout> for RynkService<'_> {
    async fn handle(&self, offset: u32) -> Result<LayoutChunk, RynkError> {
        let blob = self.ctx.layout_blob();
        let total_len = blob.len() as u32;
        let start = (offset as usize).min(blob.len());
        let end = (start + RYNK_BLE_CHUNK_SIZE).min(blob.len());
        // Page fits by construction: `end - start <= RYNK_BLE_CHUNK_SIZE`.
        let bytes = Vec::from_slice(&blob[start..end]).unwrap_or_default();
        Ok(LayoutChunk { total_len, bytes })
    }
}
