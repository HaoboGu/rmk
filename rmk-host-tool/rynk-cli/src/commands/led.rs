use rynk_host::api::status;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let led = status::get_led_indicator(client.transport()).await?;
    println!(
        "num={} caps={} scroll={} compose={} kana={}",
        led.num_lock(),
        led.caps_lock(),
        led.scroll_lock(),
        led.compose(),
        led.kana(),
    );
    Ok(())
}
