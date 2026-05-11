//! `rynk` — command-line client for the Rynk protocol.
//!
//! Each subcommand is a thin wrapper over one or two `rynk-host` calls.
//! Add new subcommands as new modules under `commands/` and a new
//! `Command` variant below.

use clap::{Parser, Subcommand};

mod commands;
mod connect;

#[derive(Parser, Debug)]
#[command(
    name = "rynk",
    about = "Host-side CLI for the Rynk protocol",
    version,
    long_about = None,
)]
struct Cli {
    /// Force a specific transport. `auto` (default) tries USB first, then BLE.
    #[arg(long, value_enum, default_value_t = TransportKind::Auto, global = true)]
    transport: TransportKind,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum TransportKind {
    Auto,
    Usb,
    Ble,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print version + capability summary.
    Info,
    /// Print the full capability struct.
    Caps {
        /// Emit JSON instead of a human-readable summary.
        #[arg(long)]
        json: bool,
    },
    /// Read one key.
    GetKey { layer: u8, row: u8, col: u8 },
    /// Read the current active layer.
    Layer,
    /// Read the live matrix bitmap.
    Matrix,
    /// Reboot the keyboard.
    Reboot,
    /// Jump to bootloader (DFU mode).
    Bootloader,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let transport = connect::connect(cli.transport).await?;
    let mut client = rynk_host::Client::connect(transport).await?;

    match cli.command {
        Command::Info => commands::info::run(&mut client).await,
        Command::Caps { json } => commands::caps::run(&mut client, json).await,
        Command::GetKey { layer, row, col } => commands::get_key::run(&mut client, layer, row, col).await,
        Command::Layer => commands::layer::run(&mut client).await,
        Command::Matrix => commands::matrix::run(&mut client).await,
        Command::Reboot => commands::reboot::run(&mut client).await,
        Command::Bootloader => commands::bootloader::run(&mut client).await,
    }
}
