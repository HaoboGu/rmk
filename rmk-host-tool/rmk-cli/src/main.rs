//! `rmk-cli` — interactive command-line client for the RMK protocol.
//!
//! Subcommands:
//! * `info` — print version + capabilities
//! * `dump-keymap` — print the entire keymap
//! * `set-key <layer> <row> <col> <keycode>` — set a single key action
//! * `bootloader` — reboot into the bootloader
//! * `reset` — soft-reset the keyboard
//! * `monitor layers` — stream layer-change events
//!
//! No `lock` / `unlock` subcommands in v1: the firmware-side lock gate is
//! deferred to v2 (plan §3.7), so a CLI surface for it would silently no-op.

use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rmk_host::Client;
use rmk_types::action::KeyAction;
use rmk_types::protocol::rmk::KeyPosition;

#[derive(Copy, Clone, Debug, ValueEnum, Default)]
enum Transport {
    #[default]
    Usb,
    Ble,
}

#[derive(Parser, Debug)]
#[command(version, about = "RMK protocol client")]
struct Cli {
    /// Which transport to use to reach the keyboard.
    #[arg(long, value_enum, default_value_t = Transport::Usb)]
    transport: Transport,
    /// USB Vendor ID to match (USB only). Hex with optional 0x prefix.
    #[arg(long, default_value = "0xc0de", value_parser = parse_hex_u16)]
    vid: u16,
    /// USB Product ID to match (USB only). Hex with optional 0x prefix.
    #[arg(long, default_value = "0xcafe", value_parser = parse_hex_u16)]
    pid: u16,
    /// BLE scan timeout in seconds (BLE only).
    #[arg(long, default_value_t = 10)]
    ble_scan_secs: u64,
    /// Substring of the advertised BLE local name to match (BLE only).
    #[arg(long, default_value = "RMK")]
    ble_name: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List every USB device visible to nusb (debug helper).
    ListDevices,
    /// List every BLE adv visible to bluest for `scan_secs` seconds (debug helper).
    BleScan {
        #[arg(long, default_value_t = 8)]
        scan_secs: u64,
    },
    /// Connect to every device whose RSSI is at least `min_rssi` and probe for
    /// the rmk_protocol GATT service. Useful when local_name isn't surfaced.
    BleProbe {
        #[arg(long, default_value_t = 12)]
        scan_secs: u64,
        #[arg(long, default_value_t = -70)]
        min_rssi: i16,
    },
    /// Print protocol version and device capabilities.
    Info,
    /// Print the entire keymap.
    DumpKeymap,
    /// Set a single key action: layer/row/col + keycode (raw u32 KeyAction bits).
    SetKey {
        layer: u8,
        row: u8,
        col: u8,
        /// `KeyAction` to set, in postcard JSON form. Until a friendly parser
        /// lands this accepts only `no` (KeyAction::No).
        #[arg(value_parser = parse_key_action)]
        action: KeyAction,
    },
    /// Reboot the keyboard into its bootloader.
    Bootloader,
    /// Soft-reset the keyboard.
    Reset,
    /// Stream events.
    Monitor {
        #[command(subcommand)]
        what: MonitorWhat,
    },
}

#[derive(Subcommand, Debug)]
enum MonitorWhat {
    /// Stream layer change events.
    Layers,
}

fn parse_hex_u16(s: &str) -> std::result::Result<u16, String> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(s, 16).map_err(|e| e.to_string())
}

