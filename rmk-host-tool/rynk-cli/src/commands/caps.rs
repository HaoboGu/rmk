use rynk_host::{Client, Transport};

pub async fn run<T: Transport>(client: &mut Client<T>, json: bool) -> anyhow::Result<()> {
    let caps = client.capabilities();
    if json {
        // Manual JSON formatting keeps this tool's deps minimal — the
        // capability struct is fixed-shape and rarely changes.
        println!("{{");
        println!("  \"num_layers\": {},", caps.num_layers);
        println!("  \"num_rows\": {},", caps.num_rows);
        println!("  \"num_cols\": {},", caps.num_cols);
        println!("  \"num_encoders\": {},", caps.num_encoders);
        println!("  \"max_combos\": {},", caps.max_combos);
        println!("  \"max_combo_keys\": {},", caps.max_combo_keys);
        println!("  \"max_macros\": {},", caps.max_macros);
        println!("  \"macro_space_size\": {},", caps.macro_space_size);
        println!("  \"max_morse\": {},", caps.max_morse);
        println!("  \"max_patterns_per_key\": {},", caps.max_patterns_per_key);
        println!("  \"max_forks\": {},", caps.max_forks);
        println!("  \"storage_enabled\": {},", caps.storage_enabled);
        println!("  \"lighting_enabled\": {},", caps.lighting_enabled);
        println!("  \"is_split\": {},", caps.is_split);
        println!("  \"num_split_peripherals\": {},", caps.num_split_peripherals);
        println!("  \"ble_enabled\": {},", caps.ble_enabled);
        println!("  \"num_ble_profiles\": {},", caps.num_ble_profiles);
        println!("  \"max_payload_size\": {},", caps.max_payload_size);
        println!("  \"max_bulk_keys\": {},", caps.max_bulk_keys);
        println!("  \"macro_chunk_size\": {},", caps.macro_chunk_size);
        println!("  \"bulk_transfer_supported\": {}", caps.bulk_transfer_supported);
        println!("}}");
    } else {
        println!("{:#?}", caps);
    }
    Ok(())
}
