//! Save states: a snapshot of the CPU at an instruction boundary (E17-04).

use crate::cpu::{Cpu, Variant};
use crate::engine::{Engine, IrqKind};
use crate::regs::Regs;

/// Everything needed to resume a [`Cpu`] exactly where it was, taken at an instruction boundary.
///
/// With the `serde` feature it implements `Serialize` and `Deserialize`, so an emulator can
/// write it in any serde format (JSON, bincode, ...) for save states. The mid-instruction engine
/// state is not part of it: [`Cpu::save_state`] only works between instructions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CpuState {
    /// Emulated variant.
    pub variant: Variant,
    /// Register file, including WZ, Q, IFF1/IFF2, IM and the HALT state.
    pub regs: Regs,
    /// The last instruction was `EI`: interrupt acceptance is delayed by one instruction.
    pub ei_delay: bool,
    /// The last instruction was `LD A,I` or `LD A,R` (NMOS INT bug).
    pub last_ld_air: bool,
    /// The RESET pin is currently held active.
    pub in_reset: bool,
    /// A rising edge of NMI was seen and not yet serviced.
    pub nmi_latch: bool,
    /// Previous sampled level of the NMI pin.
    pub nmi_prev: bool,
    /// Sampled level of the INT pin.
    pub int_level: bool,
    /// The bus is granted to another master (BUSAK active).
    pub bus_granted: bool,
}

impl Cpu {
    /// Snapshot of the CPU, or `None` while an instruction is in progress.
    ///
    /// Call it when [`Cpu::at_instruction_boundary`] is true (always the case after
    /// [`Cpu::step`]).
    pub fn save_state(&self) -> Option<CpuState> {
        if self.eng.active {
            return None;
        }
        Some(CpuState {
            variant: self.variant,
            regs: self.regs,
            ei_delay: self.eng.ei_delay,
            last_ld_air: self.eng.last_ld_air,
            in_reset: self.eng.in_reset,
            nmi_latch: self.eng.nmi_latch,
            nmi_prev: self.eng.nmi_prev,
            int_level: self.eng.int_level,
            bus_granted: self.eng.bus_granted,
        })
    }

    /// Restore a snapshot: the CPU is then at an instruction boundary, as it was when saved.
    pub fn load_state(&mut self, s: &CpuState) {
        self.variant = s.variant;
        self.regs = s.regs;
        self.eng = Engine::new();
        self.eng.irq = IrqKind::None;
        self.eng.ei_delay = s.ei_delay;
        self.eng.last_ld_air = s.last_ld_air;
        self.eng.in_reset = s.in_reset;
        self.eng.nmi_latch = s.nmi_latch;
        self.eng.nmi_prev = s.nmi_prev;
        self.eng.int_level = s.int_level;
        self.eng.bus_granted = s.bus_granted;
    }

    /// A new CPU restored from a snapshot.
    pub fn from_state(s: &CpuState) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.load_state(s);
        cpu
    }
}
