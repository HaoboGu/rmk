use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use nusb::{DeviceInfo, InterfaceInfo};
use postcard_rpc::Endpoint;
use postcard_rpc::header::VarSeqKind;
use postcard_rpc::host_client::{HostClient, HostErr, SchemaError, SchemaReport};
use postcard_rpc::postcard_schema::schema::owned::OwnedNamedType;
use postcard_rpc::standard_icd::{ERROR_PATH, PingEndpoint, WireError};
use rmk_types::action::{Action, KeyAction};
use rmk_types::keycode::{HidKeyCode, KeyCode};
use rmk_types::protocol::rmk::{
    BootloaderJump, BulkRequest, ENDPOINT_LIST, GetCapabilities, GetConnectionInfo, GetCurrentLayer, GetDefaultLayer,
    GetKeyAction, GetKeymapBulk, GetLayerCount, GetLockStatus, GetMatrixState, GetVersion, KeyPosition, LockRequest,
    MAX_BULK, ProtocolVersion, Reboot, ResetKeymap, RmkError, SetDefaultLayer, SetKeyAction, SetKeyRequest,
    SetKeymapBulk, StorageReset, StorageResetMode, UnlockRequest,
};

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
    /// Exercise postcard-rpc standard ping.
    Ping(PingArgs),
    /// Dump the schema for the connected RMK protocol version.
    Schema(ConnectArgs),
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
    Reboot(DestructiveArgs),
    /// Reset keymap to defaults (erases storage and reboots).
    ResetKeymap(DestructiveArgs),
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
struct DestructiveArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Skip confirmation prompt.
    #[arg(long, short = 'y')]
    yes: bool,
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
struct PingArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Ping value to echo through postcard-rpc standard ICD.
    #[arg(long, default_value_t = 42)]
    value: u32,
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
    /// Only basic HID keycodes (0-255) are supported; complex KeyAction
    /// variants (TapHold, LayerSwitch, etc.) are not yet available from the CLI.
    #[arg(long)]
    keycode: u8,
}

