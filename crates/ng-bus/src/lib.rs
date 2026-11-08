#![forbid(unsafe_code)]
//! ng-bus: 68000 system bus with Neo Geo memory mapping.

pub mod device;
pub mod map68k;

pub use device::{CartPort, Device, NullDevice};
pub use map68k::Bus68k;
