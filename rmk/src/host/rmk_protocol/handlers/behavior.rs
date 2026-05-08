//! Handlers for the `behavior/*` endpoint group.

use embassy_time::Duration;
use postcard_rpc::header::VarHeader;
use rmk_types::protocol::rmk::{BehaviorConfig, RmkResult};

use super::super::Ctx;

pub(crate) async fn get_behavior_config(ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> BehaviorConfig {
    BehaviorConfig {
        combo_timeout_ms: ctx.keymap.combo_timeout().as_millis() as u16,
        oneshot_timeout_ms: ctx.keymap.one_shot_timeout().as_millis() as u16,
        tap_interval_ms: ctx.keymap.tap_interval(),
        tap_capslock_interval_ms: ctx.keymap.tap_capslock_interval(),
    }
}

pub(crate) async fn set_behavior_config(ctx: &mut Ctx<'_>, _hdr: VarHeader, cfg: BehaviorConfig) -> RmkResult {
    ctx.keymap
        .set_combo_timeout(Duration::from_millis(cfg.combo_timeout_ms as u64));
    ctx.keymap
        .set_one_shot_timeout(Duration::from_millis(cfg.oneshot_timeout_ms as u64));
    ctx.keymap.set_tap_interval(cfg.tap_interval_ms);
    ctx.keymap.set_tap_capslock_interval(cfg.tap_capslock_interval_ms);
    #[cfg(feature = "storage")]
    {
        let ch = &crate::channel::FLASH_CHANNEL;
        ch.send(crate::storage::FlashOperationMessage::ComboTimeout(
            cfg.combo_timeout_ms,
        ))
        .await;
        ch.send(crate::storage::FlashOperationMessage::OneShotTimeout(
            cfg.oneshot_timeout_ms,
        ))
        .await;
        ch.send(crate::storage::FlashOperationMessage::TapInterval(cfg.tap_interval_ms))
            .await;
        ch.send(crate::storage::FlashOperationMessage::TapCapslockInterval(
            cfg.tap_capslock_interval_ms,
        ))
        .await;
    }
    Ok(())
}
