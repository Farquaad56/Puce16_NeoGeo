//! Neo Geo MVS I/O registers ($3x0000 range).
//!
//! All reads/writes are 16-bit words except where noted.
//! Active-low convention for buttons (1 = released, 0 = pressed).

use crate::device::Device;
use ng_common::{PadState, SystemButtons};

/// I/O register state.
pub struct Io {
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
}

impl Default for Io {
    fn default() -> Self {
        Io {
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
                    ((self.pad[0].bits as u16) << 8) | (self.dipsw as u16)
                } else {
                    // REG_SYSTYPE at $300080
                    let test_bit = if self.test_pressed { 0 } else { 0x80 };
                    0xFF00 | 0x40 | test_bit
                }
            }
            0x32 => {
                // REG_SOUND / REG_STATUS_A at $320000
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
            0x34 => {
                // REG_P2CNT at $340000
                ((self.pad[1].bits as u16) << 8) | 0xFF
            }
            0x38 => {
                // REG_STATUS_B at $380000
                let sys_bits = (self.system.bits & 0x0F) as u16;
                0x00FF | (sys_bits << 8) | 0x7000 | 0x8000
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

        // Watchdog kick on any write to $300001 (mirror $FE0081)
        if addr & 0xFE0081 == 0x300001 {
            self.watchdog_kick = true;
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
                0x00 => { /* REG_POUTPUT - pad select, ignored */ }
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
