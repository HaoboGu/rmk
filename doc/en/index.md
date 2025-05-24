---
layout: home

hero:
  name: 'RMK'
  text: 'A feature-rich Rust keyboard firmware.'
  tagline: 'Join our Discord server to discuss keyboard firmware development!'
  image:
    src: /images/rmk_logo.svg
    alt: Rmk Logo
  actions:
    - theme: brand
      text: Get Started
      link: /documentation/user_guide/1_guide_overview
    - theme: alt
      text: Discord Community
      link: https://discord.gg/HHGA7pQxkG

features:
  - icon: 🖥️
    title: Microcontroller Support
    link: https://github.com/embassy-rs/embassy
    linkText: embassy
    details: Powered by embassy, supports stm32/nRF/rp2040/esp32

  - icon: 🎛️
    title: Real-time Keymap Editing
    link: https://get.vial.today/
    linkText: Vial
    details: Built-in Vial support with BLE direct editing

  - icon: 🕹️
    title: Advanced Features
    details: Layer/media/system control, mouse emulation out-of-box

  - icon: 📡
    title: Wireless Connectivity
    details: BLE with auto-reconnection (nRF52840/esp32c3/s3 tested)

  - icon: ⚙️
    title: Easy Configuration
    details: Define keyboard via keyboard.toml + Rust code customization

  - icon: 🔋
    title: Low-Latency & Power
    details: 2ms wired/10ms wireless latency, months-long battery life
---
