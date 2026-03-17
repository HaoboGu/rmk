//! Resolved configuration types — the public API of `rmk-config`.
//!
//! These types represent the final, validated, defaults-applied output of the
//! 3-layer TOML merge (event defaults → chip defaults → user config).
//!
//! There are five resolved types, each consumed at a different stage:
//!
//! - [`BuildConstants`] — compile-time constants emitted by `rmk-types/build.rs`
//! - [`Identity`] — keyboard identity for USB descriptors and BLE advertising
//! - [`Hardware`] — complete hardware config for proc-macro code generation
//! - [`Behavior`] — behavioral config (combos, macros, morse, forks, etc.)
//! - [`Layout`] — keymap and encoder layout for keymap generation
//!
//! Consumers call resolution methods on [`KeyboardTomlConfig`](crate::KeyboardTomlConfig):
//! - `.build_constants()` → `BuildConstants`
//! - `.identity()` → `Identity`
//! - `.hardware()` → `Result<Hardware, String>`
//! - `.behavior()` → `Result<Behavior, String>`
//! - `.layout()` → `Result<Layout, String>`

mod behavior;
mod constants;
mod hardware;
mod identity;
mod layout;

pub use behavior::*;
pub use constants::*;
pub use hardware::*;
pub use identity::*;
pub use layout::*;
