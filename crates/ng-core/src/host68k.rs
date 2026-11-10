//! Hôte du 68000 : bus cadencé à l'horloge et rattrapage (catch-up) des autres puces.
//!
//! Le 68000 est maître : `now` (mclk) est le début de l'horloge courante, `on_tick` fait `now += 2`.
//! Chaque accès bus rattrape d'abord LSPC, Z80, YM2610, RTC et watchdog jusqu'à `now`.
//! `wait_clocks` reste à la valeur par défaut (0) jusqu'à T601.

use std::cell::RefCell;
use std::rc::Rc;

use ng_bus::backup::BackupRam;
use ng_bus::rtc::Upd4990a;
use ng_bus::{Bus68k, Io, SoundBoard, SystemLatch, Watchdog};
use ng_cart::types::FixBank;
use ng_cart::{Bios, Cart};
use ng_common::Framebuffer;
use ng_lspc::fix::FixBankType;
use ng_lspc::palette::Palette;
use ng_lspc::render::LspcRoms;
use ng_lspc::Lspc;
use puce_m68k::bus::Bus;
use puce_m68k::cpu::Cpu;
use puce_m68k::pins::Pins;

use crate::machine::{Machine, Stats};
use crate::waits::bus_wait;
use crate::ym_adapter::YmAdapter;

/// Références disjointes vers les champs de `Machine` autres que `cpu`.
pub struct Host68k<'a> {
    pub bus: &'a mut Bus68k,
    pub sound: &'a mut SoundBoard<YmAdapter>,
    pub z80: &'a mut puce_z80::Cpu,
    pub lspc: &'a Rc<RefCell<Lspc>>,
    pub pal: &'a Rc<RefCell<Palette>>,
    pub io: &'a Rc<RefCell<Io>>,
    pub backup: &'a Rc<RefCell<BackupRam>>,
    pub latch: &'a mut SystemLatch,
    pub rtc: &'a mut Upd4990a,
    pub wd: &'a mut Watchdog,
    pub cart: &'a Cart,
    pub bios: &'a Bios,
    pub fb: &'a mut Framebuffer,
    pub now: &'a mut u64,
    pub synced: &'a mut u64,
    pub reset_request: &'a mut bool,
    pub stats: &'a mut Stats,
    pub lazy: bool,
    /// Override pour les tests : si Some(f), wait_clocks appelle f(addr) au lieu de bus_wait.
    #[cfg(test)]
    pub wait_override: Option<Box<dyn Fn(u32) -> u8>>,
}

impl Machine {
    /// Sépare le CPU de l'hôte (qui emprunte tout le reste).
    pub fn split(&mut self) -> (&mut Cpu, Host68k<'_>) {
        (
            &mut self.cpu,
            Host68k {
                bus: &mut self.bus,
                sound: &mut self.sound,
                z80: &mut self.z80,
                lspc: &self.lspc,
                pal: &self.pal,
                io: &self.io,
                backup: &self.backup,
                latch: &mut self.latch,
                rtc: &mut self.rtc,
                wd: &mut self.wd,
                cart: &self.cart,
                bios: &self.bios,
                fb: &mut self.fb,
                now: &mut self.now,
                synced: &mut self.synced,
                reset_request: &mut self.reset_request,
                stats: &mut self.stats,
                lazy: self.lazy_catch_up,
                #[cfg(test)]
                wait_override: None,
            },
        )
    }
}

