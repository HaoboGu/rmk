use rynk::Client;
use rynk::io::{Read, Write};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    let led = client.get_led_indicator().await?;
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
