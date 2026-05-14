use rynk_host::api::status;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let wpm = status::get_wpm(client.transport())
        .await?
        .map_err(|e| anyhow::anyhow!("firmware rejected get_wpm: {e:?}"))?;
    println!("{wpm}");
    Ok(())
}
