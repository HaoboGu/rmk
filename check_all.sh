cd boards/stm32h7 && cargo build --release && cargo clean && cd ../.. 
cd boards/stm32f4 && cargo build --release && cargo clean && cd ../..
cd boards/stm32f1 && cargo build --release && cargo clean && cd ../..
cd boards/rp2040 && cargo build --release && cargo clean && cd ../..
cd boards/nrf52840 && cargo build --release && cargo clean && cd ../..
cd boards/nrf52840_ble && cargo build --release && cargo clean && cd ../..
cd boards/nrf52832_ble && cargo build --release && cargo clean && cd ../..
cd boards/esp32c3_ble && cargo build --release && cd ../..
cd boards/esp32s3_ble && cargo build --release && cd ../..