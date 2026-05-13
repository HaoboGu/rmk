use rynk_host::api::status;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let wpm = status::get_wpm(client.transport()).await?;
    println!("{wpm}");
    Ok(())
}
