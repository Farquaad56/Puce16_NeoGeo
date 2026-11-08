//! uPD4990A RTC emulation (port of ref/mame-neogeo/upd1990a.cpp).
//! BSD-3-Clause, copyright Curt Coder, hap.
//! Neo Geo uses the serial variant with 32.768 kHz crystal.

// license:BSD-3-Clause
// source: ref/mame-neogeo/upd1990a.cpp (upd4990a_device)

use std::time::{SystemTime, UNIX_EPOCH};

/// uPD4990A RTC device state.
pub struct Upd4990a {
    // Time counter: [sec, min, hour, day, month|weekday, year] in BCD
    time_counter: [u8; 6],
    // Shift register (7 bytes for serial mode)
    shift_reg: [u8; 7],
    // Control/state bits
    oe: bool,
    #[allow(dead_code)]
    cs: bool,
    stb: bool,
    data_in: bool,
    data_out: bool,
    c: u8,     // latched command
    clk: bool, // shift clock state
    tp: bool,  // time pulse output
    #[allow(dead_code)]
    c_unlatched: u8, // command waiting for STB (parallel mode only)
    testmode: bool,
    counter_held: bool,

    // Tick-based timing (replaces MAME timers)
    mclk_elapsed: u64,        // total master clock ticks since last advance
    tp_tick_count: u64,       // ticks until next TP toggle
    data_out_tick_count: u64, // ticks until next data out toggle
    test_tick_count: u64,     // ticks until next test tick
}

// Command modes (from upd1990a.h)
const MODE_REGISTER_HOLD: u8 = 0;
const MODE_SHIFT: u8 = 1;
const MODE_TIME_SET: u8 = 2;
const MODE_TIME_READ: u8 = 3;
const MODE_TP_64HZ: u8 = 4;
const MODE_TP_256HZ: u8 = 5;
const MODE_TP_2048HZ: u8 = 6;
const MODE_TP_4096HZ: u8 = 7;
const MODE_TP_1S_INT: u8 = 8;
const MODE_TP_10S_INT: u8 = 9;
const MODE_TP_30S_INT: u8 = 10;
const MODE_TP_60S_INT: u8 = 11;
#[allow(dead_code)]
const MODE_INT_RESET_OUTPUT: u8 = 12;
#[allow(dead_code)]
const MODE_INT_RUN_CLOCK: u8 = 13;
#[allow(dead_code)]
const MODE_INT_STOP_CLOCK: u8 = 14;
const MODE_TEST: u8 = 15;

// Neo Geo master clock: 24 MHz
const MCLK_HZ: u64 = 24_000_000;
// RTC crystal: 32768 Hz
const XTAL_HZ: u64 = 32768;

impl Default for Upd4990a {
    fn default() -> Self {
        Self::new()
    }
}

impl Upd4990a {
    pub fn new() -> Self {
        let mut rtc = Upd4990a {
            time_counter: [0; 6],
            shift_reg: [0; 7],
            oe: false,
            cs: true, // Neo Geo always has CS active
            stb: false,
            data_in: false,
            data_out: false,
            c: 0,
            clk: false,
            tp: false,
            c_unlatched: 0,
            testmode: false,
            counter_held: false,
            mclk_elapsed: 0,
            tp_tick_count: 0,
            data_out_tick_count: 0,
            test_tick_count: 0,
        };
        // Initialize to current system time
        rtc.set_time_from_system();
        // Default TP mode: 64 Hz (clock/512 * 2)
        rtc.tp_tick_count = MCLK_HZ / 512;
        rtc
    }

