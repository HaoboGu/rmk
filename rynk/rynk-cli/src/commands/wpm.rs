use rynk::Client;
use rynk::io::{Read, Write};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    let wpm = client.get_wpm().await?;
    println!("{wpm}");
    Ok(())
}
