//! Neo Geo MVS I/O registers ($3x0000 range).
//!
//! All reads/writes are 16-bit words except where noted.
//! Active-low convention for buttons (1 = released, 0 = pressed).

use crate::device::Device;
use ng_cart::Model;
use ng_common::{PadState, SystemButtons};

/// I/O register state.
pub struct Io {
    /// Machine model (MVS arcade or AES console).
    pub model: Model,
    /// Player pad states.
    pub pad: [PadState; 2],
    /// System buttons (start, select, coins, service, test).
    pub system: SystemButtons,
    /// Test button pressed (separate from system bits for REG_SYSTYPE).
    pub test_pressed: bool,
    /// Sound CPU reply byte.
    pub sound_reply: u8,
    /// Watchdog was kicked by write to $300001.
    pub watchdog_kick: bool,
    /// Pending sound command from write to $320000.
    pub pending_sound_cmd: Option<u8>,
    /// DIP switch setting (default 0xFF = all off).
    pub dipsw: u8,
    /// RTC TP pulse flag.
    pub rtc_tp: bool,
    /// RTC data out bit.
    pub rtc_data_out: bool,
    /// Memory card bank select.
    pub card_bank: u8,
    /// Une carte mémoire est insérée (bits 12-14 de $380000 à 0).
    pub card_present: bool,
    /// Panneau mahjong branché sur le port 1 (jeux `Controller::Mahjong`, MVS).
    pub mahjong: bool,
    /// Touches du panneau mahjong, 3 rangées (MJ.0 : A-G, MJ.1 : H-N, MJ.2 : Pon, Chi, Kan, Ron,
    /// Reach en bits 0-4), bit à 0 = pressée.
    pub mj: [u8; 3],
    /// Dernier octet écrit en `REG_POUTPUT` ($380001) : sélection des rangées du panneau.
    pub mj_sel: u8,
}

impl Default for Io {
    fn default() -> Self {
        Io {
            model: Model::Mvs,
            pad: [PadState::default(), PadState::default()],
            system: SystemButtons::default(),
            test_pressed: false,
            sound_reply: 0x00,
            watchdog_kick: false,
            pending_sound_cmd: None,
            dipsw: 0xFF,
            rtc_tp: false,
            rtc_data_out: false,
            card_bank: 0,
            card_present: false,
            mahjong: false,
            mj: [0xFF; 3],
            mj_sel: 0,
        }
    }
}

impl Io {
    /// `neogeo_mjctrl_ctrl_device::read_ctrl` (MAME) : 0xFF puis ET des rangées sélectionnées par
    /// les bits 0, 1 et 2 de `mj_sel` (une touche pressée met son bit à 0).
    pub fn mahjong_read_ctrl(&self) -> u8 {
        let mut d = 0xFFu8;
        for (i, row) in self.mj.iter().enumerate() {
            if self.mj_sel & (1 << i) != 0 {
                d &= row;
            }
        }
        d
    }

    /// Bits 12-14 de $380000 : 0x7000 = pas de carte, 0 = carte présente.
    fn card_bits(&self) -> u16 {
        if self.card_present {
            0
        } else {
            0x7000
        }
    }
}

