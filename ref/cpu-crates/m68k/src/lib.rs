//! puce-m68k: a cycle-accurate Motorola 68000 CPU core.
//!
//! The core owns all of its state, has no clock, no global and no allocation.
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(any(test, feature = "std"))]
extern crate std;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod alu;
pub mod bus;
pub mod cpu;
pub mod decode;
pub mod ea;
pub mod engine;
pub mod insn;
pub mod pins;
pub mod regs;
pub mod state;
// pub mod sem;
#[cfg(feature = "disasm")]
pub mod disasm;
pub mod exception;
#[cfg(feature = "inspect")]
pub mod probe;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e01_01_version_const() {
        assert!(!VERSION.is_empty());
        assert_eq!(VERSION, "0.1.0");
    }

    #[test]
    fn e01_01_no_std_attr() {
        let src = include_str!("lib.rs");
        assert!(src.contains("#![no_std]"));
        assert!(src.contains("#![forbid(unsafe_code)]"));
    }
}
