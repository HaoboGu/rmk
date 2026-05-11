use rynk_host::api::keymap;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>, layer: u8, row: u8, col: u8) -> anyhow::Result<()> {
    let action = keymap::get_key(client.transport(), layer, row, col).await?;
    println!("{action:?}");
    Ok(())
}
