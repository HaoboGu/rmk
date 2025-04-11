$initialDir = Get-Location

# Compile examples
$buildDirs = @(
    "examples/use_rust/esp32c3_ble",
    "examples/use_rust/esp32c6_ble",
    "examples/use_rust/esp32s3_ble",
    "examples/use_config/esp32c3_ble",
    "examples/use_config/esp32c6_ble",
    "examples/use_config/esp32s3_ble",
    "examples/use_rust/nrf52832_ble",
    "examples/use_rust/nrf52840",
    "examples/use_rust/nrf52840_ble",
    "examples/use_rust/rp2040",
    "examples/use_rust/rp2040_direct_pin",
    "examples/use_rust/stm32f1",
    "examples/use_rust/stm32f4",
    "examples/use_rust/stm32h7",
    "examples/use_rust/stm32h7_async",
    "examples/use_rust/hpm5300",
    "examples/use_rust/rp2040_split",
    "examples/use_rust/nrf52840_ble_split",
    "examples/use_rust/py32f07x",
    "examples/use_config/nrf52832_ble",
    "examples/use_config/nrf52840_ble",
    "examples/use_config/rp2040",
    "examples/use_rust/rp2040_direct_pin",
    "examples/use_config/stm32f1",
    "examples/use_config/stm32f4",
    "examples/use_config/stm32h7",
    "examples/use_config/rp2040_split",
    "examples/use_config/nrf52840_ble_split"
)

foreach ($dir in $buildDirs) {
    Set-Location $dir
    if ($dir -like "*split*") {
        cargo build --release --bin central
        cargo build --release --bin peripheral
    } else {
        cargo build --release
    }
    Set-Location $initialDir
}

foreach ($dir in $buildDirs) {
    Set-Location $dir
    cargo clean
    Set-Location $initialDir
}