fn parse_key_action(s: &str) -> std::result::Result<KeyAction, String> {
    match s {
        "no" => Ok(KeyAction::No),
        other => Err(format!(
            "Only `no` accepted in v1; got `{other}`. Friendly KeyAction parsing is a follow-up."
        )),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Debug-helper subcommands skip the connection step.
    match &cli.command {
        Command::ListDevices => return list_devices(),
        Command::BleScan { scan_secs } => return ble_scan(*scan_secs).await,
        Command::BleProbe { scan_secs, min_rssi } => return ble_probe(*scan_secs, *min_rssi).await,
        _ => {}
    }

    let client = match cli.transport {
        Transport::Usb => Client::connect_usb(|d| d.vendor_id() == cli.vid && d.product_id() == cli.pid)
            .await
            .context("failed to connect to keyboard over USB")?,
        Transport::Ble => Client::connect_ble(
            Duration::from_secs(cli.ble_scan_secs),
            Some(cli.ble_name.as_str()),
        )
        .await
        .context("failed to connect to keyboard over BLE")?,
    };

    match cli.command {
        Command::ListDevices | Command::BleScan { .. } | Command::BleProbe { .. } => {
            unreachable!("handled above")
        }
        Command::Info => print_info(&client).await?,
        Command::DumpKeymap => dump_keymap(&client).await?,
        Command::SetKey {
            layer,
            row,
            col,
            action,
        } => {
            client
                .set_key_action(KeyPosition { layer, row, col }, action)
                .await
                .context("set_key_action")?;
            println!("OK");
        }
        Command::Bootloader => client.bootloader_jump().await?,
        Command::Reset => client.reboot().await?,
        Command::Monitor {
            what: MonitorWhat::Layers,
        } => monitor_layers(&client).await?,
    }
    Ok(())
}

async fn ble_probe(scan_secs: u64, min_rssi: i16) -> Result<()> {
    use std::collections::HashMap;

    use bluest::Adapter;
    use futures::StreamExt;
    use rmk_host::ble::RMK_PROTOCOL_SERVICE_UUID;

    let adapter = Adapter::default()
        .await
        .ok_or_else(|| anyhow::anyhow!("no default BLE adapter"))?;
    adapter.wait_available().await?;

    // Collect unique candidate devices first.
    let mut stream = adapter.scan(&[]).await?;
    println!("scanning {scan_secs}s for devices with RSSI ≥ {min_rssi}…");
    let mut candidates: HashMap<bluest::DeviceId, (bluest::Device, i16)> = HashMap::new();
    let _ = tokio::time::timeout(Duration::from_secs(scan_secs), async {
        while let Some(adv) = stream.next().await {
            let rssi = adv.rssi.unwrap_or(i16::MIN);
            if rssi >= min_rssi {
                candidates.insert(adv.device.id(), (adv.device, rssi));
            }
        }
    })
    .await;
    drop(stream);

    println!("probing {} candidate device(s)…", candidates.len());
    for (dev, rssi) in candidates.into_values() {
        let label = dev
            .name()
            .ok()
            .unwrap_or_else(|| format!("{:?}", dev.id()));
        print!("  rssi={rssi:>4} {label} → ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        // Per-device cap so a single misbehaving device doesn't stall the run.
        let probe = async {
            adapter.connect_device(&dev).await?;
            let services = dev.discover_services().await?;
            Ok::<_, bluest::Error>(services)
        };
        match tokio::time::timeout(Duration::from_secs(6), probe).await {
            Err(_) => println!("timed out"),
            Ok(Err(e)) => println!("error: {e:?}"),
            Ok(Ok(services)) => {
                let has_ours = services.iter().any(|s| s.uuid() == RMK_PROTOCOL_SERVICE_UUID);
                println!(
                    "{} services, rmk_protocol = {}",
                    services.len(),
                    if has_ours { "FOUND" } else { "no" }
                );
                if has_ours {
                    for s in &services {
                        println!("    service {}", s.uuid());
                    }
                }
            }
        }
        let _ = adapter.disconnect_device(&dev).await;
    }
    Ok(())
}

async fn ble_scan(scan_secs: u64) -> Result<()> {
    use bluest::Adapter;
    use futures::StreamExt;

    let adapter = Adapter::default()
        .await
        .ok_or_else(|| anyhow::anyhow!("no default BLE adapter"))?;
    adapter.wait_available().await?;

    let mut stream = adapter.scan(&[]).await?;
    println!("scanning for {scan_secs}s…");
    let _ = tokio::time::timeout(Duration::from_secs(scan_secs), async move {
        while let Some(adv) = stream.next().await {
            let name = adv
                .adv_data
                .local_name
                .as_deref()
                .unwrap_or("?")
                .to_string();
            let services: Vec<String> = adv.adv_data.services.iter().map(|u| u.to_string()).collect();
            let rssi = adv.rssi.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
            println!(
                "{:<28} rssi={:>4} services=[{}]",
                name,
                rssi,
                services.join(", ")
            );
        }
    })
    .await;
    Ok(())
}

fn list_devices() -> Result<()> {
    use rmk_host::nusb;
    println!("{:<6} {:<6} {:<8} {}", "VID", "PID", "CLASS", "PRODUCT");
    for d in nusb::list_devices().context("nusb::list_devices")? {
        let class = d.class();
        println!(
            "{:#06x} {:#06x} {:#04x}     {} ({})",
            d.vendor_id(),
            d.product_id(),
            class,
            d.product_string().unwrap_or("?"),
            d.manufacturer_string().unwrap_or("?"),
        );
    }
    Ok(())
}

async fn print_info(client: &Client) -> Result<()> {
    let version = client.get_version().await?;
    let caps = client.capabilities();
    println!("protocol version: {}.{}", version.major, version.minor);
    println!(
        "layout: {} layers x {} rows x {} cols",
        caps.num_layers, caps.num_rows, caps.num_cols
    );
    println!(
        "features: storage={} ble={} split={} bulk={}",
        caps.storage_enabled, caps.ble_enabled, caps.is_split, caps.bulk_transfer_supported
    );
    Ok(())
}

async fn dump_keymap(client: &Client) -> Result<()> {
    let caps = client.capabilities();
    for layer in 0..caps.num_layers {
        println!("# layer {layer}");
        for row in 0..caps.num_rows {
            for col in 0..caps.num_cols {
                let action = client
                    .get_key_action(KeyPosition { layer, row, col })
                    .await?;
                print!("({row},{col})={action:?} ");
            }
            println!();
        }
    }
    Ok(())
}

async fn monitor_layers(client: &Client) -> Result<()> {
    let mut sub = client
        .subscribe_layer_changes(8)
        .await
        .context("subscribe layer changes")?;
    println!("Listening for layer changes... (Ctrl+C to quit)");
    loop {
        match sub.recv().await {
            Ok(layer) => println!("layer = {layer}"),
            Err(e) => {
                eprintln!("subscription closed: {e:?}");
                break;
            }
        }
    }
    Ok(())
}
