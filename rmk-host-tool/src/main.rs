use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use nusb::{DeviceInfo, InterfaceInfo};
use postcard_rpc::header::VarSeqKind;
use postcard_rpc::host_client::{HostClient, HostErr};
use postcard_rpc::standard_icd::{ERROR_PATH, WireError};
use rmk_types::protocol::rmk::{
    BulkRequest, GetCapabilities, GetDefaultLayer, GetKeyAction, GetKeymapBulk, GetLayerCount,
    GetLockStatus, GetVersion, KeyPosition, Reboot, ResetKeymap, SetDefaultLayer, SetKeyAction,
    SetKeyRequest, StorageReset, StorageResetMode,
};
use rmk_types::action::{Action, KeyAction};
use rmk_types::keycode::{HidKeyCode, KeyCode};

const DEFAULT_OUTGOING_DEPTH: usize = 8;
const VENDOR_INTERFACE_CLASS: u8 = 0xFF;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List matching USB devices and interfaces.
    List(DeviceArgs),
    /// Connect over raw USB bulk and run the RMK handshake.
    Handshake(HandshakeArgs),
    /// Get a single key action from the keymap.
    GetKey(GetKeyArgs),
    /// Set a single key action in the keymap.
    SetKey(SetKeyArgs),
    /// Dump the entire keymap from the device.
    DumpKeymap(ConnectArgs),
    /// Get lock status.
    GetLockStatus(ConnectArgs),
    /// Get default layer.
    GetDefaultLayer(ConnectArgs),
    /// Set default layer.
    SetDefaultLayer(SetDefaultLayerArgs),
    /// Reboot the device.
    Reboot(ConnectArgs),
    /// Reset keymap to defaults (erases storage and reboots).
    ResetKeymap(ConnectArgs),
    /// Reset storage (erases storage and reboots).
    StorageReset(StorageResetArgs),
}

#[derive(Args, Debug, Clone)]
struct DeviceArgs {
    /// Filter by vendor ID, decimal or hex like 0x1209.
    #[arg(long, value_parser = parse_u16)]
    vid: Option<u16>,
    /// Filter by product ID, decimal or hex like 0x0001.
    #[arg(long, value_parser = parse_u16)]
    pid: Option<u16>,
    /// Filter by serial number.
    #[arg(long)]
    serial: Option<String>,
    /// Select a specific USB interface number.
    #[arg(long)]
    interface_number: Option<u8>,
}

#[derive(Args, Debug, Clone)]
struct ConnectArgs {
    #[command(flatten)]
    device: DeviceArgs,
    /// Outgoing queue depth for postcard-rpc host client.
    #[arg(long, default_value_t = DEFAULT_OUTGOING_DEPTH)]
    outgoing_depth: usize,
    /// Sequence-number width used by the host client.
    #[arg(long, value_enum, default_value_t = SeqKindArg::Seq2)]
    seq_kind: SeqKindArg,
}

#[derive(Args, Debug, Clone)]
struct HandshakeArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Fail if `GetCapabilities` is not implemented yet.
    #[arg(long)]
    require_capabilities: bool,
}

#[derive(Args, Debug, Clone)]
struct GetKeyArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Layer number.
    #[arg(long)]
    layer: u8,
    /// Row number.
    #[arg(long)]
    row: u8,
    /// Column number.
    #[arg(long)]
    col: u8,
}

#[derive(Args, Debug, Clone)]
struct SetKeyArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Layer number.
    #[arg(long)]
    layer: u8,
    /// Row number.
    #[arg(long)]
    row: u8,
    /// Column number.
    #[arg(long)]
    col: u8,
    /// HID keycode value (e.g. 0x04 for KeyA). Decimal or hex with 0x prefix.
    #[arg(long, value_parser = parse_u16)]
    keycode: u16,
}

#[derive(Args, Debug, Clone)]
struct SetDefaultLayerArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Layer number to set as default.
    #[arg(long)]
    layer: u8,
}

#[derive(Args, Debug, Clone)]
struct StorageResetArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Reset mode: "full" erases everything, "layout" erases only layout data.
    #[arg(long, default_value = "full")]
    mode: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum SeqKindArg {
    Seq1,
    Seq2,
    Seq4,
}

