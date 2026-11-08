//! Cycle-accurate Zilog Z80 CPU emulator core.
//!
//! This crate provides a reusable, cycle-perfect Z80 implementation suitable for
//! console and arcade emulation. It is `no_std`, allocation-free, and contains no
//! unsafe code. It has no dependency and no internal clock: the host decides when
//! time passes. It models the NMOS Z80, including the undocumented flags and the
//! `WZ`/`Q` behaviour.
//!
//! # Two ways to drive the CPU
//!
//! * [`Cpu::step`] runs one instruction against a [`Bus`] (memory, ports, interrupts,
//!   wait states, `RETI` observation) and returns the T-states used. This is the easy way.
//! * [`Cpu::tick`] runs exactly one T-state against a set of [`Pins`]. The host wires the
//!   address/data/control pins itself: this is what a console or an arcade board with
//!   shared buses needs. `step` is built on `tick`, there is a single timing implementation.
//!
//! # Example
//!
//! ```
//! use puce_z80::{Cpu, SimpleRam};
//!
//! let mut ram = SimpleRam::new();
//! // LD A,1 ; ADD A,A ; HALT
//! ram.mem[..4].copy_from_slice(&[0x3E, 0x01, 0x87, 0x76]);
//!
//! let mut cpu = Cpu::new();
//! let mut tstates = 0;
//! for _ in 0..3 {
//!     tstates += cpu.step(&mut ram);
//! }
//! assert_eq!(cpu.regs.a, 2);
//! assert_eq!(tstates, 7 + 4 + 4);
//! ```
//!
//! # Features
//!
//! * `std`: enables `SimpleRam::boxed`.
//! * `inspect`: journal of the last ALU operation and bus activity, for debuggers.
//! * `disasm`: instruction disassembler.
//! * `serde`: `Serialize` / `Deserialize` for [`Regs`], [`Variant`] and [`CpuState`] (save states).
//!
//! Complete hosts (CP/M, ZX-like contention, shared bus, IM 2 with a CTC) are in `examples/`.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(any(test, feature = "std"))]
extern crate std;

/// Crate version string (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use bus::{Bus, Cycle, SimpleRam};
pub use cpu::{Cpu, Variant};
pub use pins::Pins;
pub use regs::Regs;
pub use state::CpuState;

// Module declarations - uncommented as each is implemented in subsequent tasks.
// Execution engine (decode, plan, mcycle).
pub(crate) mod engine;
// Pin state and accessors.
pub mod pins;
// Register file and flag constants.
pub mod regs;
// CPU top-level struct and tick interface.
pub mod cpu;

mod state;
// Bus trait and SimpleRam test implementation.
pub mod bus;
// Instruction semantics dispatch (stub, filled by E05+).
pub mod sem;
// Generated opcode decode tables.
pub(crate) mod tables;
// Planned modules (enable in the task that implements them):
// pub mod bus;     Bus trait and SimpleRam test implementation
// pub mod cpu;     Cpu struct: new/reset/tick/step/at_instruction_boundary
pub mod alu;
// pub mod sem;     Instruction semantics dispatch
// NMI, INT, HALT, RESET handling (RESET since E09-01).
pub(crate) mod int;

/// Internal API exposed for integration tests.
#[doc(hidden)]
pub mod __internal {
    pub use crate::engine::decode::plan_of;
    pub use crate::engine::decode::{decode, Decoded, Table};
}

#[cfg(feature = "disasm")]
pub mod disasm;
#[cfg(feature = "inspect")]
pub mod probe;
#[cfg(not(feature = "inspect"))]
#[path = "probe.rs"]
mod probe; // Inspection journal and history
#[cfg(test)]
mod tables_check;

#[cfg(test)]
mod tests {
    use super::*;
    use regs::flag;

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

    #[test]
    fn e01_06_pair_roundtrip() {
        let mut r = Regs::default();
        r.set_bc(0x1234);
        assert_eq!(r.b, 0x12);
        assert_eq!(r.c, 0x34);
        assert_eq!(r.bc(), 0x1234);

        r.set_af(0xABCD);
        assert_eq!(r.a, 0xAB);
        assert_eq!(r.f, 0xCD);
        assert_eq!(r.af(), 0xABCD);

        r.set_de(0x5678);
        assert_eq!(r.d, 0x56);
        assert_eq!(r.e, 0x78);
        assert_eq!(r.de(), 0x5678);

        r.set_hl(0x9ABC);
        assert_eq!(r.h, 0x9A);
        assert_eq!(r.l, 0xBC);
        assert_eq!(r.hl(), 0x9ABC);

        // Shadow registers
        r.set_bc2(0xFEDC);
        assert_eq!(r.b2, 0xFE);
        assert_eq!(r.c2, 0xDC);
        assert_eq!(r.bc2(), 0xFEDC);

        r.set_af2(0x1122);
        assert_eq!(r.a2, 0x11);
        assert_eq!(r.f2, 0x22);
        assert_eq!(r.af2(), 0x1122);

        r.set_de2(0x3344);
        assert_eq!(r.d2, 0x33);
        assert_eq!(r.e2, 0x44);
        assert_eq!(r.de2(), 0x3344);

        r.set_hl2(0x5566);
        assert_eq!(r.h2, 0x55);
        assert_eq!(r.l2, 0x66);
        assert_eq!(r.hl2(), 0x5566);
    }

