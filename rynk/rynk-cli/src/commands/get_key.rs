use rynk::Client;
use rynk::io::{Read, Write};

pub async fn run<T: Read + Write>(client: &mut Client<T>, layer: u8, row: u8, col: u8) -> anyhow::Result<()> {
    let action = client.get_key(layer, row, col).await?;
    println!("{action:?}");
    Ok(())
}
