//! End-to-end hardware test for `rmk-host`.
//!
//! Runs against USB by default; pass `ble` for BLE. Setters write a different
//! value, verify it, then restore the original. Exits non-zero if any command
//! hits a transport error or a verify mismatch (device rejections are logged
//! but expected for some commands, so they don't fail the run).
//!
//! ```text
//! cargo run --example hw_test            # USB CDC serial (default)
//! cargo run --example hw_test -- ble     # BLE GATT
//! ```

use log::{error, info, warn};
use rmk_host::rmk_types::morse::MorseProfile;
use rmk_host::types::{Combo, EncoderAction, MacroData, StorageResetMode};
use rmk_host::{Client, RequestError, Transport};
use rmk_host_ble::connect_ble;
use rmk_host_serial::connect_serial;

/// Log a value-returning command; count transport errors as failures.
macro_rules! report {
    ($fails:ident, $label:expr, $res:expr) => {
        match $res {
            Ok(v) => info!("  ✓ {:<22} {:?}", $label, v),
            Err(RequestError::Rejected(e)) => warn!("  ⚠ {:<22} rejected: {:?}", $label, e),
            Err(e) => {
                error!("  ✗ {:<22} {}", $label, e);
                $fails += 1;
            }
        }
    };
}

/// Log a unit-returning command; count transport errors as failures.
macro_rules! ack {
    ($fails:ident, $label:expr, $res:expr) => {
        match $res {
            Ok(()) => info!("  ✓ {}", $label),
            Err(RequestError::Rejected(e)) => warn!("  ⚠ {} rejected: {:?}", $label, e),
            Err(e) => {
                error!("  ✗ {} {}", $label, e);
                $fails += 1;
            }
        }
    };
}

