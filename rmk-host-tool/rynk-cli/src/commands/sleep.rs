use rynk_host::api::status;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let sleeping = status::get_sleep_state(client.transport())
        .await?
        .map_err(|e| anyhow::anyhow!("firmware rejected get_sleep_state: {e:?}"))?;
    println!("{sleeping}");
    Ok(())
}