impl From<SeqKindArg> for VarSeqKind {
    fn from(value: SeqKindArg) -> Self {
        match value {
            SeqKindArg::Seq1 => VarSeqKind::Seq1,
            SeqKindArg::Seq2 => VarSeqKind::Seq2,
            SeqKindArg::Seq4 => VarSeqKind::Seq4,
        }
    }
}

#[derive(Clone)]
struct SelectedDevice {
    device: DeviceInfo,
    interface_number: u8,
    interface_info: Option<InterfaceInfo>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::List(args) => list_devices(&args),
        Command::Handshake(args) => handshake(args).await,
        Command::GetKey(args) => get_key(args).await,
        Command::SetKey(args) => set_key(args).await,
        Command::DumpKeymap(args) => dump_keymap(args).await,
        Command::GetLockStatus(args) => get_lock_status(args).await,
        Command::GetDefaultLayer(args) => get_default_layer(args).await,
        Command::SetDefaultLayer(args) => set_default_layer(args).await,
        Command::Reboot(args) => reboot(args).await,
        Command::ResetKeymap(args) => reset_keymap(args).await,
        Command::StorageReset(args) => storage_reset(args).await,
    }
}

fn list_devices(args: &DeviceArgs) -> Result<()> {
    let devices = matching_devices(args)?;
    if devices.is_empty() {
        println!("No matching USB devices found.");
        return Ok(());
    }

    for (index, device) in devices.iter().enumerate() {
        print_device_summary(index, device);
        let interfaces: Vec<_> = device.interfaces().collect();
        if interfaces.is_empty() {
            println!("  interfaces: <none exposed by OS>");
        } else {
            for interface in interfaces {
                println!(
                    "  interface {} class=0x{:02X} subclass=0x{:02X} protocol=0x{:02X}{}",
                    interface.interface_number(),
                    interface.class(),
                    interface.subclass(),
                    interface.protocol(),
                    interface
                        .interface_string()
                        .map(|s| format!(" string={s:?}"))
                        .unwrap_or_default(),
                );
            }
        }
    }

    Ok(())
}

fn connect(args: &ConnectArgs) -> Result<HostClient<WireError>> {
    let selected = select_device(&args.device)?;
    print_selected_device(&selected);

    HostClient::<WireError>::try_from_nusb_and_interface(
        &selected.device,
        selected.interface_number as usize,
        ERROR_PATH,
        args.outgoing_depth,
        args.seq_kind.into(),
    )
    .map_err(|err| anyhow!(err))
}

async fn handshake(args: HandshakeArgs) -> Result<()> {
    let client = connect(&args.connect)?;

    let version = client
        .send_resp::<GetVersion>(&())
        .await
        .context("GetVersion request failed")?;

    println!("protocol.version={}.{}", version.major, version.minor);

    match client.send_resp::<GetCapabilities>(&()).await {
        Ok(capabilities) => {
            println!("capabilities.num_layers={}", capabilities.num_layers);
            println!("capabilities.num_rows={}", capabilities.num_rows);
            println!("capabilities.num_cols={}", capabilities.num_cols);
            println!("capabilities.num_encoders={}", capabilities.num_encoders);
            println!("capabilities.max_combos={}", capabilities.max_combos);
            println!("capabilities.max_macros={}", capabilities.max_macros);
            println!("capabilities.macro_space_size={}", capabilities.macro_space_size);
            println!("capabilities.max_morse={}", capabilities.max_morse);
            println!("capabilities.max_forks={}", capabilities.max_forks);
            println!("capabilities.has_storage={}", capabilities.has_storage);
            println!("capabilities.has_split={}", capabilities.has_split);
            println!(
                "capabilities.num_split_peripherals={}",
                capabilities.num_split_peripherals
            );
            println!("capabilities.has_ble={}", capabilities.has_ble);
            println!("capabilities.num_ble_profiles={}", capabilities.num_ble_profiles);
            println!("capabilities.has_lighting={}", capabilities.has_lighting);
            println!("capabilities.max_payload_size={}", capabilities.max_payload_size);
        }
        Err(HostErr::Wire(WireError::UnknownKey)) => {
            if args.require_capabilities {
                bail!("GetCapabilities is not implemented on the connected firmware yet");
            }
            println!("capabilities=unsupported (firmware replied WireError::UnknownKey)");
        }
        Err(err) => return Err(err).context("GetCapabilities request failed"),
    }

    Ok(())
}

