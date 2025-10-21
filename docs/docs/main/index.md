---
pageType: home

hero:
  # name: RMK
  text: A feature-rich Rust keyboard firmware
  tagline: Join our Discord server for discussions, support, and community collaboration!
  image:
    src: ./rmk_logo.svg
    alt: RMK Logo
  actions:
    - theme: brand
      text: Get Started
      link: /guide/introduction
    - theme: alt
      text: Discord
      link: https://discord.gg/HHGA7pQxkG

features:
  - title: Extensive Microcontroller Support
    details: Powered by embassy, with robust support for STM32, nRF, RP2040, and ESP32
    icon: 🖥️
    link: https://github.com/embassy-rs/embassy

  - title: Real-time Keymap Configuration
    details: Native Vial support, enabling real-time keymap modification over BLE connections wirelessly
    icon: 🧪
    link: https://get.vial.today

  - title: Advanced Features
    details: Layer switching, media controls and tap-hold keys are available out-of-the-box
    icon: 🕹️

  - title: Wireless Connectivity
    details: BLE wireless support with automatic reconnection and multi-device (tested on nRF52840, ESP32-C3, and ESP32-S3)
    icon: 📡

  - title: Easy Configuration
    details: Define your keyboard through a single keyboard.toml file
    icon: ⚙️

  - title: Performance & Efficiency
    details: Ultra-low 2ms wired/10ms wireless latency with months of battery life
    icon: 🔋
---
