//! Boucle de trame : le 68000 avance une horloge à la fois, le reste suit par catch-up.

use ng_bus::Ym;
use ng_common::MCLK_PER_FRAME;

use crate::machine::Machine;

impl Machine {
    /// Exécute une trame (405 504 mclk). Un reset demandé par le watchdog est traité entre deux horloges.
    pub fn run_frame(&mut self) {
        let mut target = self.frame_start + MCLK_PER_FRAME;
        while self.now < target {
            {
                let (cpu, mut host) = self.split();
                if cpu.step_clock(&mut host).is_none() {
                    // CPU arrêté (double faute) : le temps passe quand même
                    *host.now += 2;
                }
            }
            if self.reset_request {
                self.reset();
                target = self.frame_start + MCLK_PER_FRAME;
            }
        }
        {
            let (_, mut host) = self.split();
            host.catch_up();
            let now = *host.now;
            host.sound.ym.advance(now);
        }
        self.frame_start = target;
        self.audio.append(&mut self.sound.ym.samples);
    }

    /// Termine l'instruction en cours (avant un savestate ou un instantané de débogueur).
    pub fn finish_instruction(&mut self) {
        while !self.cpu.at_instruction_boundary() {
            let (cpu, mut host) = self.split();
            cpu.step_clock(&mut host);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::machine::tests_support::{machine_with, program_bios};
    use ng_bus::Watchdog;
    use puce_m68k::bus::Bus;

    /// RAM, registres LSPC et chien de garde mêlés : le rattrapage paresseux ne doit rien changer.
    const MIXED: [u8; 22] = [
        0x33, 0xC0, 0x00, 0x10, 0x00, 0x00, // move.w d0,($100000).l
        0x52, 0x40, // addq.w #1,d0
        0x30, 0x39, 0x00, 0x3C, 0x00, 0x06, // move.w ($3C0006).l,d0
        0x33, 0xC0, 0x00, 0x30, 0x00, 0x00, // move.w d0,($300000).l
        0x60, 0xEA, // bra début
    ];

    #[test]
    fn lazy_catch_up_gives_identical_state() {
        let mut eager = machine_with(program_bios(&MIXED), 0x00);
        eager.lazy_catch_up = false;
        let mut lazy = machine_with(program_bios(&MIXED), 0x00);
        for _ in 0..5 {
            eager.run_frame();
            lazy.run_frame();
        }
        assert!(lazy.stats.catch_ups < eager.stats.catch_ups);
        assert_eq!(eager.framebuffer().pixels, lazy.framebuffer().pixels);
        assert_eq!(eager.save_state(), lazy.save_state());
    }

    #[test]
    fn nop_loop_runs_a_full_frame() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        m.run_frame();
        assert!(m.now >= 405_504);
        assert!(m.lspc.borrow().time() >= 405_504);
        assert_eq!(m.frame_start, 405_504);
    }

    #[test]
    fn on_tick_advances_two_mclk() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        let before = m.now;
        let (cpu, mut host) = m.split();
        cpu.step_clock(&mut host);
        assert_eq!(*host.now, before + 2);
    }

    #[test]
    fn open_bus_returns_last_word_only_when_enabled() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71]), 0x00);
        m.bus.work_ram[0] = 0x12;
        m.bus.work_ram[1] = 0x34;
        {
            let (_, mut host) = m.split();
            host.read(0x100000, 1, true, true);
            assert_eq!(host.read(0xE00000, 1, true, true), 0xFFFF);
        }
        m.open_bus_follows_prefetch = true;
        let (_, mut host) = m.split();
        host.read(0x100000, 1, true, true);
        assert_eq!(host.read(0xE00000, 1, true, true), 0x1234);
    }

    #[test]
    fn sound_command_is_visible_immediately() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        let (_, mut host) = m.split();
        // move.b d0,$320000 : adresse paire, voie haute (uds), donnée en D8-D15
        host.write(0x320000, 1, true, false, 0x5500);
        assert!(host.sound.cmd_pending);
        assert_eq!(host.sound.cmd, 0x55);
    }

    #[test]
    fn byte_write_at_odd_address_kicks_the_watchdog() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        m.wd.advance(1000);
        let (_, mut host) = m.split();
        host.write(0x300000, 1, false, true, 0x0000); // move.b d0,$300001
        assert_eq!(host.wd.counter, 0);
    }

    #[test]
    fn z80_never_overshoots_now() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        m.now = 6000;
        let (_, mut host) = m.split();
        host.catch_up();
        assert_eq!(host.sound.time, 6000);
        *host.now = 6005;
        host.catch_up();
        assert_eq!(host.sound.time, 6000);
    }

    #[test]
    fn latch_write_selects_cart_vectors() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        let (_, mut host) = m.split();
        // A1-A3 = 1 (Q1), A4 = 1 : $3A0013 active les vecteurs de la cartouche
        host.write(0x3A0013, 1, false, true, 0x0000);
        assert!(host.bus.vectors_from_cart);
        host.write(0x3A0003, 1, false, true, 0x0000);
        assert!(!host.bus.vectors_from_cart);
    }

    #[test]
    fn watchdog_times_out_after_3_244_030_mclk() {
        let mut wd = Watchdog::default();
        assert!(!wd.advance(3_244_029));
        assert!(wd.advance(1));
    }

    #[test]
    fn watchdog_requests_a_reset_in_the_machine() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        m.now = 3_300_000;
        let (_, mut host) = m.split();
        host.catch_up();
        assert!(*host.reset_request);
    }

    #[test]
    fn run_frame_survives_a_watchdog_reset() {
        let mut m = machine_with(program_bios(&[0x4E, 0x71, 0x60, 0xFC]), 0x00);
        for _ in 0..9 {
            m.run_frame(); // 9 trames > 3 244 030 mclk sans kick
        }
        assert!(m.frame_start > 0);
    }
}
