use rynk::io::{Read, Write};
use rynk::{Client, RequestError, TransportError};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    // Fire-and-forget: the firmware may reset before the frame is fully
    // acked, so a clean Ok or a transport disconnect both mean success.
    match client.reboot().await {
        Ok(()) | Err(RequestError::Transport(TransportError::Disconnected)) => {
            println!("reboot requested");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
