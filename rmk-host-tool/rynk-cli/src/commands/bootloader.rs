use rynk_host::api::system;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    match system::bootloader_jump(client.transport()).await {
        Ok(()) | Err(rynk_host::TransportError::Disconnected) | Err(rynk_host::TransportError::Timeout) => {
            println!("bootloader jump requested");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
