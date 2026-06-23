use rynk::Client;
use rynk::io::{Read, Write};

pub async fn run<T: Read + Write>(client: &mut Client<T>) -> anyhow::Result<()> {
    let m = client.get_matrix_state().await?;
    // Each bit = one key, row-major. Print as hex for quick inspection.
    for b in &m.pressed_bitmap {
        print!("{b:02x} ");
    }
    println!();
    Ok(())
}
