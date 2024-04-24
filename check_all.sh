cd examples/use_rust/esp32c3_ble && cargo build --release && cd ../../..
cd examples/use_rust/esp32s3_ble && cargo build --release && cd ../../..
cd examples/use_rust/nrf52832_ble && cargo build --release && cargo clean && cd ../../..
cd examples/use_rust/nrf52840 && cargo build --release && cargo clean && cd ../../..
cd examples/use_rust/nrf52840_ble && cargo build --release && cargo clean && cd ../../..
cd examples/use_rust/rp2040 && cargo build --release && cargo clean && cd ../../..
cd examples/use_rust/stm32f1 && cargo build --release && cargo clean && cd ../../..
cd examples/use_rust/stm32f4 && cargo build --release && cargo clean && cd ../../..
cd examples/use_rust/stm32h7 && cargo build --release && cargo clean && cd ../../.. 

cd examples/use_config/esp32c3_ble && cargo build --release && cd ../../..
cd examples/use_config/esp32s3_ble && cargo build --release && cd ../../..
cd examples/use_config/nrf52832_ble && cargo build --release && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_ble && cargo build --release && cargo clean && cd ../../.. 
cd examples/use_config/nrf52840_usb && cargo build --release && cargo clean && cd ../../.. 
cd examples/use_config/rp2040 && cargo build --release && cargo clean && cd ../../.. 
cd examples/use_config/stm32f1 && cargo build --release && cargo clean && cd ../../..
cd examples/use_config/stm32f4 && cargo build --release && cargo clean && cd ../../..
cd examples/use_config/stm32h7 && cargo build --release && cargo clean && cd ../../.. 