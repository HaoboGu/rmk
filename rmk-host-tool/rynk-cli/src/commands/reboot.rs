use rynk_host::api::system;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    // The firmware reboots before the response can be read in some cases —
    // either an Ok or a transport disconnect is success.
    match system::reboot(client.transport()).await {
        Ok(()) | Err(rynk_host::TransportError::Disconnected) | Err(rynk_host::TransportError::Timeout) => {
            println!("reboot requested");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
