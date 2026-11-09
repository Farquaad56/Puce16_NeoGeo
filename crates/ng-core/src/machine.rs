//! La machine Neo Geo : 68000 maître, Z80 son, bus, LSPC, YM2610, cartouche et BIOS.
//!
//! T600a : structure, construction et reset. L'ordonnanceur (catch-up, `run_frame`) arrive en T600b/c ;
//! `Host68k` n'a ici que le strict nécessaire pour dérouler la séquence de reset du 68000
//! (lecture des vecteurs, 40 horloges) et sera étendu par T600b.

use std::cell::RefCell;
use std::rc::Rc;

use ng_bus::backup::BackupRam;
use ng_bus::memcard::MemCardStub;
use ng_bus::rtc::Upd4990a;
use ng_bus::{Bus68k, Io, SoundBoard, SystemLatch, Watchdog};
use ng_cart::{Bios, Cart, Model};
use ng_common::Framebuffer;
use ng_lspc::palette::Palette;
use ng_lspc::Lspc;
use puce_m68k::cpu::Cpu;

use crate::shared::{Shared, SharedLspc, SharedPalette};
use crate::ym_adapter::YmAdapter;

/// Nombre maximal d'horloges pour terminer un reset (garde contre une boucle infinie).
const RESET_CLOCK_LIMIT: u32 = 1000;

pub struct Machine {
    pub cpu: Cpu,
    pub z80: puce_z80::Cpu,
    /// Bus 68000 : contient des adaptateurs vers io, lspc, palette, backup.
    pub bus: Bus68k,
    /// Carte son : Z80 + YM2610.
    pub sound: SoundBoard<YmAdapter>,
    // Périphériques partagés entre le bus et la machine (pas de threads dans le coeur).
    pub lspc: Rc<RefCell<Lspc>>,
    pub pal: Rc<RefCell<Palette>>,
    pub io: Rc<RefCell<Io>>,
    pub backup: Rc<RefCell<BackupRam>>,
    /// Verrou système $3A0000.
    pub latch: SystemLatch,
    pub rtc: Upd4990a,
    /// Cartouche (les régions P, M et V ont été déplacées vers le bus et la carte son).
    pub cart: Cart,
    /// BIOS (les régions main et sm1 ont été déplacées vers le bus et la carte son).
    pub bios: Bios,
    pub model: Model,
    /// Temps 68000 en mclk depuis le reset (début de l'horloge courante).
    pub now: u64,
    pub frame_start: u64,
    pub wd: Watchdog,
    pub fb: Framebuffer,
    /// Échantillons audio produits pendant la trame.
    pub audio: Vec<[f32; 2]>,
    /// Dernier instant (mclk) auquel RTC et watchdog ont été rattrapés.
    pub synced: u64,
    /// Le watchdog a expiré : reset demandé à la fin de l'horloge courante.
    pub reset_request: bool,
}

impl Machine {
    /// Construit la machine ; appelle `reset()`.
    pub fn new(mut cart: Cart, mut bios: Bios, model: Model) -> Machine {
        // périphériques partagés
        let lspc = Rc::new(RefCell::new(Lspc::new()));
        let pal = Rc::new(RefCell::new(Palette::new()));
        let io = Rc::new(RefCell::new(Io {
            model: model.clone(),
            ..Io::default()
        }));
        let backup = Rc::new(RefCell::new(BackupRam::new()));

        // bus : BIOS et P ROM sont déplacées dans le bus
        let mut bus = Bus68k::new(std::mem::take(&mut bios.main), std::mem::take(&mut cart.p));
        bus.model = model.clone();
        bus.io = Box::new(Shared(io.clone()));
        bus.lspc = Box::new(SharedLspc(lspc.clone()));
        bus.palette = Box::new(SharedPalette(pal.clone()));
        bus.backup = Box::new(Shared(backup.clone()));
        bus.memcard = Box::new(MemCardStub);

        // carte son : sm1 du BIOS, M1 de la cartouche, V ROM pour l'ADPCM (A et B)
        let mut sound = SoundBoard::new(YmAdapter::new(std::mem::take(&mut cart.v)));
        sound.sm1 = std::mem::take(&mut bios.sm1);
        sound.m1 = std::mem::take(&mut cart.m);

        let mut machine = Machine {
            cpu: Cpu::default(),
            z80: puce_z80::Cpu::new(),
            bus,
            sound,
            lspc,
            pal,
            io,
            backup,
            latch: SystemLatch::default(),
            rtc: Upd4990a::new(),
            cart,
            bios,
            model: model.clone(),
            now: 0,
            frame_start: 0,
            wd: Watchdog::default(),
            fb: Framebuffer::new(),
            audio: Vec::new(),
            synced: 0,
            reset_request: false,
        };
        machine.reset();
        machine
    }

    /// Reset complet : 68000 (vecteurs lus dans le BIOS), Z80, YM, LSPC, verrou, watchdog, temps à zéro.
    pub fn reset(&mut self) {
        // état du bus et des périphériques
        self.bus.vectors_from_cart = false;
        self.latch = SystemLatch::default();
        self.wd = Watchdog::default();
        self.rtc = Upd4990a::new();
        *self.lspc.borrow_mut() = Lspc::new(); // IRQ3 en attente
        self.audio.clear();
        self.now = 0;
        self.frame_start = 0;
        self.synced = 0;
        self.reset_request = false;

        // carte son : Z80 puis YM
        self.z80 = puce_z80::Cpu::new();
        self.z80.reset();
        self.sound.reset();
        self.sound.time = 0;
        self.sound.ym.reset();

        // 68000 : reset puis première frontière d'instruction
        self.cpu = Cpu::default();
        self.cpu.reset();
        self.finish_reset();
    }