async fn get_key(args: GetKeyArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    let pos = KeyPosition {
        layer: args.layer,
        row: args.row,
        col: args.col,
    };
    let action = client
        .send_resp::<GetKeyAction>(&pos)
        .await
        .context("GetKeyAction request failed")?;
    println!("key[{},{},{}] = {:?}", args.layer, args.row, args.col, action);
    Ok(())
}

async fn set_key(args: SetKeyArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    if args.keycode > u8::MAX as u16 {
        bail!("keycode {} exceeds u8 range (max 255); high byte would be silently truncated", args.keycode);
    }
    let hid_code = HidKeyCode::from(args.keycode as u8);
    let action = KeyAction::Single(Action::Key(KeyCode::Hid(hid_code)));
    let req = SetKeyRequest {
        position: KeyPosition {
            layer: args.layer,
            row: args.row,
            col: args.col,
        },
        action,
    };
    let result = client
        .send_resp::<SetKeyAction>(&req)
        .await
        .context("SetKeyAction request failed")?;
    match result {
        Ok(()) => println!(
            "key[{},{},{}] set to {:?}",
            args.layer, args.row, args.col, action
        ),
        Err(e) => println!("SetKeyAction error: {:?}", e),
    }
    Ok(())
}

async fn dump_keymap(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;

    let caps = client
        .send_resp::<GetCapabilities>(&())
        .await
        .context("GetCapabilities request failed")?;

    let num_layers = client
        .send_resp::<GetLayerCount>(&())
        .await
        .context("GetLayerCount request failed")?;

    println!(
        "Keymap: {} layers x {} rows x {} cols",
        num_layers, caps.num_rows, caps.num_cols
    );

    for layer in 0..num_layers {
        println!("\n=== Layer {} ===", layer);
        let mut row: u8 = 0;
        let mut col: u8 = 0;
        let total = caps.num_rows as u16 * caps.num_cols as u16;
        let mut fetched: u16 = 0;

        while fetched < total {
            let count = (total - fetched).min(32);
            let req = BulkRequest {
                layer,
                start_row: row,
                start_col: col,
                count,
            };
            let actions = client
                .send_resp::<GetKeymapBulk>(&req)
                .await
                .context("GetKeymapBulk request failed")?;

            if actions.is_empty() {
                break;
            }

            for action in actions.iter() {
                if col == 0 {
                    print!("  row {:2}: ", row);
                }
                print!("{:?} ", action);
                col += 1;
                if col >= caps.num_cols {
                    println!();
                    col = 0;
                    row += 1;
                }
                fetched += 1;
            }
        }
    }

    Ok(())
}

async fn get_lock_status(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    let status = client
        .send_resp::<GetLockStatus>(&())
        .await
        .context("GetLockStatus request failed")?;
    println!(
        "locked={} awaiting_keys={} remaining_keys={}",
        status.locked, status.awaiting_keys, status.remaining_keys
    );
    Ok(())
}

async fn get_default_layer(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    let layer = client
        .send_resp::<GetDefaultLayer>(&())
        .await
        .context("GetDefaultLayer request failed")?;
    println!("default_layer={}", layer);
    Ok(())
}

async fn set_default_layer(args: SetDefaultLayerArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    let result = client
        .send_resp::<SetDefaultLayer>(&args.layer)
        .await
        .context("SetDefaultLayer request failed")?;
    match result {
        Ok(()) => println!("default_layer set to {}", args.layer),
        Err(e) => println!("SetDefaultLayer error: {:?}", e),
    }
    Ok(())
}

async fn reboot(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    // Device will disconnect after replying, so ignore connection errors
    match client.send_resp::<Reboot>(&()).await {
        Ok(()) => println!("Reboot command sent, device is rebooting..."),
        Err(HostErr::Wire(WireError::UnknownKey)) => println!("Reboot not supported"),
        Err(HostErr::Closed) => println!("Reboot command sent (device disconnected as expected)"),
        Err(e) => eprintln!("Reboot failed: {e:?}"),
    }
    Ok(())
}

async fn reset_keymap(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    match client.send_resp::<ResetKeymap>(&()).await {
        Ok(Ok(())) => println!("ResetKeymap command sent, device is rebooting..."),
        Ok(Err(e)) => println!("ResetKeymap error: {:?}", e),
        Err(HostErr::Closed) => println!("ResetKeymap command sent (device disconnected as expected)"),
        Err(e) => eprintln!("ResetKeymap failed: {e:?}"),
    }
    Ok(())
}