/// Re-read after a set and compare; count mismatches and errors as failures.
macro_rules! verify {
    ($fails:ident, $label:expr, $expected:expr, $reget:expr) => {
        match $reget.await {
            Ok(actual) if actual == $expected => info!("  ↺ {:<18} {:?} == {:?}", $label, $expected, actual),
            Ok(actual) => {
                error!("  ✗ {:<18} {:?} != {:?}", $label, $expected, actual);
                $fails += 1;
            }
            Err(e) => {
                error!("  ✗ {:<18} {}", $label, e);
                $fails += 1;
            }
        }
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_target(false)
        .init();

    match std::env::args().nth(1).as_deref().unwrap_or("usb") {
        "usb" => {
            info!("connecting over USB CDC serial…");
            run_all(connect_serial().await?, false).await
        }
        "ble" => {
            info!("connecting over BLE GATT…");
            run_all(connect_ble().await?, true).await
        }
        other => Err(format!("unknown transport {other:?}; use 'usb' (default) or 'ble'").into()),
    }
}

/// Exercise non-reboot Rynk commands.
async fn run_all<T: Transport>(mut client: Client<T>, over_ble: bool) -> Result<(), Box<dyn std::error::Error>> {
    let caps = *client.capabilities();
    let mut fails = 0u32;
    info!(
        "connected over {} (protocol v{}.{}) — keymap {}×{}×{}, {} combos, {} forks, {} morse, ble={}",
        if over_ble { "BLE" } else { "USB" },
        client.protocol_version().major,
        client.protocol_version().minor,
        caps.num_layers,
        caps.num_rows,
        caps.num_cols,
        caps.max_combos,
        caps.max_forks,
        caps.max_morse,
        caps.ble_enabled,
    );

    info!("── system ──");
    report!(fails, "get_version", client.get_version().await);
    report!(fails, "get_capabilities", client.get_capabilities().await);

    info!("── keymap ──");
    // Read, set a different value, verify, restore.
    match client.get_default_layer().await {
        // Only round-trip an in-range value: restoring an out-of-range `orig`
        // would be rejected by the firmware, fail the verify spuriously, and
        // leave the device on the test's mutated layer.
        Ok(orig) if (orig as usize) < caps.num_layers as usize => {
            info!("  ✓ {:<22} {orig}", "get default_layer");
            let new = orig.wrapping_add(1) % caps.num_layers.max(1);
            if new != orig {
                ack!(
                    fails,
                    format!("set default_layer ={new}"),
                    client.set_default_layer(new).await
                );
                verify!(fails, "default_layer", new, client.get_default_layer());
                ack!(
                    fails,
                    format!("restore default_layer ={orig}"),
                    client.set_default_layer(orig).await
                );
                verify!(fails, "default_layer", orig, client.get_default_layer());
            } else {
                ack!(fails, "set default_layer", client.set_default_layer(orig).await);
            }
        }
        Ok(orig) => warn!(
            "  ⚠ {:<20} out-of-range default_layer {orig} (num_layers={}); skipping round-trip",
            "get default_layer", caps.num_layers
        ),
        other => report!(fails, "get default_layer", other),
    }
    // Read the whole keymap by walking the single-key endpoint.
    let mut total = 0usize;
    let mut scan_error = None;
    'scan: for layer in 0..caps.num_layers {
        for row in 0..caps.num_rows {
            for col in 0..caps.num_cols {
                match client.get_key(layer, row, col).await {
                    Ok(_) => total += 1,
                    Err(e) => {
                        scan_error = Some(e);
                        break 'scan;
                    }
                }
            }
        }
    }
    if let Some(e) = scan_error {
        error!("  ✗ scan keymap {e}");
        fails += 1;
    } else {
        info!(
            "  ✓ {:<22} {total} keys across {} layers",
            "scan keymap", caps.num_layers
        );
    }
    // Set L0(0,0), verify, restore.
    let key_00 = client.get_key(0, 0, 0).await;
    let key_01 = client.get_key(0, 0, 1).await;
    match (key_00, key_01) {
        (Ok(orig), Ok(alt)) if alt != orig => {
            info!("  ✓ {:<22} {orig:?}", "get key L0(0,0)");
            ack!(
                fails,
                format!("set key L0(0,0) ={alt:?}"),
                client.set_key(0, 0, 0, alt).await
            );
            verify!(fails, "key L0(0,0)", alt, client.get_key(0, 0, 0));
            ack!(fails, "restore key L0(0,0)", client.set_key(0, 0, 0, orig).await);
            verify!(fails, "key L0(0,0)", orig, client.get_key(0, 0, 0));
        }
        (Ok(orig), _) => {
            info!("  ✓ {:<22} {orig:?}", "get key L0(0,0)");
            ack!(fails, "set key L0(0,0)", client.set_key(0, 0, 0, orig).await);
        }
        (other, _) => report!(fails, "get key L0(0,0)", other),
    }
    // Encoders: swap rotation actions, verify, restore.
    if caps.num_encoders == 0 {
        info!("  (no encoders configured — exercising dispatch at index 0)");
    }
    match client.get_encoder(0, 0).await {
        Ok(orig) => {
            info!("  ✓ {:<22} {orig:?}", "get_encoder 0/L0");
            let changed = EncoderAction::new(orig.counter_clockwise, orig.clockwise);
            if changed != orig {
                ack!(
                    fails,
                    "set_encoder 0/L0 (swap)",
                    client.set_encoder(0, 0, changed).await
                );
                verify!(fails, "encoder 0/L0", changed, client.get_encoder(0, 0));
                ack!(fails, "restore encoder 0/L0", client.set_encoder(0, 0, orig).await);
                verify!(fails, "encoder 0/L0", orig, client.get_encoder(0, 0));
            } else {
                ack!(
                    fails,
                    "set_encoder 0/L0 (write-back)",
                    client.set_encoder(0, 0, orig).await
                );
            }
        }
        other => {
            report!(fails, "get_encoder 0/L0", other);
            ack!(
                fails,
                "set_encoder 0/L0 (dispatch only)",
                client.set_encoder(0, 0, EncoderAction::default()).await
            );
        }
    }

    info!("── macros ──");
    if caps.max_macros == 0 {
        info!("  (no macro slots — exercising dispatch at index 0)");
    }
    report!(fails, "get_macro 0", client.get_macro(0, 0).await);
    ack!(
        fails,
        "set_macro 0",
        client
            .set_macro(
                0,
                0,
                MacroData {
                    data: Default::default()
                }
            )
            .await
    );

    info!("── combos ──");
    // Combo: set a different valid combo, verify, restore.
    if caps.max_combos > 0 {
        match client.get_combo(0).await {
            Ok(orig) => {
                info!("  ✓ {:<22} {orig:?}", "get combo 0");
                let k0 = client.get_key(0, 0, 0).await.ok();
                let k1 = client.get_key(0, 1, 0).await.ok();
                if let (Some(k0), Some(k1)) = (k0, k1) {
                    let mut actions = heapless::Vec::new();
                    let _ = actions.push(k0);
                    let _ = actions.push(k1);
                    let changed = Combo {
                        actions,
                        output: k0,
                        layer: Some(1),
                    };
                    ack!(
                        fails,
                        "set combo 0 (mutate)",
                        client.set_combo(0, changed.clone()).await
                    );
                    verify!(fails, "combo 0", changed, client.get_combo(0));
                    ack!(fails, "restore combo 0", client.set_combo(0, orig.clone()).await);
                    verify!(fails, "combo 0", orig, client.get_combo(0));
                } else {
                    ack!(
                        fails,
                        "set combo 0 (write-back)",
                        client.set_combo(0, orig.clone()).await
                    );
                }
            }
            other => report!(fails, "get combo 0", other),
        }
    }

    info!("── forks ──");
    // Fork: mutate one output, verify, restore.
    if caps.max_forks > 0 {
        match client.get_fork(0).await {
            Ok(orig) => {
                info!("  ✓ {:<22} {orig:?}", "get fork 0");
                let key = client.get_key(0, 0, 1).await.ok();
                match key {
                    Some(key) if key != orig.positive_output => {
                        let mut changed = orig;
                        changed.positive_output = key;
                        ack!(
                            fails,
                            format!("set fork 0 (positive_output={key:?})"),
                            client.set_fork(0, changed).await
                        );
                        verify!(fails, "fork 0", changed, client.get_fork(0));
                        ack!(fails, "restore fork 0", client.set_fork(0, orig).await);
                        verify!(fails, "fork 0", orig, client.get_fork(0));
                    }
                    _ => ack!(fails, "set fork 0 (write-back)", client.set_fork(0, orig).await),
                }
            }
            other => report!(fails, "get fork 0", other),
        }
    }

    info!("── morse ──");
    // Morse: mutate hold timeout, verify, restore.
    if caps.max_morse > 0 {
        match client.get_morse(0).await {
            Ok(orig) => {
                info!("  ✓ {:<22} {orig:?}", "get morse 0");
                let mut changed = orig.clone();
                changed.profile = MorseProfile::const_default().with_hold_timeout_ms(Some(180));
                ack!(
                    fails,
                    "set morse 0 (mutate)",
                    client.set_morse(0, changed.clone()).await
                );
                match client.get_morse(0).await {
                    // `Morse` has no `PartialEq`.
                    Ok(actual) if format!("{changed:?}") == format!("{actual:?}") => {
                        info!("  ↺ {:<18} {:?} == {:?}", "morse 0", changed.profile, actual.profile)
                    }
                    Ok(actual) => {
                        error!("  ✗ {:<18} {:?} != {:?}", "morse 0", changed, actual);
                        fails += 1;
                    }
                    other => report!(fails, "morse 0", other),
                }
                ack!(fails, "restore morse 0", client.set_morse(0, orig).await);
            }
            other => report!(fails, "get morse 0", other),
        }
    }

    info!("── behavior ──");
    match client.get_behavior().await {
        Ok(orig) => {
            info!("  ✓ {:<22} {orig:?}", "get behavior");
            let mut changed = orig;
            changed.combo_timeout_ms = orig.combo_timeout_ms.wrapping_add(5);
            ack!(
                fails,
                format!("set behavior (combo_timeout={})", changed.combo_timeout_ms),
                client.set_behavior(changed).await
            );
            verify!(fails, "behavior", changed, client.get_behavior());
            ack!(fails, "restore behavior", client.set_behavior(orig).await);
            verify!(fails, "behavior", orig, client.get_behavior());
        }
        other => report!(fails, "get behavior", other),
    }

    info!("── status ──");
    report!(fails, "get_current_layer", client.get_current_layer().await);
    report!(fails, "get_matrix_state", client.get_matrix_state().await);
    report!(fails, "get_wpm", client.get_wpm().await);
    report!(fails, "get_sleep_state", client.get_sleep_state().await);
    report!(fails, "get_led_indicator", client.get_led_indicator().await);
    // Gate feature-specific status commands on capabilities.
    if caps.ble_enabled {
        report!(fails, "get_battery_status", client.get_battery_status().await);
    } else {
        info!("  (no BLE — skipping get_battery_status)");
    }
    if caps.is_split && caps.ble_enabled {
        for slot in 0..caps.num_split_peripherals {
            report!(
                fails,
                format!("get_peripheral_status {slot}"),
                client.get_peripheral_status(slot).await
            );
        }
    } else {
        info!("  (not a split-BLE keyboard — skipping get_peripheral_status)");
    }

    info!("── connection ──");
    report!(fails, "get_connection_type", client.get_connection_type().await);
    report!(fails, "get_connection_status", client.get_connection_status().await);
    if caps.ble_enabled {
        match client.get_ble_status().await {
            Ok(s) => {
                info!("  ✓ {:<22} {:?}", "get_ble_status", s);
                // Re-selecting the active profile is a no-op.
                ack!(
                    fails,
                    format!("switch_ble_profile {} (already active)", s.profile),
                    client.switch_ble_profile(s.profile).await
                );
            }
            other => {
                report!(fails, "get_ble_status", other);
                info!("  (active profile unknown — skipping switch_ble_profile)");
            }
        }
        // Clearing bonds is not restorable.
        info!("  (skipping clear_ble_profile: deleting a bond is unrestorable)");
    } else {
        info!("  (no BLE — skipping ble_status / switch / clear profile)");
    }

    info!("── storage ──");
    if over_ble {
        // A storage wipe includes bonds.
        info!("  (over BLE — skipping storage_reset: a wipe would drop this BLE link)");
    } else {
        // Exercise the command path without wiping user data.
        ack!(
            fails,
            "storage_reset(LayoutOnly) [expect Unimplemented until mode-aware reset lands]",
            client.storage_reset(StorageResetMode::LayoutOnly).await
        );
    }

    info!("── topics (best-effort drain) ──");
    // Drain queued topics and brief late arrivals.
    let mut topic_count = 0;
    while let Ok(Ok(event)) = tokio::time::timeout(std::time::Duration::from_millis(300), client.next_event()).await {
        info!("  ⤷ topic {event:?}");
        topic_count += 1;
    }
    if topic_count == 0 {
        info!("  (none received — topics are event-driven, e.g. key presses)");
    }

    if fails == 0 {
        info!(
            "done — command sweep passed over {}.",
            if over_ble { "BLE" } else { "USB" }
        );
        Ok(())
    } else {
        Err(format!("{fails} command(s) failed").into())
    }
}