    /// Déroule les horloges du reset du 68000 jusqu'à la première frontière d'instruction
    /// (40 horloges = 80 mclk ; les vecteurs sont lus dans le BIOS car `vectors_from_cart` est faux).
    fn finish_reset(&mut self) {
        for _ in 0..RESET_CLOCK_LIMIT {
            let (cpu, mut host) = self.split();
            cpu.step_clock(&mut host);
            if cpu.at_instruction_boundary() {
                return;
            }
        }
        panic!("le reset du 68000 ne se termine pas");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ng_cart::find_def;

    /// BIOS synthétique de 0x20000 octets : SP = 0x10F300, PC = 0xC00402.
    fn synthetic_bios() -> Bios {
        let mut main = vec![0u8; 0x20000];
        main[0..4].copy_from_slice(&0x0010_F300u32.to_be_bytes());
        main[4..8].copy_from_slice(&0x00C0_0402u32.to_be_bytes());
        Bios {
            main,
            lo: Vec::new(),
            sfix: Vec::new(),
            sm1: vec![0xAA; 0x1000],
        }
    }

    fn synthetic_cart() -> Cart {
        Cart {
            def: find_def("mslug").expect("definition mslug"),
            p: vec![0x12; 0x1000],
            s: vec![0; 0x100],
            m: vec![0x34; 0x2000],
            m_crypt: Vec::new(),
            v: vec![0x56; 0x400],
            c: vec![0; 0x100],
        }
    }

    fn machine() -> Machine {
        Machine::new(synthetic_cart(), synthetic_bios(), Model::Mvs)
    }

    #[test]
    fn construction_reads_reset_vectors_from_the_bios() {
        let m = machine();
        let s = m.cpu.state();
        assert_eq!(s.ssp, 0x0010_F300);
        assert_eq!(s.pc, 0x00C0_0402 + 4);
        assert_eq!(m.now, 80);
        assert!(m.cpu.at_instruction_boundary());
        assert!(!m.cpu.is_halted());
    }

    #[test]
    fn regions_are_moved_to_their_owners() {
        let m = machine();
        assert_eq!(m.bus.bios.len(), 0x20000);
        assert_eq!(m.bus.cart_p.len(), 0x1000);
        assert_eq!(m.sound.sm1.len(), 0x1000);
        assert_eq!(m.sound.m1.len(), 0x2000);
        assert!(m.cart.p.is_empty() && m.cart.m.is_empty() && m.cart.v.is_empty());
        assert!(m.bios.main.is_empty() && m.bios.sm1.is_empty());
        assert_eq!(m.sound.ym.intf().borrow().adpcm_rom.len(), 0x400);
        assert_eq!(m.model, Model::Mvs);
    }

    #[test]
    fn bus_devices_are_the_shared_ones() {
        let mut m = machine();
        // palette
        m.bus.write16(0x400002, 0x7FFF, true, true);
        assert_eq!(m.pal.borrow().read16(1), 0x7FFF);
        // LSPC : modulo
        m.bus.write16(0x3C0004, 0x0040, true, true);
        assert_eq!(m.lspc.borrow().vram.get_modulo(), 0x0040);
        // E/S : réponse son visible dans le registre de statut
        m.io.borrow_mut().sound_reply = 0x5A;
        assert_eq!(m.bus.read16(0x320000) >> 8, 0x5A);
        // RAM de sauvegarde, verrouillée puis déverrouillée
        m.backup.borrow_mut().unlocked = true;
        m.bus.write16(0xD00000, 0xBEEF, true, true);
        assert_eq!(m.backup.borrow().data[0], 0xBE);
    }

    #[test]
    fn reset_restores_a_clean_state() {
        let mut m = machine();
        m.bus.write16(0x3C0004, 0x0040, true, true);
        m.bus.vectors_from_cart = true;
        m.audio.push([1.0, 1.0]);
        m.sound.time = 1234;
        m.reset();
        assert_eq!(m.now, 80);
        assert_eq!(m.frame_start, 0);
        assert!(!m.bus.vectors_from_cart);
        assert!(m.audio.is_empty());
        assert!(m.sound.time <= m.now && m.sound.time > 1234 - 1234);
        assert_eq!(m.lspc.borrow().vram.get_modulo(), 0);
        assert_eq!(m.cpu.state().pc, 0x00C0_0402 + 4);
    }
}

/// Aides de test partagées avec les autres modules du crate.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::*;
    use ng_cart::find_def;

    /// BIOS de 0x20000 octets : SP = 0x10F300, PC = 0xC00402, `code` copié en 0x402.
    pub fn program_bios(code: &[u8]) -> Bios {
        let mut main = vec![0u8; 0x20000];
        main[0..4].copy_from_slice(&0x0010_F300u32.to_be_bytes());
        main[4..8].copy_from_slice(&0x00C0_0402u32.to_be_bytes());
        main[0x402..0x402 + code.len()].copy_from_slice(code);
        Bios {
            main,
            lo: vec![0; 0x20000],
            sfix: vec![0; 0x20000],
            sm1: vec![0; 0x1000],
        }
    }

    /// Machine MVS avec la cartouche synthétique ; `fill` remplit M1 (0x00 = NOP pour le Z80).
    pub fn machine_with(bios: Bios, fill: u8) -> Machine {
        let cart = Cart {
            def: find_def("mslug").expect("definition mslug"),
            p: vec![0x12; 0x1000],
            s: vec![0; 0x100],
            m: vec![fill; 0x2000],
            m_crypt: Vec::new(),
            v: vec![0x56; 0x400],
            c: vec![0; 0x100],
        };
        Machine::new(cart, bios, Model::Mvs)
    }
}
