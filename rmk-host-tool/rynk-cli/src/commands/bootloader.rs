use rynk_host::api::system;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    match system::bootloader_jump(client.transport()).await {
        Ok(Ok(())) | Err(rynk_host::TransportError::Disconnected) | Err(rynk_host::TransportError::Timeout) => {
            println!("bootloader jump requested");
            Ok(())
        }
        Ok(Err(e)) => Err(anyhow::anyhow!("firmware rejected bootloader jump: {e:?}")),
        Err(e) => Err(e.into()),
    }
}
