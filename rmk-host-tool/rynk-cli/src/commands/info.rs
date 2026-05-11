use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>) -> anyhow::Result<()> {
    let caps = client.capabilities();
    println!(
        "Layout:           {} layers × {} rows × {} cols\n\
         Encoders:         {}\n\
         Combos:           up to {} (≤{} keys each)\n\
         Macros:           up to {} ({} bytes)\n\
         Morse:            up to {} (≤{} patterns each)\n\
         Forks:            up to {}\n\
         Storage:          {}\n\
         BLE:              {} ({} profiles)\n\
         Split:            {} (peripherals: {})\n\
         Bulk transfer:    {}",
        caps.num_layers,
        caps.num_rows,
        caps.num_cols,
        caps.num_encoders,
        caps.max_combos,
        caps.max_combo_keys,
        caps.max_macros,
        caps.macro_space_size,
        caps.max_morse,
        caps.max_patterns_per_key,
        caps.max_forks,
        if caps.storage_enabled { "enabled" } else { "disabled" },
        if caps.ble_enabled { "enabled" } else { "disabled" },
        caps.num_ble_profiles,
        if caps.is_split { "yes" } else { "no" },
        caps.num_split_peripherals,
        if caps.bulk_transfer_supported {
            "enabled"
        } else {
            "disabled"
        },
    );
    Ok(())
}