impl Device for Io {
    fn read(&mut self, addr: u32) -> u16 {
        // Decode by high byte (mask $FE0000)
        let region = (addr >> 16) & 0xFE;

        match region {
            0x30 => {
                if addr & 0x80 == 0 {
                    // REG_P1CNT / REG_DIPSW at $300000
                    if self.model == Model::Aes {
                        // AES: no DIP switches, lower byte always 0xFF
                        ((self.pad[0].bits as u16) << 8) | 0xFF
                    } else if self.mahjong {
                        // Panneau mahjong : rangées sélectionnées ; DIP « Controller » (0x04) = 0
                        // (« Mahjong Panel », défaut de `INPUT_PORTS_START( neogeo_mj )`).
                        ((self.mahjong_read_ctrl() as u16) << 8) | (self.dipsw & !0x04) as u16
                    } else {
                        // MVS: lower byte is DIP switch setting
                        ((self.pad[0].bits as u16) << 8) | (self.dipsw as u16)
                    }
                } else {
                    // REG_SYSTYPE at $300080
                    if self.model == Model::Aes {
                        // AES: no TEST port, return open bus (0xFF00)
                        0xFF00
                    } else {
                        let test_bit = if self.test_pressed { 0 } else { 0x80 };
                        0xFF00 | 0x40 | test_bit
                    }
                }
            }
            0x32 => {
                // REG_SOUND / REG_STATUS_A at $320000
                if self.model == Model::Aes {
                    // AES: no RTC, no coin bits - just sound reply in upper byte
                    (self.sound_reply as u16) << 8
                } else {
                    let mut status_a: u16 = 0;
                    // Bits 0-4: coin1, coin2, service, coin3, coin4 (active-low)
                    if self.system.bits & (1 << 4) != 0 {
                        status_a |= 1;
                    } // coin1 released
                    if self.system.bits & (1 << 5) != 0 {
                        status_a |= 2;
                    } // coin2 released
                    if self.system.bits & (1 << 6) != 0 {
                        status_a |= 4;
                    } // service released
                      // Bits 3-4: coin3, coin4 - not in SystemButtons, always released
                    status_a |= 8 | 16;
                    // Bit 5: slot type (MVS = 0)
                    // Bit 6: RTC TP
                    if self.rtc_tp {
                        status_a |= 64;
                    }
                    // Bit 7: RTC data out
                    if self.rtc_data_out {
                        status_a |= 128;
                    }

                    ((self.sound_reply as u16) << 8) | status_a
                }
            }
            0x34 => {
                // REG_P2CNT at $340000
                if self.model == Model::Aes {
                    // AES: no DIP switches, lower byte always 0xFF
                    ((self.pad[1].bits as u16) << 8) | 0xFF
                } else {
                    ((self.pad[1].bits as u16) << 8) | 0xFF
                }
            }
            0x38 => {
                // REG_STATUS_B at $380000
                if self.model == Model::Aes {
                    // AES: bit 15 = 0, start/select in bits 8-11
                    let sys_bits = (self.system.bits & 0x0F) as u16;
                    0x00FF
                        | ((sys_bits & 0x03) << 8)
                        | (((sys_bits >> 2) & 0x03) << 10)
                        | self.card_bits()
                } else {
                    // MVS: bit 15 = 1
                    let mut sys_bits = (self.system.bits & 0x0F) as u16;
                    // Panneau mahjong : SELECT masqué (relâché) quand le bit 2 de la sélection est à 1.
                    if self.mahjong && self.mj_sel & 4 != 0 {
                        sys_bits |= 2;
                    }
                    0x00FF | (sys_bits << 8) | self.card_bits() | 0x8000
                }
            }
            _ => {
                // Unmapped I/O region - open bus
                0xFFFF
            }
        }
    }

