//! Instantané de débogage (T802a) : copie en lecture seule de l'état visible des CPU et des RAM.

use crate::machine::Machine;

/// Copie de l'état de la machine, envoyable à l'interface.
#[derive(Clone, Debug)]
pub struct DebugSnapshot {
    pub m68k: puce_m68k::state::CpuState,
    pub z80: puce_z80::Regs,
    pub now: u64,
    pub vpos: u16,
    /// 64 Kio ($100000-$10FFFF).
    pub work_ram: Vec<u8>,
    /// RAM du Z80 (2 Kio).
    pub z80_ram: Vec<u8>,
}

impl Machine {
    /// Copie l'état courant. Termine d'abord l'instruction du 68000 en cours.
    pub fn debug_snapshot(&mut self) -> DebugSnapshot {
        self.finish_instruction();
        DebugSnapshot {
            m68k: self.cpu.state(),
            z80: self.z80.regs,
            now: self.now,
            vpos: self.lspc.borrow().vpos,
            work_ram: self.bus.work_ram.to_vec(),
            z80_ram: self.sound.ram.to_vec(),
        }
    }

    /// Lecture d'un octet SANS effet de bord : vecteurs/BIOS, ROM P, RAM de travail ; 0xFF ailleurs.
    pub fn peek68k(&self, addr: u32) -> u8 {
        let a = (addr & 0xFF_FFFF) as usize;
        let b = &self.bus;
        match a >> 20 {
            0x0 if a < 0x80 => {
                let rom = if b.vectors_from_cart {
                    &b.cart_p
                } else {
                    &b.bios
                };
                rom.get(a).copied().unwrap_or(0xFF)
            }
            0x0 => b.cart_p.get(a).copied().unwrap_or(0xFF),
            0x1 => b.work_ram[a & 0xFFFF],
            0xC if !b.bios.is_empty() => b.bios[(a & 0xFFFFF) % b.bios.len()],
            _ => 0xFF,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::machine::tests_support::{machine_with, program_bios};

    #[test]
    fn snapshot_after_reset() {
        let mut m = machine_with(program_bios(&[]), 0);
        let s = m.debug_snapshot();
        assert_eq!(s.m68k.pc, 0x00C0_0402 + 4);
        assert_eq!(s.work_ram.len(), 0x10000);
        assert_eq!(s.z80_ram.len(), 0x800);
    }

    #[test]
    fn peek_has_no_side_effect_and_maps() {
        let m = machine_with(program_bios(&[0xAB]), 0);
        assert_eq!(m.peek68k(0xC00000), m.bus.bios[0]);
        assert_eq!(m.peek68k(0xC00402), 0xAB);
        assert_eq!(m.peek68k(0xE00000), 0xFF);
        assert_eq!(m.peek68k(0x3C0000), 0xFF);
        assert_eq!(m.peek68k(0x100000), m.bus.work_ram[0]);
    }
}