async fn storage_reset(args: StorageResetArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    let mode = match args.mode.as_str() {
        "full" => StorageResetMode::Full,
        "layout" => StorageResetMode::LayoutOnly,
        other => bail!("Unknown reset mode: {other}. Use 'full' or 'layout'."),
    };
    match client.send_resp::<StorageReset>(&mode).await {
        Ok(()) => println!("StorageReset({:?}) sent, device is rebooting...", mode),
        Err(HostErr::Closed) => println!("StorageReset command sent (device disconnected as expected)"),
        Err(e) => eprintln!("StorageReset failed: {e:?}"),
    }
    Ok(())
}

fn matching_devices(args: &DeviceArgs) -> Result<Vec<DeviceInfo>> {
    let devices = nusb::list_devices().context("failed to enumerate USB devices")?;
    Ok(devices.filter(|device| matches_device(device, args)).collect())
}

fn select_device(args: &DeviceArgs) -> Result<SelectedDevice> {
    let mut devices = matching_devices(args)?;
    if devices.is_empty() {
        bail!("no matching USB devices found")
    }

    if devices.len() > 1 {
        eprintln!("matched {} devices; selecting the first one", devices.len());
    }

    let device = devices.remove(0);
    let interface = select_interface(&device, args)?;
    Ok(SelectedDevice {
        device,
        interface_number: interface.interface_number,
        interface_info: interface.interface_info,
    })
}

fn matches_device(device: &DeviceInfo, args: &DeviceArgs) -> bool {
    args.vid.is_none_or(|vid| device.vendor_id() == vid)
        && args.pid.is_none_or(|pid| device.product_id() == pid)
        && args
            .serial
            .as_deref()
            .is_none_or(|serial| device.serial_number() == Some(serial))
}

struct SelectedInterface {
    interface_number: u8,
    interface_info: Option<InterfaceInfo>,
}

fn select_interface(device: &DeviceInfo, args: &DeviceArgs) -> Result<SelectedInterface> {
    let interfaces: Vec<_> = device.interfaces().cloned().collect();

    if let Some(interface_number) = args.interface_number {
        let interface_info = interfaces
            .iter()
            .find(|interface| interface.interface_number() == interface_number)
            .cloned();
        return Ok(SelectedInterface {
            interface_number,
            interface_info,
        });
    }

    if let Some(interface_info) = interfaces
        .into_iter()
        .find(|interface| interface.class() == VENDOR_INTERFACE_CLASS)
    {
        return Ok(SelectedInterface {
            interface_number: interface_info.interface_number(),
            interface_info: Some(interface_info),
        });
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, interface class metadata may not be available.
        // The bulk interface is typically at index 2 (after two HID
        // interfaces), but this depends on firmware configuration.
        // If this default is wrong, pass --interface-number explicitly.
        eprintln!("no vendor-class interface found; defaulting to interface 2 (bulk is added after HID)");
        eprintln!("if this is wrong, pass --interface-number explicitly");
        return Ok(SelectedInterface {
            interface_number: 2,
            interface_info: None,
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        bail!("no vendor-class USB interface found; try `list` or pass --interface-number`")
    }
}

fn print_device_summary(index: usize, device: &DeviceInfo) {
    println!(
        "[{index}] {:04X}:{:04X} bus={} addr={} product={:?} manufacturer={:?} serial={:?}",
        device.vendor_id(),
        device.product_id(),
        device.bus_number(),
        device.device_address(),
        device.product_string(),
        device.manufacturer_string(),
        device.serial_number(),
    );
}

fn print_selected_device(selected: &SelectedDevice) {
    print_device_summary(0, &selected.device);
    match &selected.interface_info {
        Some(interface) => println!(
            "selected interface {} class=0x{:02X} subclass=0x{:02X} protocol=0x{:02X}",
            interface.interface_number(),
            interface.class(),
            interface.subclass(),
            interface.protocol(),
        ),
        None => println!("selected interface {}", selected.interface_number),
    }
}

fn parse_u16(value: &str) -> Result<u16, String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u16>().map_err(|err| err.to_string())
    }
}