    /// Set inputs from Neo Geo REG_RTCCTRL ($380051).
    /// Bits: 0=DIN, 1=CLK, 2=STB
    pub fn set_inputs(&mut self, stb: bool, clk: bool, din: bool) {
        // Rising edge of STB latches command and executes it
        if !self.stb && stb {
            self.execute_command();
        }

        // Rising edge of CLK shifts data (in serial mode always clock command register)
        if !self.clk && clk {
            let in_bit = din;
            // Serial mode: shift command register bits 0-2 into position 6
            let cmd_bit = self.shift_reg[6] & 1;
            self.shift_reg[6] >>= 1;
            self.shift_reg[6] |= if in_bit { 8 } else { 0 };

            if self.c == MODE_SHIFT {
                // Clock shift register
                let max_shift = 6; // serial mode
                for i in 0..max_shift {
                    self.shift_reg[i] >>= 1;
                    if i == (max_shift - 1) {
                        self.shift_reg[i] |= if cmd_bit != 0 { 0x80 } else { 0 };
                    } else {
                        let next = self.shift_reg[i + 1];
                        self.shift_reg[i] |= (next >> 7) << 7;
                    }
                }
                // Data out LSB of shift register
                self.data_out = (self.shift_reg[0] & 1) != 0;
            }
        }

        self.stb = stb;
        self.clk = clk;
        self.data_in = din;
    }

    /// Get data out signal (bit 7 of $320001).
    pub fn data_out(&self) -> bool {
        // In test mode, always drive output
        if self.testmode {
            return self.data_out;
        }
        // Otherwise high-impedance when OE is low (reads as 1 on Neo Geo bus)
        if !self.oe {
            return true;
        }
        self.data_out
    }

    /// Get time pulse signal (bit 6 of $320001).
    pub fn tp(&self) -> bool {
        self.tp
    }

    /// Get raw time counter values for testing.
    pub fn time_counter(&self) -> [u8; 6] {
        self.time_counter
    }

    /// Advance RTC by elapsed master clock ticks.
    pub fn advance(&mut self, mclk: u64) {
        self.mclk_elapsed += mclk;

        // TP tick (time pulse output)
        if self.tp_tick_count > 0 && self.mclk_elapsed >= self.tp_tick_count {
            self.tp = !self.tp;
            self.mclk_elapsed -= self.tp_tick_count;
            // Reset for next toggle (half period)
            self.tp_tick_count = MCLK_HZ / 512; // Default 64 Hz mode
        }

        // Data out tick (register hold mode: 1 Hz pulse)
        if self.c == MODE_REGISTER_HOLD
            && !self.counter_held
            && self.data_out_tick_count > 0
            && self.mclk_elapsed >= self.data_out_tick_count
        {
            self.data_out = !self.data_out;
            self.mclk_elapsed -= self.data_out_tick_count;
            // Next toggle: half period of 1 Hz = 0.5 s in mclk ticks
            self.data_out_tick_count = MCLK_HZ / (XTAL_HZ * 2);
        }

        // Test mode tick
        if self.testmode && self.test_tick_count > 0 && self.mclk_elapsed >= self.test_tick_count {
            self.advance_test_mode();
            self.mclk_elapsed -= self.test_tick_count;
            // uPD4990A test mode: 8192 Hz = clock/4
            self.test_tick_count = MCLK_HZ / 4;
        }

        // Clock tick (second counter) - only if not held and not in test mode
        if !self.counter_held && !self.testmode {
            // One second = XTAL_HZ ticks of crystal = MCLK_HZ/XTAL_HZ mclk ticks
            let one_second_mclk = MCLK_HZ / XTAL_HZ;
            while self.mclk_elapsed >= one_second_mclk {
                self.advance_seconds();
                self.mclk_elapsed -= one_second_mclk;
            }
        }
    }

