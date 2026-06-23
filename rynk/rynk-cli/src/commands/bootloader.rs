use rynk::io::{Read, Write};
use rynk::{Client, RequestError, TransportError};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    // Fire-and-forget, same contract as `reboot`: the device jumps to the
    // bootloader before acking, so a disconnect is also success.
    match client.bootloader_jump().await {
        Ok(()) | Err(RequestError::Transport(TransportError::Disconnected)) => {
            println!("bootloader jump requested");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
