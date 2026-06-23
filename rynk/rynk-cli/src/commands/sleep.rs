use rynk::Client;
use rynk::io::{Read, Write};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    let sleeping = client.get_sleep_state().await?;
    println!("{sleeping}");
    Ok(())
}