    /// Set time from system clock.
    pub fn set_time_from_system(&mut self) {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                let secs = duration.as_secs() as i64;
                // Convert to local time (simplified: assume UTC for now)
                let s = secs % 60;
                let m = (secs / 60) % 60;
                let h = (secs / 3600) % 24;
                // Days since epoch -> date calculation omitted for simplicity
                // Use fixed values for day/month/year in test mode
                self.set_time(2024, 1, 1, 1, h as u8, m as u8, s as u8);
            }
            Err(_) => {
                self.set_time(2024, 1, 1, 1, 0, 0, 0);
            }
        }
    }

    /// Set time explicitly (for testing).
    #[allow(clippy::too_many_arguments)]
    pub fn set_time(
        &mut self,
        year: u16,
        month: u8,
        day: u8,
        weekday: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) {
        self.time_counter[0] = to_bcd(second);
        self.time_counter[1] = to_bcd(minute);
        self.time_counter[2] = to_bcd(hour);
        self.time_counter[3] = to_bcd(day);
        self.time_counter[4] = (month << 4) | ((weekday - 1) & 0x0F);
        self.time_counter[5] = to_bcd((year % 100) as u8);
    }

    fn execute_command(&mut self) {
        // In serial mode, command is in shift_reg[6] bits 0-2
        let cmd = self.shift_reg[6] & 7;
        self.c = cmd;

        let was_testmode = self.testmode;

        match cmd {
            MODE_REGISTER_HOLD => {
                // Release counter if held
                self.counter_held = false;
                // Disable test mode
                self.testmode = false;
                // 1 Hz data out pulse (half period)
                self.data_out_tick_count = MCLK_HZ / (XTAL_HZ * 2);
                // TP: if coming from test mode or TP not set, default to 64 Hz
                if was_testmode || self.tp_tick_count == 0 {
                    self.tp_tick_count = MCLK_HZ / 512;
                }
            }
            MODE_SHIFT => {
                // Release counter if held
                self.counter_held = false;
                // Data out LSB of shift register
                self.data_out = (self.shift_reg[0] & 1) != 0;
                // In test mode, TP is 32 Hz
                if self.testmode {
                    self.tp_tick_count = MCLK_HZ / 1024;
                }
            }
            MODE_TIME_SET => {
                // Hold time counter
                self.counter_held = true;
                // Data out LSB of shift register
                self.data_out = (self.shift_reg[0] & 1) != 0;
                // Load shift register into time counter
                let max_shift = 6;
                for i in 0..max_shift {
                    self.time_counter[i] = self.shift_reg[i];
                }
                // In test mode, disable TP
                if self.testmode {
                    self.tp_tick_count = 0;
                    self.tp = false;
                }
            }
            MODE_TIME_READ => {
                // Release counter if held
                if self.counter_held {
                    self.counter_held = false;
                }
                // Load time counter into shift register
                let max_shift = 6;
                for i in 0..max_shift {
                    self.shift_reg[i] = self.time_counter[i];
                }
                // uPD4990A: 1 Hz data out pulse
                self.data_out_tick_count = MCLK_HZ / (XTAL_HZ * 2);
                // In test mode, TP is 32 Hz
                if self.testmode {
                    self.tp_tick_count = MCLK_HZ / 1024;
                }
            }
            MODE_TP_64HZ => {
                self.tp_tick_count = MCLK_HZ / 512;
            }
            MODE_TP_256HZ => {
                self.tp_tick_count = MCLK_HZ / 128;
            }
            MODE_TP_2048HZ => {
                self.tp_tick_count = MCLK_HZ / 16;
            }
            MODE_TP_4096HZ => {
                self.tp_tick_count = MCLK_HZ / 8;
            }
            // Timer interval modes (simplified)
            MODE_TP_1S_INT | MODE_TP_10S_INT | MODE_TP_30S_INT | MODE_TP_60S_INT => {
                // Not used by Neo Geo BIOS, stub
            }
            MODE_TEST => {
                self.testmode = true;
                // uPD4990A test mode: 8192 Hz
                self.test_tick_count = MCLK_HZ / 4;
            }
            _ => {}
        }
    }

    fn advance_seconds(&mut self) {
        // Increment seconds, carry through minutes/hours/days/months/years
        let mut s = from_bcd(self.time_counter[0]);
        s += 1;
        if s >= 60 {
            s = 0;
            let mut m = from_bcd(self.time_counter[1]);
            m += 1;
            if m >= 60 {
                m = 0;
                let mut h = from_bcd(self.time_counter[2]);
                h += 1;
                if h >= 24 {
                    h = 0;
                    // Day increment (simplified, no month/year rollover)
                    let mut d = from_bcd(self.time_counter[3]);
                    d += 1;
                    self.time_counter[3] = to_bcd(d);
                }
                self.time_counter[2] = to_bcd(h);
            }
            self.time_counter[1] = to_bcd(m);
        }
        self.time_counter[0] = to_bcd(s);
    }

    fn advance_test_mode(&mut self) {
        if self.counter_held {
            return;
        }
        // Simplified test mode: just increment seconds counter
        let mut s = from_bcd(self.time_counter[0]);
        s += 1;
        if s >= 60 {
            s = 0;
        }
        self.time_counter[0] = to_bcd(s);
    }
}

