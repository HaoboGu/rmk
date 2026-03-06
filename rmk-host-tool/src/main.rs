use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use nusb::{DeviceInfo, InterfaceInfo};
use postcard_rpc::header::VarSeqKind;
use postcard_rpc::host_client::{HostClient, HostErr};
use postcard_rpc::standard_icd::{ERROR_PATH, WireError};
use rmk_types::protocol::rmk::{GetCapabilities, GetVersion};

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
struct HandshakeArgs {
    #[command(flatten)]
    device: DeviceArgs,
    /// Outgoing queue depth for postcard-rpc host client.
    #[arg(long, default_value_t = DEFAULT_OUTGOING_DEPTH)]
    outgoing_depth: usize,
    /// Sequence-number width used by the host client.
    #[arg(long, value_enum, default_value_t = SeqKindArg::Seq2)]
    seq_kind: SeqKindArg,
    /// Fail if `GetCapabilities` is not implemented yet.
    #[arg(long)]
    require_capabilities: bool,
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

async fn handshake(args: HandshakeArgs) -> Result<()> {
    let selected = select_device(&args.device)?;
    print_selected_device(&selected);

    let client = HostClient::<WireError>::try_from_nusb_and_interface(
        &selected.device,
        selected.interface_number as usize,
        ERROR_PATH,
        args.outgoing_depth,
        args.seq_kind.into(),
    )
    .map_err(|err| anyhow!(err))?;

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
        eprintln!("no interface metadata exposed by Windows; falling back to interface 0");
        return Ok(SelectedInterface {
            interface_number: 0,
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
