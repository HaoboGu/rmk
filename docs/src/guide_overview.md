# Overview

This guide introduces how to build your own keyboard firmware using RMK. RMK is a Rust crate which helps you create your own keyboard firmware easily, with lots of features such as layers, dynamic keymap, [vial](https://get.vial.today/) support, etc.

Using RMK requires basic knowledge of Rust programming and embedded devices. If you're not familiar with Rust, [The Official Rust Book](https://doc.rust-lang.org/book/) is a good start. And if you're not familiar with **embedded Rust**, we recommend you to read [The Embedded Rust Book](https://docs.rust-embedded.org/book/) first. 

There are 3 main steps of the guide:

- setup the RMK environment

- create a RMK project

- compile the firmware and flash

After completing the all 3 steps of the guide, you'll learn how to create your personal keyboard firmware using pure Rust and make it run on your real-world hardware!