    #[test]
    fn e01_06_ixh_ixl() {
        let mut r = Regs::default();
        r.set_ixh(0x12);
        r.set_ixl(0x34);
        assert_eq!(r.ix, 0x1234);
        assert_eq!(r.ixh(), 0x12);
        assert_eq!(r.ixl(), 0x34);

        // Modify high byte only - low should be preserved
        r.set_ixh(0x56);
        assert_eq!(r.ix, 0x5634);
        assert_eq!(r.ixl(), 0x34);

        // Modify low byte only - high should be preserved
        r.set_ixl(0x78);
        assert_eq!(r.ix, 0x5678);
        assert_eq!(r.ixh(), 0x56);

        // Same for IY
        r.set_iyh(0xAA);
        r.set_iyl(0xBB);
        assert_eq!(r.iy, 0xAABB);
        assert_eq!(r.iyh(), 0xAA);
        assert_eq!(r.iyl(), 0xBB);

        r.set_iyh(0xCC);
        assert_eq!(r.iy, 0xCCBB);
        assert_eq!(r.iyl(), 0xBB);

        r.set_iyl(0xDD);
        assert_eq!(r.iy, 0xCCDD);
        assert_eq!(r.iyh(), 0xCC);
    }

    #[test]
    fn e01_06_exx() {
        let mut r = Regs::default();
        r.set_bc(0x1122);
        r.set_de(0x3344);
        r.set_hl(0x5566);
        r.set_bc2(0xAABB);
        r.set_de2(0xCCDD);
        r.set_hl2(0xEEFF);
        r.a = 0x10;
        r.f = 0x20;

        r.exx();

        // BC DE HL should now hold the shadow values
        assert_eq!(r.bc(), 0xAABB);
        assert_eq!(r.de(), 0xCCDD);
        assert_eq!(r.hl(), 0xEEFF);

        // Shadow registers should now hold the original values
        assert_eq!(r.bc2(), 0x1122);
        assert_eq!(r.de2(), 0x3344);
        assert_eq!(r.hl2(), 0x5566);

        // AF should be unchanged
        assert_eq!(r.a, 0x10);
        assert_eq!(r.f, 0x20);
    }

    #[test]
    fn e01_06_ex_af() {
        let mut r = Regs {
            a: 0x10,
            f: 0x20,
            a2: 0x30,
            f2: 0x40,
            b: 0xFF,
            ..Default::default()
        };

        r.ex_af();

        assert_eq!(r.a, 0x30);
        assert_eq!(r.f, 0x40);
        assert_eq!(r.a2, 0x10);
        assert_eq!(r.f2, 0x20);
        // Other registers unchanged
        assert_eq!(r.b, 0xFF);
    }

    #[test]
    fn e01_06_ex_de_hl() {
        let mut r = Regs::default();
        r.set_de(0x1122);
        r.set_hl(0x3344);
        r.b = 0xFF;

        r.ex_de_hl();

        assert_eq!(r.de(), 0x3344);
        assert_eq!(r.hl(), 0x1122);
        // Other registers unchanged
        assert_eq!(r.b, 0xFF);
    }

    #[test]
    fn e01_06_flag_values() {
        assert_eq!(flag::S, 0x80);
        assert_eq!(flag::Z, 0x40);
        assert_eq!(flag::Y, 0x20);
        assert_eq!(flag::H, 0x10);
        assert_eq!(flag::X, 0x08);
        assert_eq!(flag::PV, 0x04);
        assert_eq!(flag::N, 0x02);
        assert_eq!(flag::C, 0x01);

        // All flags are disjoint (no overlapping bits)
        let all = flag::S | flag::Z | flag::Y | flag::H | flag::X | flag::PV | flag::N | flag::C;
        assert_eq!(all, 0xFF);

        // Verify no overlap by checking pairwise AND is zero for distinct flags
        let flags = [
            flag::S,
            flag::Z,
            flag::Y,
            flag::H,
            flag::X,
            flag::PV,
            flag::N,
            flag::C,
        ];
        for i in 0..flags.len() {
            for j in (i + 1)..flags.len() {
                assert_eq!(flags[i] & flags[j], 0);
            }
        }
    }
}