#[derive(Args, Debug, Clone)]
struct SetDefaultLayerArgs {
    #[command(flatten)]
    connect: ConnectArgs,
    /// Layer number to set as default.
    #[arg(long)]
    layer: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum StorageResetModeArg {
    /// Erase everything.
    Full,
    /// Erase only layout data.
    Layout,
}

impl From<StorageResetModeArg> for StorageResetMode {
    fn from(value: StorageResetModeArg) -> Self {
        match value {
            StorageResetModeArg::Full => StorageResetMode::Full,
            StorageResetModeArg::Layout => StorageResetMode::LayoutOnly,
        }
    }
}

#[derive(Args, Debug, Clone)]
struct StorageResetArgs {
    #[command(flatten)]
    destructive: DestructiveArgs,
    /// Reset mode.
    #[arg(long, default_value = "full")]
    mode: StorageResetModeArg,
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
        Command::Ping(args) => ping(args).await,
        Command::Schema(args) => schema(args).await,
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

/// Check protocol version compatibility and print a warning if mismatched.
/// Returns the firmware version on success.
async fn check_version(client: &HostClient<WireError>) -> Result<ProtocolVersion> {
    let version = client
        .send_resp::<GetVersion>(&())
        .await
        .context("GetVersion request failed")?;
    let expected = ProtocolVersion::CURRENT;
    if !expected.is_backward_compatible_with(&version) {
        eprintln!(
            "WARNING: firmware protocol version {}.{} is not compatible with host version {}.{}",
            version.major, version.minor, expected.major, expected.minor
        );
    }
    Ok(version)
}

/// Send an UnlockRequest so write operations are accepted by the firmware.
/// Currently the firmware unlocks immediately without a physical challenge.
async fn ensure_unlocked(client: &HostClient<WireError>) -> Result<()> {
    client
        .send_resp::<UnlockRequest>(&())
        .await
        .context("UnlockRequest failed")?;
    Ok(())
}

async fn ping(args: PingArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    let echoed = client
        .send_resp::<PingEndpoint>(&args.value)
        .await
        .context("PingEndpoint request failed")?;
    println!("ping.request={}", args.value);
    println!("ping.response={}", echoed);
    if echoed != args.value {
        bail!("unexpected ping response: expected {}, got {}", args.value, echoed);
    }
    Ok(())
}

async fn schema(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    let version = check_version(&client).await?;
    let (mut report, schema_source) = match client.get_schema_report().await {
        Ok(report) => (report, "device"),
        Err(SchemaError::Comms(HostErr::Wire(WireError::UnknownKey))) => (local_schema_report(), "shared-icd"),
        Err(SchemaError::InvalidReportData | SchemaError::LostData) => (local_schema_report(), "shared-icd"),
        Err(err) => return Err(err).context("schema discovery failed"),
    };

    report.endpoints.sort_by(|a, b| a.path.cmp(&b.path));
    report.topics_in.sort_by(|a, b| a.path.cmp(&b.path));
    report.topics_out.sort_by(|a, b| a.path.cmp(&b.path));

    println!("protocol.version={}.{}", version.major, version.minor);
    println!("schema.source={schema_source}");
    println!("schema.types={}", report.types.len());
    println!("schema.endpoints={}", report.endpoints.len());
    println!("schema.topics.in={}", report.topics_in.len());
    println!("schema.topics.out={}", report.topics_out.len());

    if !report.endpoints.is_empty() {
        println!("\n# Endpoints");
        for endpoint in report.endpoints {
            println!(
                "{} req={} {} resp={} {}",
                endpoint.path,
                format_key(endpoint.req_key),
                endpoint.req_ty,
                format_key(endpoint.resp_key),
                endpoint.resp_ty,
            );
        }
    }

    if !report.topics_in.is_empty() {
        println!("\n# Topics ToServer");
        for topic in report.topics_in {
            println!("{} key={} {}", topic.path, format_key(topic.key), topic.ty);
        }
    }

    if !report.topics_out.is_empty() {
        println!("\n# Topics ToClient");
        for topic in report.topics_out {
            println!("{} key={} {}", topic.path, format_key(topic.key), topic.ty);
        }
    }

    Ok(())
}

fn local_schema_report() -> SchemaReport {
    let mut report = SchemaReport::default();

    for ty in ENDPOINT_LIST.types {
        report.add_type(OwnedNamedType::from(*ty));
    }

    report
        .add_endpoint(
            PingEndpoint::PATH.to_string(),
            PingEndpoint::REQ_KEY,
            PingEndpoint::RESP_KEY,
        )
        .expect("standard ping endpoint should resolve");

    for path in [
        GetVersion::PATH,
        GetCapabilities::PATH,
        GetLockStatus::PATH,
        UnlockRequest::PATH,
        LockRequest::PATH,
        Reboot::PATH,
        BootloaderJump::PATH,
        StorageReset::PATH,
        GetKeyAction::PATH,
        SetKeyAction::PATH,
        GetKeymapBulk::PATH,
        SetKeymapBulk::PATH,
        GetLayerCount::PATH,
        GetDefaultLayer::PATH,
        SetDefaultLayer::PATH,
        ResetKeymap::PATH,
        GetConnectionInfo::PATH,
        GetCurrentLayer::PATH,
        GetMatrixState::PATH,
    ] {
        let endpoint = ENDPOINT_LIST
            .endpoints
            .iter()
            .find(|entry| entry.0 == path)
            .expect("implemented endpoint should exist in shared ICD");
        report
            .add_endpoint(path.to_string(), endpoint.1, endpoint.2)
            .expect("implemented endpoint schema should resolve");
    }

    report
}

async fn handshake(args: HandshakeArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    let version = check_version(&client).await?;
    println!("protocol.version={}.{}", version.major, version.minor);

    match client.send_resp::<GetCapabilities>(&()).await {
        Ok(caps) => {
            println!("capabilities={:#?}", caps);
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
    check_version(&client).await?;
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
    check_version(&client).await?;
    ensure_unlocked(&client).await?;
    let hid_code = HidKeyCode::from(args.keycode);
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
        Ok(()) => println!("key[{},{},{}] set to {:?}", args.layer, args.row, args.col, action),
        Err(e) => bail!("SetKeyAction error: {:?}", e),
    }
    Ok(())
}

async fn dump_keymap(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    check_version(&client).await?;

    let caps = client
        .send_resp::<GetCapabilities>(&())
        .await
        .context("GetCapabilities request failed")?;

    if caps.num_layers == 0 || caps.num_rows == 0 || caps.num_cols == 0 {
        bail!(
            "invalid capabilities: layers={} rows={} cols={} (all must be > 0)",
            caps.num_layers,
            caps.num_rows,
            caps.num_cols
        );
    }

    println!(
        "Keymap: {} layers x {} rows x {} cols",
        caps.num_layers, caps.num_rows, caps.num_cols
    );

    let num_cols = caps.num_cols as u16;
    for layer in 0..caps.num_layers {
        println!("\n=== Layer {} ===", layer);
        let total = caps.num_rows as u16 * num_cols;
        let mut fetched: u16 = 0;

        while fetched < total {
            let row = (fetched / num_cols) as u8;
            let col = (fetched % num_cols) as u8;
            let count = ((total - fetched).min(MAX_BULK as u16)) as u8;
            let actions = client
                .send_resp::<GetKeymapBulk>(&BulkRequest {
                    layer,
                    start_row: row,
                    start_col: col,
                    count,
                })
                .await
                .context("GetKeymapBulk request failed")?;

            if actions.is_empty() {
                break;
            }

            for action in actions.iter() {
                if fetched % num_cols == 0 {
                    print!("  row {:2}: ", fetched / num_cols);
                }
                print!("{:?} ", action);
                fetched += 1;
                if fetched % num_cols == 0 {
                    println!();
                }
            }
        }
    }

    Ok(())
}

async fn get_lock_status(args: ConnectArgs) -> Result<()> {
    let client = connect(&args)?;
    check_version(&client).await?;
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
    check_version(&client).await?;
    let layer = client
        .send_resp::<GetDefaultLayer>(&())
        .await
        .context("GetDefaultLayer request failed")?;
    println!("default_layer={}", layer);
    Ok(())
}

async fn set_default_layer(args: SetDefaultLayerArgs) -> Result<()> {
    let client = connect(&args.connect)?;
    check_version(&client).await?;
    ensure_unlocked(&client).await?;
    let result = client
        .send_resp::<SetDefaultLayer>(&args.layer)
        .await
        .context("SetDefaultLayer request failed")?;
    match result {
        Ok(()) => println!("default_layer set to {}", args.layer),
        Err(e) => bail!("SetDefaultLayer error: {:?}", e),
    }
    Ok(())
}

async fn reboot(args: DestructiveArgs) -> Result<()> {
    if !args.yes {
        confirm_destructive("This will reboot the keyboard device.")?;
    }
    let client = connect(&args.connect)?;
    check_version(&client).await?;
    ensure_unlocked(&client).await?;
    handle_reboot_response(client.send_resp::<Reboot>(&()).await, "Reboot")
}

async fn reset_keymap(args: DestructiveArgs) -> Result<()> {
    if !args.yes {
        confirm_destructive("This will erase all keymap data and reboot. BLE bonds may also be lost.")?;
    }
    let client = connect(&args.connect)?;
    check_version(&client).await?;
    ensure_unlocked(&client).await?;
    handle_reboot_response(client.send_resp::<ResetKeymap>(&()).await, "ResetKeymap")
}

async fn storage_reset(args: StorageResetArgs) -> Result<()> {
    let warning = match args.mode {
        StorageResetModeArg::Full => "This will erase ALL storage and reboot. All configuration data will be lost.",
        StorageResetModeArg::Layout => {
            "WARNING: Layout-only reset is not yet implemented in firmware.\n\
             This currently falls back to a FULL erase, which also clears:\n\
             - BLE bonding information\n\
             - Behavior configuration (combos, morse, forks)\n\
             - Connection preferences\n\
             The device will reboot after erasing."
        }
    };
    if !args.destructive.yes {
        confirm_destructive(warning)?;
    }
    let client = connect(&args.destructive.connect)?;
    check_version(&client).await?;
    ensure_unlocked(&client).await?;
    let mode = StorageResetMode::from(args.mode);
    handle_reboot_response(
        client.send_resp::<StorageReset>(&mode).await,
        &format!("StorageReset({mode:?})"),
    )
}

/// Handle the result of a destructive command that reboots the device.
/// The device disconnects after replying, so `HostErr::Closed` is expected success.
fn handle_reboot_response(result: Result<Result<(), RmkError>, HostErr<WireError>>, cmd: &str) -> Result<()> {
    match result {
        Ok(Ok(())) => println!("{cmd} command sent, device is rebooting..."),
        Ok(Err(e)) => bail!("{cmd} rejected: {e}"),
        Err(HostErr::Closed) => println!("{cmd} command sent (device disconnected as expected)"),
        Err(e) => bail!("{cmd} failed: {e:?}"),
    }
    Ok(())
}

fn confirm_destructive(message: &str) -> Result<()> {
    eprintln!("{}", message);
    eprint!("Continue? [y/N] ");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("failed to read confirmation")?;
    if !input.trim().eq_ignore_ascii_case("y") {
        bail!("aborted by user");
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

    let device = devices.swap_remove(0);
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

fn format_key(key: postcard_rpc::Key) -> String {
    format!("0x{:016X}", u64::from_le_bytes(key.to_bytes()))
}

fn parse_u16(value: &str) -> Result<u16, String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("0x").or_else(|| value.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u16>().map_err(|err| err.to_string())
    }
}