impl Host68k<'_> {
    /// Fait avancer le LSPC (et rend les lignes atteintes) jusqu'à `now`.
    fn lspc_advance(&mut self) {
        let pal = self.pal.borrow();
        let fix_bank = match self.cart.def.prot.fix_bank {
            FixBank::Std => FixBankType::Std,
            FixBank::Garou => FixBankType::Garou,
            FixBank::Kof2000 => FixBankType::Kof2000,
        };
        let roms = LspcRoms {
            c: &self.cart.c,
            s_cart: &self.cart.s,
            s_bios: &self.bios.sfix,
            lo: &self.bios.lo,
            use_cart_fix: self.latch.use_cart_fix_audio(),
            fix_bank,
            pal: &pal,
        };
        self.lspc
            .borrow_mut()
            .advance_render(*self.now, &roms, self.fb);
    }

    /// Fait tourner le Z80 T-state par T-state jusqu'à `t` (jamais au-delà).
    pub fn z80_run_until(&mut self, t: u64) {
        while self.sound.time + 6 <= t {
            self.sound.tick(self.z80);
            self.stats.z80_ticks += 1;
        }
    }

    /// Un accès à `addr` doit-il rattraper les périphériques ? Non pour la ROM P, la RAM de travail
    /// et le BIOS (aucun périphérique n'y est observable), tant que le watchdog n'expire pas.
    fn needs_sync(&self, addr: u32) -> bool {
        if !self.lazy {
            return true;
        }
        match (addr & 0xFF_FFFF) >> 20 {
            0x0 | 0x1 | 0xC => *self.now - *self.synced >= self.wd.headroom(),
            _ => true,
        }
    }

    /// Rattrape LSPC, Z80, YM2610, RTC et watchdog jusqu'à `now`.
    pub fn catch_up(&mut self) {
        self.stats.catch_ups += 1;
        let now = *self.now;
        self.lspc_advance();
        // Le YM n'est plus avancé ici : `SoundBoard::tick` l'amène à l'instant du Z80 (T900b), ce qui
        // rend le résultat indépendant de la fréquence des rattrapages. Fin de trame : `run_frame`.
        self.z80_run_until(now);
        let delta = now.saturating_sub(*self.synced);
        if delta > 0 {
            self.rtc.advance(delta);
            if self.wd.advance(delta) {
                *self.reset_request = true;
            }
            *self.synced = now;
        }
        self.sync_io_inputs();
    }

    /// Recopie dans `Io` les sorties du uPD4990A (TP, DATA OUT : $320001 bits 6 et 7) et la réponse son.
    fn sync_io_inputs(&mut self) {
        let mut io = self.io.borrow_mut();
        io.rtc_tp = self.rtc.tp();
        io.rtc_data_out = self.rtc.data_out();
        // Réponse du Z80 (port $0C) lue par le 68000 en $320000.
        io.sound_reply = self.sound.reply;
    }

    /// REG_RTCCTRL ($380051) : bit 0 DIN, bit 1 CLK, bit 2 STB.
    fn rtc_control(&mut self, addr: u32, lds: bool, data: u16) {
        if lds && addr & 0xFE_0000 == 0x38_0000 && (addr >> 1) & 0x78 == 0x28 {
            self.rtc
                .set_inputs(data & 4 != 0, data & 2 != 0, data & 1 != 0);
            self.sync_io_inputs();
        }
    }

    /// Effets de bord d'une écriture : commande son, verrou système, coup de chien de garde.
    fn after_write(&mut self, addr: u32) {
        let cmd = self.io.borrow_mut().pending_sound_cmd.take();
        if let Some(v) = cmd {
            self.sound.host_write_cmd(v);
        }
        if (0x3A_0000..0x3C_0000).contains(&(addr & 0xFF_FFFF)) {
            self.latch.write(addr);
            self.bus.vectors_from_cart = self.latch.use_cart_vectors();
            self.sound.use_cart = self.latch.use_cart_fix_audio();
            let mut pal = self.pal.borrow_mut();
            pal.bank = self.latch.palette_bank();
            pal.shadow = self.latch.shadow();
            self.backup.borrow_mut().unlocked = self.latch.sram_unlocked();
        }
        let kick = std::mem::take(&mut self.io.borrow_mut().watchdog_kick);
        if kick {
            self.wd.kick();
        }
    }
}

impl Bus for Host68k<'_> {
    fn read(&mut self, addr: u32, _fc: u8, _uds: bool, _lds: bool) -> u16 {
        if self.needs_sync(addr) {
            self.catch_up();
        }
        self.bus.read16(addr)
    }

    fn write(&mut self, addr: u32, _fc: u8, uds: bool, lds: bool, data: u16) {
        if self.needs_sync(addr) {
            self.catch_up();
        }
        self.bus.write16(addr, data, uds, lds);
        self.rtc_control(addr, lds, data);
        self.after_write(addr);
    }

    fn ipl(&mut self) -> u8 {
        self.lspc_advance();
        self.lspc.borrow().irq_level()
    }

    #[cfg(test)]
    fn wait_clocks(&mut self, addr: u32, _write: bool) -> u8 {
        if let Some(ref f) = self.wait_override {
            return f(addr);
        }
        bus_wait(addr, _write) as u8
    }

    #[cfg(not(test))]
    fn wait_clocks(&mut self, addr: u32, write: bool) -> u8 {
        bus_wait(addr, write) as u8
    }

    fn int_ack(&mut self, _level: u8) -> Option<u8> {
        None
    }

    fn on_tick(&mut self, _pins: Pins) {
        *self.now += 2;
    }
}
