#![deny(unsafe_op_in_unsafe_fn)]

pub mod capabilities;
pub mod hotkey;
pub mod machine;

#[cfg(target_os = "linux")]
pub mod portal;

pub use capabilities::{
    Capabilities, CapabilityNote, DisplayServer, Environment, HotkeyBackend, NoteSeverity,
};
pub use hotkey::{Hotkey, HotkeyError, Modifier};
pub use machine::{Acceleration, Gpu, GpuKind, Machine};
