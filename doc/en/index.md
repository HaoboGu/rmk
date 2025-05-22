---
# https://vitepress.dev/reference/default-theme-home-page
layout: home

hero:
  name: "RMK"
  text: "A feature-rich Rust keyboard firmware."
  tagline: "Join our Discord server to discuss keyboard firmware development!"
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
  - icon: 🦀
    title: Memory Safety & Performance
    details: Rust's ownership ensures memory safety without GC, matching C's efficiency.

  - icon: 🛠️
    title: Cross-Platform Hardware
    details: Supports Nordic, RP2040, ESP32 with single-command compilation.

  - icon: ⌨️
    title: Live Keymap Editing
    details: VIA/Vial GUI support for real-time keymap/RGB adjustments.

  - icon: 📶
    title: Multi-Protocol Wireless
    details: BLE 5.2 + 2.4G hybrid mode with smart power management.

  - icon: 🔄
    title: Hot-Swap Profiles
    details: EEPROM stores 5+ keymap profiles for instant switching.

  - icon: 🧩
    title: Modular Architecture
    details: Decoupled matrix scan, HID protocol, and keymap layers.

  - icon: ⚡
    title: CLI Toolchain
    details: Built-in firmware build/flash/debug tools via cargo-rmk.

  - icon: 🌍
    title: Open Ecosystem
    details: Full OSS stack from PCB design to production.
---
