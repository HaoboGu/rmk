//! Behavior-config handlers (combo timeout, one-shot timeout, tap intervals).

use rmk_types::protocol::rynk::command::{GetBehaviorConfig, SetBehaviorConfig};
use rmk_types::protocol::rynk::{BehaviorConfig, RynkError};

use super::super::RynkService;
use super::Handle;

impl Handle<GetBehaviorConfig> for RynkService<'_> {
    async fn handle(&self, _: ()) -> Result<BehaviorConfig, RynkError> {
        Ok(BehaviorConfig {
            combo_timeout_ms: u32::try_from(self.ctx.combo_timeout().as_millis()).unwrap_or(u32::MAX),
            oneshot_timeout_ms: u32::try_from(self.ctx.one_shot_timeout().as_millis()).unwrap_or(u32::MAX),
            tap_interval_ms: self.ctx.tap_interval(),
            tap_capslock_interval_ms: self.ctx.tap_capslock_interval(),
        })
    }
}

impl Handle<SetBehaviorConfig> for RynkService<'_> {
    async fn handle(&self, cfg: BehaviorConfig) -> Result<(), RynkError> {
        self.ctx.set_combo_timeout(cfg.combo_timeout_ms).await;
        self.ctx.set_one_shot_timeout(cfg.oneshot_timeout_ms).await;
        self.ctx.set_tap_interval(cfg.tap_interval_ms).await;
        self.ctx.set_tap_capslock_interval(cfg.tap_capslock_interval_ms).await;
        Ok(())
    }
}
