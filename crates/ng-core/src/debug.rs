//! Instantané de débogage (T802a) : copie en lecture seule de l'état visible des CPU et des RAM.

use ng_cart::types::FixBank;
use ng_lspc::fix::{draw_fixed_line, FixBankType};

use crate::machine::Machine;

/// Largeur x hauteur de l'image de la couche fix (40 x 32 tuiles de 8 pixels).
pub const FIX_W: usize = 320;
pub const FIX_H: usize = 256;

/// Registres LSPC utiles au débogage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LspcDebug {
    pub offset: u16,
    pub modulo: u16,
    pub irq2_ctrl: u16,
    pub display_counter: u32,
    pub anim_counter: u8,
    pub anim_speed: u8,
    pub anim_disabled: bool,
    /// Niveau d'interruption demandé au 68000 (0 = aucune).
    pub irq_level: u8,
    /// Banque de palette active (0 ou 1) et mode ombre.
    pub pal_bank: u8,
    pub shadow: bool,
    /// La couche fix vient de la cartouche (sinon du BIOS).
    pub fix_from_cart: bool,
}

/// Quantité de données copiées dans un instantané : seulement ce que l'onglet visible demande.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugLevel {
    #[default]
    Off,
    /// Registres des CPU et du LSPC seulement.
    Cpu,
    /// + RAM de travail, RAM Z80, VRAM, RAM palette.
    Memory,
    /// + VRAM, RAM palette et image de la couche fix (sans les RAM des CPU).
    Video,
    /// Tout.
    Full,
}

/// Copie de l'état de la machine, envoyable à l'interface.
#[derive(Clone, Debug)]
pub struct DebugSnapshot {
    pub m68k: puce_m68k::state::CpuState,
    pub z80: puce_z80::Regs,
    /// Le 68000 est arrêté (double faute).
    pub m68k_halted: bool,
    /// Numéro de la trame en cours.
    pub frame: u64,
    pub now: u64,
    pub vpos: u16,
    /// 64 Kio ($100000-$10FFFF).
    pub work_ram: Vec<u8>,
    /// RAM du Z80 (2 Kio).
    pub z80_ram: Vec<u8>,
    /// VRAM LSPC (0x8800 mots).
    pub vram: Vec<u16>,
    /// RAM palette (0x2000 mots, deux banques).
    pub palette: Vec<u16>,
    pub lspc: LspcDebug,
    /// Couche fix seule sur fond du « backdrop », 320x256, 0x00RRGGBB.
    pub fix_image: Vec<u32>,
}

impl Machine {
    /// Copie tout l'état visible. Termine d'abord l'instruction du 68000 en cours.
    pub fn debug_snapshot(&mut self) -> DebugSnapshot {
        self.debug_snapshot_level(DebugLevel::Full)
    }

    /// Copie l'état demandé par `level` ; les tableaux non demandés sont vides.
    pub fn debug_snapshot_level(&mut self, level: DebugLevel) -> DebugSnapshot {
        self.finish_instruction();
        let (cpu_ram, video, fix) = match level {
            DebugLevel::Off | DebugLevel::Cpu => (false, false, false),
            DebugLevel::Memory => (true, true, false),
            DebugLevel::Video => (false, true, true),
            DebugLevel::Full => (true, true, true),
        };
        let fix_image = if fix {
            self.render_fix_image()
        } else {
            Vec::new()
        };
        let lspc = self.lspc.borrow();
        let pal = self.pal.borrow();
        DebugSnapshot {
            m68k: self.cpu.state(),
            z80: self.z80.regs,
            m68k_halted: self.cpu.is_halted(),
            frame: self.frame_count(),
            now: self.now,
            vpos: lspc.vpos,
            work_ram: if cpu_ram {
                self.bus.work_ram.to_vec()
            } else {
                Vec::new()
            },
            z80_ram: if cpu_ram {
                self.sound.ram.to_vec()
            } else {
                Vec::new()
            },
            vram: if video {
                lspc.vram.data.to_vec()
            } else {
                Vec::new()
            },
            palette: if video { pal.ram.to_vec() } else { Vec::new() },
            lspc: LspcDebug {
                offset: lspc.vram.get_offset(),
                modulo: lspc.vram.get_modulo(),
                irq2_ctrl: lspc.irq2_ctrl,
                display_counter: lspc.display_counter,
                anim_counter: lspc.anim_counter,
                anim_speed: lspc.anim_speed,
                anim_disabled: lspc.anim_disabled,
                irq_level: lspc.irq_level(),
                pal_bank: pal.bank,
                shadow: pal.shadow,
                fix_from_cart: self.latch.use_cart_fix_audio(),
            },
            fix_image,
        }
    }

    /// Rend la couche fix complète (256 lignes) avec la ROM S ou le fix du BIOS selon le verrou.
    fn render_fix_image(&self) -> Vec<u32> {
        let bank = match self.cart.def.prot.fix_bank {
            FixBank::Std => FixBankType::Std,
            FixBank::Garou => FixBankType::Garou,
            FixBank::Kof2000 => FixBankType::Kof2000,
        };
        let from_cart = self.latch.use_cart_fix_audio();
        let gfx = if from_cart {
            &self.cart.s
        } else {
            &self.bios.sfix
        };
        let mut img = vec![0u32; FIX_W * FIX_H];
        if gfx.is_empty() {
            return img;
        }
        let pal = self.pal.borrow();
        let pens = pal.pens_base();
        let lspc = self.lspc.borrow();
        for (y, line) in img.chunks_mut(FIX_W).enumerate() {
            let row: &mut [u32; FIX_W] = line.try_into().expect("ligne de 320 pixels");
            row.fill(pens[0xFFF]);
            draw_fixed_line(
                &lspc.vram.data[..],
                gfx,
                from_cart,
                bank,
                pens,
                y as u16,
                row,
            );
        }
        img
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
        assert_eq!(s.vram.len(), 0x8800);
        assert_eq!(s.palette.len(), 0x2000);
        assert_eq!(s.fix_image.len(), 320 * 256);
    }

    #[test]
    fn levels_copy_only_what_is_asked() {
        use super::DebugLevel;
        let mut m = machine_with(program_bios(&[]), 0);
        let s = m.debug_snapshot_level(DebugLevel::Cpu);
        assert!(s.work_ram.is_empty() && s.vram.is_empty() && s.fix_image.is_empty());
        assert_eq!(s.m68k.pc, 0x00C0_0402 + 4);
        let s = m.debug_snapshot_level(DebugLevel::Memory);
        assert_eq!((s.work_ram.len(), s.vram.len()), (0x10000, 0x8800));
        assert!(s.fix_image.is_empty());
        let s = m.debug_snapshot_level(DebugLevel::Video);
        assert!(s.work_ram.is_empty());
        assert_eq!((s.palette.len(), s.fix_image.len()), (0x2000, 320 * 256));
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
