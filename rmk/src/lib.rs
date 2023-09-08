#![no_std]
#![feature(type_alias_impl_trait)]
#![allow(dead_code)]
// Make rust analyzer happy with num-enum crate
#![allow(non_snake_case,non_upper_case_globals)]

pub mod action;
pub mod config;
pub mod debounce;
pub mod keyboard;
pub mod keycode;
pub mod layout;
pub mod layout_macro;
pub mod matrix;
pub mod usb;
#[macro_use]
pub mod rtt_logger;