    fn write(&mut self, addr: u32, data: u16, _upper: bool, _lower: bool) {
        // I/O register decoding is based on address parity, not byte select flags.
        // Even addresses use lower byte; odd addresses use upper byte.
        let byte = if addr & 1 == 0 {
            (data & 0xFF) as u8
        } else {
            ((data >> 8) & 0xFF) as u8
        };

        // Watchdog kick on any write to $300001 (mirror $FE0081) - MVS only
        if addr & 0xFE0081 == 0x300001 {
            if self.model == Model::Mvs {
                self.watchdog_kick = true;
            }
            // AES: no watchdog, ignore write
            return;
        }

        // Sound command on even address write to $320000
        if (addr >> 16) & 0xFE == 0x32 && addr & 1 == 0 {
            self.pending_sound_cmd = Some(byte);
            return;
        }

        // Port outputs at $3800xx odd addresses
        if (addr >> 16) & 0xFE == 0x38 && addr & 1 != 0 {
            let off = (addr >> 1) & 0x78;
            match off {
                0x00 => self.mj_sel = byte, // REG_POUTPUT : sélection des rangées du panneau mahjong
                0x08 => {
                    self.card_bank = byte & 7;
                } // REG_CRDBANK
                0x10 => { /* REG_SLOT - slot select, ignored */ }
                0x18 => { /* REG_LEDLATCHES - LED latch, ignored */ }
                0x20 => { /* REG_LEDDATA - LED data, ignored */ }
                0x28 => {
                    // REG_RTCCTRL
                    self.rtc_data_out = byte & 1 == 1;
                    // RTC clock and strobe would be handled by T304
                }
                0x30 | 0x70 => { /* Coin counters/locks, ignored */ }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_io() -> Io {
        Io::default()
    }

    #[test]
    fn mahjong_rows_follow_selection() {
        // MAME neogeo_mj : rangée 0 = A-G (bits 0-6), rangée 1 = H-N, rangée 2 = Pon, Chi, Kan, Ron, Reach.
        let mut io = Io {
            mahjong: true,
            ..Io::default()
        };
        // repos : tout à 1 quelle que soit la sélection
        assert_eq!(io.read(0x300000) >> 8, 0xFF);
        io.mj[0] = !0x01; // touche A pressée
        io.mj[1] = !0x04; // touche J pressée
        io.mj[2] = !0x08; // Ron
        io.write(0x380001, 0x0000, true, false); // POUTPUT = 0 -> aucune rangée
        assert_eq!(io.read(0x300000) >> 8, 0xFF);
        io.write(0x380001, 0x0100, true, false);
        assert_eq!(io.mj_sel, 1);
        assert_eq!(io.read(0x300000) >> 8, 0xFE);
        io.write(0x380001, 0x0200, true, false);
        assert_eq!(io.read(0x300000) >> 8, 0xFB);
        io.write(0x380001, 0x0400, true, false);
        assert_eq!(io.read(0x300000) >> 8, 0xF7);
        io.write(0x380001, 0x0300, true, false); // rangées 0 et 1 : ET des deux
        assert_eq!(io.read(0x300000) >> 8, 0xFE & 0xFB);
        // DIP : bit « Controller » (0x04) à 0 = panneau mahjong
        assert_eq!(io.read(0x300000) & 0xFF, 0xFB);
    }

    #[test]
    fn mahjong_select_is_masked_when_row2_selected() {
        let mut io = Io {
            mahjong: true,
            ..Io::default()
        };
        io.system.bits = !0x02; // SELECT J1 pressé
        assert_eq!(io.read(0x380000) & 0x0200, 0);
        io.write(0x380001, 0x0400, true, false);
        assert_eq!(io.read(0x380000) & 0x0200, 0x0200);
        // sans panneau, aucun masquage
        let mut j = Io::default();
        j.system.bits = !0x02;
        j.write(0x380001, 0x0400, true, false);
        assert_eq!(j.read(0x380000) & 0x0200, 0);
    }

    #[test]
    fn read_p1cnt_rest() {
        let mut io = make_io();
        // pad[0].bits = 0xFF (all released), dipsw = 0xFF
        let val = io.read(0x300000);
        assert_eq!(val, 0xFFFF);
    }

    #[test]
    fn read_p1cnt_button_pressed() {
        let mut io = make_io();
        // Button A pressed (bit 4 cleared)
        io.pad[0].bits &= !(1 << 4);
        let val = io.read(0x300000);
        assert_eq!(val, 0xEFFF);
    }

    #[test]
    fn read_systype_rest() {
        let mut io = make_io();
        let val = io.read(0x300080);
        // 0xFF00 | 0x40 | 0x80 (test not pressed) = 0xFFC0
        assert_eq!(val, 0xFFC0);
    }

    #[test]
    fn read_systype_test_pressed() {
        let mut io = make_io();
        io.test_pressed = true;
        let val = io.read(0x300080);
        // 0xFF00 | 0x40 | 0x00 (test pressed) = 0xFF40
        assert_eq!(val, 0xFF40);
    }

    #[test]
    fn read_sound_status_rest() {
        let mut io = make_io();
        io.sound_reply = 0x5A;
        // status_a: all coins released (bits 0-4 set), bit 5=0, rtc bits clear
        // = 1|2|4|8|16 = 31 = 0x1F
        let val = io.read(0x320000);
        assert_eq!(val, 0x5A1F);
    }

    #[test]
    fn read_sound_status_coin_pressed() {
        let mut io = make_io();
        io.sound_reply = 0x5A;
        // Coin1 pressed (bit 4 of system.bits cleared)
        io.system.bits &= !(1 << 4);
        // status_a: bit 0 clear, bits 1-4 set = 2|4|8|16 = 30 = 0x1E
        let val = io.read(0x320000);
        assert_eq!(val, 0x5A1E);
    }

    #[test]
    fn read_sound_status_rtc_tp() {
        let mut io = make_io();
        io.sound_reply = 0x5A;
        io.rtc_tp = true;
        // status_a: bits 0-4 set (31) + bit 6 set (64) = 95 = 0x5F
        let val = io.read(0x320000);
        assert_eq!(val, 0x5A5F);
    }

    #[test]
    fn read_p2cnt_rest() {
        let mut io = make_io();
        let val = io.read(0x340000);
        // pad[1].bits = 0xFF, lower byte always 0xFF
        assert_eq!(val, 0xFFFF);
    }

    #[test]
    fn read_status_b_rest() {
        let mut io = make_io();
        // system.bits & 0x0F = 0x0F (all released)
        // 0x00FF | (0x0F << 8) | 0x7000 | 0x8000 = 0xFFFF
        let val = io.read(0x380000);
        assert_eq!(val, 0xFFFF);
    }

    #[test]
    fn read_status_b_start1_pressed() {
        let mut io = make_io();
        // Start1 pressed (bit 0 cleared)
        io.system.bits &= !(1 << 0);
        // system.bits & 0x0F = 0xFE
        // 0x00FF | (0xFE << 8) | 0x7000 | 0x8000 = 0xFEFF
        let val = io.read(0x380000);
        assert_eq!(val, 0xFEFF);
    }

    #[test]
    fn write_watchdog_kick() {
        let mut io = make_io();
        io.write(0x300001, 0x0000, false, true);
        assert!(io.watchdog_kick);
    }

    #[test]
    fn write_sound_command() {
        let mut io = make_io();
        // Even address $320000 uses lower byte of word
        io.write(0x320000, 0x0077, true, true);
        assert_eq!(io.pending_sound_cmd, Some(0x77));
    }

    #[test]
    fn card_detect_bits() {
        let mut io = Io::default();
        assert_eq!(io.read(0x380000) & 0x7000, 0x7000);
        io.card_present = true;
        assert_eq!(io.read(0x380000) & 0x7000, 0);
    }

    #[test]
    fn write_card_bank() {
        let mut io = make_io();
        // REG_CRDBANK at $380011 (odd address)
        io.write(0x380011, 0x0500, false, true);
        assert_eq!(io.card_bank, 5);
    }

    #[test]
    fn write_rtc_control() {
        let mut io = make_io();
        // REG_RTCCTRL at $380051 (odd address)
        io.write(0x380051, 0x0100, false, true);
        assert!(io.rtc_data_out);
    }

    #[test]
    fn unmapped_io_returns_ffff() {
        let mut io = make_io();
        // $360000 is not a defined I/O region
        let val = io.read(0x360000);
        assert_eq!(val, 0xFFFF);
    }
}
