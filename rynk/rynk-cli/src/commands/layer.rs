use rynk::Client;
use rynk::io::{Read, Write};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    let layer = client.get_current_layer().await?;
    println!("{layer}");
    Ok(())
}
