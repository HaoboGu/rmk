use rynk_host::api::status;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let layer = status::get_current_layer(client.transport())
        .await?
        .map_err(|e| anyhow::anyhow!("firmware rejected get_current_layer: {e:?}"))?;
    println!("{layer}");
    Ok(())
}
