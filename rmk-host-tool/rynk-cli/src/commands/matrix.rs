use rynk_host::api::status;
use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let m = status::get_matrix_state(client.transport()).await?;
    // Each bit = one key, row-major. Print as hex for quick inspection.
    for b in &m.pressed_bitmap {
        print!("{b:02x} ");
    }
    println!();
    Ok(())
}