fn to_bcd(val: u8) -> u8 {
    ((val / 10) << 4) | (val % 10)
}

fn from_bcd(bcd: u8) -> u8 {
    (bcd >> 4) * 10 + (bcd & 0x0F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_time_bcd_encoding() {
        let mut rtc = Upd4990a::new();
        // Set to Feb 29, 2024 (leap year), Thursday (day 5), 12:34:56
        rtc.set_time(2024, 2, 29, 5, 12, 34, 56);
        // Expected BCD: sec=0x56, min=0x34, hour=0x12, day=0x29, month|weekday=0x24, year=0x24
        assert_eq!(rtc.time_counter()[0], 0x56); // seconds
        assert_eq!(rtc.time_counter()[1], 0x34); // minutes
        assert_eq!(rtc.time_counter()[2], 0x12); // hours
        assert_eq!(rtc.time_counter()[3], 0x29); // day
        assert_eq!(rtc.time_counter()[4], 0x24); // month=2, weekday=5 -> (2<<4)|(5-1)=0x24
        assert_eq!(rtc.time_counter()[5], 0x24); // year % 100 = 24
    }

    #[test]
    fn register_hold_data_out_toggle() {
        let mut rtc = Upd4990a::new();
        // Set to register hold mode (command 0)
        rtc.shift_reg[6] = 0;
        rtc.execute_command();
        assert_eq!(rtc.c, MODE_REGISTER_HOLD);

        // Advance by half period of 1 Hz data out (should toggle once)
        let half_period = MCLK_HZ / (XTAL_HZ * 2);
        let initial_out = rtc.data_out;
        rtc.advance(half_period + 1);
        assert_ne!(rtc.data_out, initial_out);

        // Advance another half period (should toggle back)
        let _second_out = rtc.data_out;
        rtc.advance(half_period + 1);
        assert_eq!(rtc.data_out, initial_out);
    }

    #[test]
    fn tp_64hz_default() {
        let mut rtc = Upd4990a::new();
        // Default TP mode is 64 Hz (half period = MCLK_HZ/512)
        let half_period = MCLK_HZ / 512;
        let initial_tp = rtc.tp();
        rtc.advance(half_period + 1);
        assert_ne!(rtc.tp(), initial_tp);
    }

    #[test]
    fn tp_mode_change() {
        let mut rtc = Upd4990a::new();
        // Switch to 256 Hz TP mode (command 5)
        rtc.shift_reg[6] = MODE_TP_256HZ;
        rtc.execute_command();

        // Half period of 256 Hz = MCLK_HZ/128
        let half_period = MCLK_HZ / 128;
        let initial_tp = rtc.tp();
        rtc.advance(half_period + 1);
        assert_ne!(rtc.tp(), initial_tp);
    }

    #[test]
    fn time_advances() {
        let mut rtc = Upd4990a::new();
        rtc.set_time(2024, 1, 1, 1, 0, 0, 58); // 58 seconds

        // Advance by 3 seconds (should wrap to 01)
        let one_second = MCLK_HZ / XTAL_HZ;
        rtc.advance(one_second * 3 + 1);

        assert_eq!(rtc.time_counter()[0], 0x01); // seconds wrapped: 58+3=61 -> 01
    }
}
