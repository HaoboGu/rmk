# Compile examples
cd examples/use_rust/esp32c3_ble && cargo build --release && cd ../../..
cd examples/use_rust/esp32c6_ble && cargo build --release && cd ../../..
cd examples/use_rust/esp32s3_ble && cargo build --release && cd ../../..
cd examples/use_config/esp32c3_ble && cargo build --release && cd ../../..
cd examples/use_config/esp32c6_ble && cargo build --release && cd ../../..
cd examples/use_config/esp32s3_ble && cargo build --release && cd ../../..

cd examples/use_rust/nrf52832_ble && cargo build --release && cd ../../..
cd examples/use_rust/nrf52840 && cargo build --release && cd ../../..
cd examples/use_rust/nrf52840_ble && cargo build --release && cd ../../..
cd examples/use_rust/rp2040 && cargo build --release && cd ../../..
cd examples/use_rust/rp2040_direct_pin && cargo build --release && cd ../../.. 
cd examples/use_rust/stm32f1 && cargo build --release && cd ../../..
cd examples/use_rust/stm32f4 && cargo build --release && cd ../../..
cd examples/use_rust/stm32h7 && cargo build --release && cd ../../.. 
cd examples/use_rust/stm32h7_async && cargo build --release && cd ../../.. 
cd examples/use_rust/hpm5300 && cargo build --release && cd ../../.. 
cd examples/use_rust/rp2040_split && cargo build --release --bin central && cargo build --release --bin peripheral && cd ../../.. 
cd examples/use_rust/nrf52840_ble_split && cargo build --release --bin central && cargo build --release --bin peripheral && cd ../../.. 

cd examples/use_config/nrf52832_ble && cargo build --release && cd ../../.. 
cd examples/use_config/nrf52840_ble && cargo build --release && cd ../../.. 
cd examples/use_config/nrf52840_usb && cargo build --release && cd ../../.. 
cd examples/use_config/rp2040 && cargo build --release && cd ../../.. 
cd examples/use_config/rp2040_direct_pin && cargo build --release && cd ../../.. 
cd examples/use_config/stm32f1 && cargo build --release && cd ../../..
cd examples/use_config/stm32f4 && cargo build --release && cd ../../..
cd examples/use_config/stm32h7 && cargo build --release && cd ../../.. 
cd examples/use_config/rp2040_split && cargo build --release --bin central && cargo build --release --bin peripheral && cd ../../.. 
cd examples/use_config/nrf52840_ble_split && cargo build --release --bin central && cargo build --release --bin peripheral && cd ../../.. 

# # Clean examples
cd examples/use_rust/nrf52832_ble && cargo clean && cd ../../..
cd examples/use_rust/nrf52840 && cargo clean && cd ../../..
cd examples/use_rust/nrf52840_ble && cargo clean && cd ../../..
cd examples/use_rust/rp2040 && cargo clean && cd ../../..
cd examples/use_rust/rp2040_direct_pin && cargo clean && cd ../../.. 
cd examples/use_rust/stm32f1 && cargo clean && cd ../../..
cd examples/use_rust/stm32f4 && cargo clean && cd ../../..
cd examples/use_rust/stm32h7 && cargo clean && cd ../../..  
cd examples/use_rust/stm32h7_async && cargo clean && cd ../../.. 
cd examples/use_rust/hpm5300 && cargo clean && cd ../../.. 
cd examples/use_rust/rp2040_split && cargo clean && cd ../../.. 
cd examples/use_rust/nrf52840_ble_split && cargo clean && cd ../../.. 


cd examples/use_config/nrf52832_ble && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_ble && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_usb && cargo clean && cd ../../.. 
cd examples/use_config/rp2040 && cargo clean && cd ../../.. 
cd examples/use_config/rp2040_direct_pin && cargo clean && cd ../../.. 
cd examples/use_config/stm32f1 && cargo clean && cd ../../..
cd examples/use_config/stm32f4 && cargo clean && cd ../../..
cd examples/use_config/stm32h7 && cargo clean && cd ../../..
cd examples/use_config/rp2040_split && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_ble_split && cargo clean && cd ../../.. 