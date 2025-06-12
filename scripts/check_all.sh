# Compile examples
cd examples/use_rust/nrf52832_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/nrf52840 && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/nrf52840_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/nrf52840_ble_split && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/pi_pico_w_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/pi_pico_w_ble_split && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/rp2040 && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/rp2040_direct_pin && cargo update && cargo build --release && cd ../../.. 
cd examples/use_rust/rp2040_split && cargo update && cargo build --release && cd ../../.. 
cd examples/use_rust/rp2040_split_pio && cargo update && cargo build --release && cd ../../.. 
cd examples/use_rust/rp2350 && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/stm32f1 && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/stm32f4 && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/stm32h7 && cargo update && cargo build --release && cd ../../.. 
cd examples/use_rust/py32f07x && cargo update && cargo build --release && cd ../../.. 

cd examples/use_config/nrf52832_ble && cargo update && cargo build --release && cd ../../.. 
cd examples/use_config/nrf52840_ble && cargo update && cargo build --release && cd ../../.. 
cd examples/use_config/nrf52840_ble_split && cargo update && cargo build --release --bin central && cargo update && cargo build --release --bin peripheral && cd ../../.. 
cd examples/use_config/nrf52840_ble_split_direct_pin && cargo update && cargo build --release --bin central && cargo update && cargo build --release --bin peripheral && cd ../../.. 
cd examples/use_config/pi_pico_w_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_config/pi_pico_w_ble_split && cargo update && cargo build --release && cd ../../..
cd examples/use_config/rp2040 && cargo update && cargo build --release && cd ../../.. 
cd examples/use_config/rp2040_direct_pin && cargo update && cargo build --release && cd ../../.. 
cd examples/use_config/rp2040_split && cargo update && cargo build --release --bin central && cargo update && cargo build --release --bin peripheral && cd ../../.. 
cd examples/use_config/rp2040_split_pio && cargo update && cargo build --release --bin central && cargo update && cargo build --release --bin peripheral && cd ../../.. 
cd examples/use_config/stm32f1 && cargo update && cargo build --release && cd ../../..
cd examples/use_config/stm32f4 && cargo update && cargo build --release && cd ../../..
cd examples/use_config/stm32h7 && cargo update && cargo build --release && cd ../../.. 

cd examples/use_rust/esp32c3_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_rust/esp32c6_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_config/esp32c3_ble && cargo update && cargo build --release && cd ../../..
cd examples/use_config/esp32c6_ble && cargo update && cargo build --release && cd ../../..
. ~/export-esp.sh
cd examples/use_rust/esp32s3_ble && cargo +esp build --release && cd ../../..
cd examples/use_config/esp32s3_ble && cargo +esp build --release && cd ../../..

# Clean examples
cd examples/use_rust/nrf52832_ble && cargo clean && cd ../../..
cd examples/use_rust/nrf52840 && cargo clean && cd ../../..
cd examples/use_rust/nrf52840_ble && cargo clean && cd ../../..
cd examples/use_rust/nrf52840_ble_split && cargo clean && cd ../../..
cd examples/use_rust/rp2040 && cargo clean && cd ../../..
cd examples/use_rust/rp2040_direct_pin && cargo clean && cd ../../.. 
cd examples/use_rust/rp2040_split && cargo clean && cd ../../.. 
cd examples/use_rust/rp2040_split_pio && cargo clean && cd ../../.. 
cd examples/use_rust/rp2350 && cargo clean && cd ../../..
cd examples/use_rust/stm32f1 && cargo clean && cd ../../..
cd examples/use_rust/stm32f4 && cargo clean && cd ../../..
cd examples/use_rust/stm32h7 && cargo clean && cd ../../.. 
cd examples/use_rust/py32f07x && cargo clean && cd ../../.. 

cd examples/use_config/nrf52832_ble && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_ble && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_ble_split && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_ble_split_direct_pin && cargo clean && cd ../../.. 
cd examples/use_config/rp2040 && cargo clean && cd ../../.. 
cd examples/use_config/rp2040_direct_pin && cargo clean && cd ../../.. 
cd examples/use_config/rp2040_split && cargo clean && cd ../../.. 
cd examples/use_config/rp2040_split_pio && cargo clean && cd ../../.. 
cd examples/use_config/stm32f1 && cargo clean && cd ../../..
cd examples/use_config/stm32f4 && cargo clean && cd ../../..
cd examples/use_config/stm32h7 && cargo clean && cd ../../..
