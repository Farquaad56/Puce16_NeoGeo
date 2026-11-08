// license:BSD-3-Clause
// copyright-holders:Curt Coder, hap
//! uPD4990A RTC (mode série, câblage Neo Geo : CS=OE=1, C0-C2=1).
//! Port de ref/mame-neogeo/upd1990a.cpp (upd4990a_device) ; l'horloge RTC est le quartz 32,768 kHz.
//!
//! Temps interne en « unités » : 1 mclk (24 MHz) = 64 unités, 1 tick du quartz = 46 875 unités,
//! 1 seconde = 1 536 000 000 unités. Cela rend EXACTES toutes les périodes de MAME (2048 Hz et
//! 4096 Hz donnent des demi-périodes fractionnaires en mclk).
//!
//! PORT-NOTE : les timers MAME (`emu_timer`) sont des compteurs à rebours ; une minuterie réarmée
//! avec un délai nul s'exécute aussitôt (à la fin de la commande). Le mode parallèle (1990A) et
//! la branche `oe == 0` du test_tick ne sont pas portés (câblage Neo Geo fixe).

const UNITS_PER_MCLK: u64 = 64;
const UNITS_PER_XTAL: u64 = 46_875;
const UNITS_PER_SEC: u64 = 1_536_000_000;
/// Période de 0,5 s (Register Hold et Time Read : impulsion DATA OUT à 1 Hz).
const HALF_SEC: u64 = UNITS_PER_SEC / 2;
/// Demi-périodes de TP pour les modes 64 Hz, 256 Hz, 2048 Hz, 4096 Hz.
const TP_PERIOD: [u64; 4] = [12_000_000, 3_000_000, 375_000, 187_500];
/// Mode test : tick à 8192 Hz ; TP à 32 Hz (timer 64 Hz).
const TEST_PERIOD: u64 = 187_500;
const TEST_TP_PERIOD: u64 = 24_000_000;
/// Nombre de ticks du quartz dans 1/64 s (diviseur du uPD4990A).
const DIVIDER_TICKS: u64 = 512;

const MODE_REGISTER_HOLD: u8 = 0;
const MODE_SHIFT: u8 = 1;
const MODE_TIME_SET: u8 = 2;
const MODE_TIME_READ: u8 = 3;
const MODE_TP_64HZ: u8 = 4;
const MODE_TP_1S_INT: u8 = 8;
const MODE_TP_60S_INT: u8 = 11;
const MODE_TEST: u8 = 15;

const T_CLOCK: usize = 0;
const T_TP: usize = 1;
const T_DATA: usize = 2;
const T_TEST: usize = 3;

#[derive(Clone, Copy, Debug, Default)]
struct Timer {
    enabled: bool,
    left: u64,
    period: u64,
}

impl Timer {
    /// `adjust(start, period)` de MAME : arme (et active) la minuterie.
    fn adjust(&mut self, start: u64, period: u64) {
        self.enabled = true;
        self.left = start;
        self.period = period;
    }
}

/// Calendrier (rôle de device_rtc_interface : avance d'une seconde, mois et années bissextiles).
#[derive(Clone, Copy, Debug)]
struct Calendar {
    year: u8, // 0..99
    month: u8,
    day: u8,
    dow: u8, // 1..7
    hour: u8,
    minute: u8,
    second: u8,
}

impl Calendar {
    fn days_in_month(&self) -> u8 {
        match self.month {
            2 => {
                if self.year.is_multiple_of(4) {
                    29
                } else {
                    28
                }
            }
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    fn advance_seconds(&mut self) {
        self.second += 1;
        if self.second < 60 {
            return;
        }
        self.second = 0;
        self.minute += 1;
        if self.minute < 60 {
            return;
        }
        self.minute = 0;
        self.hour += 1;
        if self.hour < 24 {
            return;
        }
        self.hour = 0;
        self.dow = if self.dow >= 7 { 1 } else { self.dow + 1 };
        self.day += 1;
        if self.day <= self.days_in_month() {
            return;
        }
        self.day = 1;
        self.month += 1;
        if self.month > 12 {
            self.month = 1;
            self.year = (self.year + 1) % 100;
        }
    }
}

/// uPD4990A.
#[derive(Clone, Debug)]
pub struct Upd4990a {
    time_counter: [u8; 6],
    shift_reg: [u8; 7],
    cal: Calendar,
    stb: bool,
    data_in: bool,
    data_out: bool,
    c: u8,
    clk: bool,
    tp: bool,
    testmode: bool,
    counter_held: bool,
    timers: [Timer; 4],
    now: u64,
}

impl Default for Upd4990a {
    fn default() -> Self {
        Self::new()
    }
}

fn to_bcd(v: u8) -> u8 {
    ((v / 10) << 4) | (v % 10)
}

fn from_bcd(b: u8) -> u8 {
    (b >> 4) * 10 + (b & 0x0F)
}

impl Upd4990a {
    pub fn new() -> Self {
        let mut timers = [Timer::default(); 4];
        // device_start : timer d'horloge, 1 s sur le quartz 32,768 kHz
        timers[T_CLOCK].adjust(UNITS_PER_SEC, UNITS_PER_SEC);
        let mut rtc = Upd4990a {
            time_counter: [0; 6],
            shift_reg: [0; 7],
            cal: Calendar {
                year: 0,
                month: 1,
                day: 1,
                dow: 1,
                hour: 0,
                minute: 0,
                second: 0,
            },
            stb: false,
            data_in: false,
            data_out: false,
            c: 0,
            clk: false,
            tp: false,
            testmode: false,
            counter_held: false,
            timers,
            now: 0,
        };
        rtc.clock_updated();
        rtc
    }

    /// Registre REG_RTCCTRL ($380051) : bit 0 DIN, bit 1 CLK, bit 2 STB. Ordre MAME : DIN, CLK, STB.
    pub fn set_inputs(&mut self, stb: bool, clk: bool, din: bool) {
        self.data_in = din;
        self.clk_w(clk);
        self.stb_w(stb);
    }

    /// Bit 7 de $320001 (DATA OUT ; haute impédance = 1 si OE bas, jamais ici).
    pub fn data_out(&self) -> bool {
        self.data_out
    }

    /// Bit 6 de $320001 (TP).
    pub fn tp(&self) -> bool {
        self.tp
    }

    /// Compteur de temps : [secondes, minutes, heures, jour] en BCD, mois<<4 | (jour_semaine-1), année BCD.
    pub fn time_counter(&self) -> [u8; 6] {
        self.time_counter
    }

    /// Fixe l'heure (fournie par le Machine). `weekday` : 1 = dimanche .. 7 = samedi.
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
        self.cal = Calendar {
            year: (year % 100) as u8,
            month,
            day,
            dow: weekday,
            hour,
            minute,
            second,
        };
        self.clock_updated();
    }

    /// Fait avancer le temps de `mclk` périodes de l'horloge maître (24 MHz).
    pub fn advance(&mut self, mclk: u64) {
        let mut rem = mclk * UNITS_PER_MCLK;
        self.run_due();
        loop {
            let step = self
                .timers
                .iter()
                .filter(|t| t.enabled)
                .map(|t| t.left)
                .min();
            match step {
                Some(s) if s <= rem => {
                    self.elapse(s);
                    rem -= s;
                    self.run_due();
                }
                _ => {
                    self.elapse(rem);
                    break;
                }
            }
        }
    }

    fn elapse(&mut self, units: u64) {
        self.now += units;
        for t in self.timers.iter_mut().filter(|t| t.enabled) {
            t.left -= units;
        }
    }

    fn run_due(&mut self) {
        loop {
            let mut fired = false;
            for i in 0..self.timers.len() {
                if self.timers[i].enabled && self.timers[i].left == 0 {
                    self.timers[i].left = self.timers[i].period;
                    self.fire(i);
                    fired = true;
                }
            }
            if !fired {
                break;
            }
        }
    }

    fn fire(&mut self, i: usize) {
        match i {
            T_CLOCK => {
                self.cal.advance_seconds();
                self.clock_updated();
            }
            T_TP => self.tp = !self.tp,
            T_DATA => self.data_out = !self.data_out,
            _ => self.test_tick(),
        }
    }

    /// rtc_clock_updated
    fn clock_updated(&mut self) {
        let c = &self.cal;
        self.time_counter = [
            to_bcd(c.second),
            to_bcd(c.minute),
            to_bcd(c.hour),
            to_bcd(c.day),
            (c.month << 4) | (c.dow.wrapping_sub(1) & 0x0F),
            to_bcd(c.year),
        ];
    }

    /// test_tick (branche OE = 1)
    fn test_tick(&mut self) {
        if self.counter_held {
            return;
        }
        self.data_out = self.time_counter[5] == 0;
        for b in self.time_counter.iter_mut() {
            *b = b.wrapping_add(1);
            if *b != 0 {
                return;
            }
        }
    }

    /// release_counter : fin d'un Time Set (la 1re seconde suivante arrive 63/64 s après le prochain pas du diviseur).
    fn release_counter(&mut self) {
        if !self.counter_held {
            return;
        }
        self.counter_held = false;
        if self.testmode {
            return;
        }
        let now_ticks = self.now / UNITS_PER_XTAL;
        let first = DIVIDER_TICKS - (now_ticks % DIVIDER_TICKS) + 63 * DIVIDER_TICKS;
        self.timers[T_CLOCK].adjust(first * UNITS_PER_XTAL, UNITS_PER_SEC);
    }

    fn clk_w(&mut self, state: bool) {
        if !self.clk && state {
            // mode série : le registre de commande est toujours cadencé
            let in_bit = self.shift_reg[6] & 1;
            self.shift_reg[6] >>= 1;
            self.shift_reg[6] |= (self.data_in as u8) << 3;

            if self.c == MODE_SHIFT {
                for i in 0..6 {
                    self.shift_reg[i] >>= 1;
                    if i == 5 {
                        self.shift_reg[i] |= in_bit << 7;
                    } else {
                        self.shift_reg[i] |= (self.shift_reg[i + 1] << 7) & 0x80;
                    }
                }
                self.data_out = self.shift_reg[0] & 1 != 0;
            }
        }
        self.clk = state;
    }

    fn stb_w(&mut self, state: bool) {
        if !self.stb && state {
            self.c = self.shift_reg[6];
            let c = self.c;
            let was_testmode = self.testmode;

            if c == MODE_REGISTER_HOLD || (MODE_TP_64HZ..MODE_TEST).contains(&c) {
                if !self.counter_held {
                    self.timers[T_CLOCK].enabled = true;
                }
                self.testmode = false;
                self.timers[T_TEST].enabled = false;
            }

            match c {
                MODE_REGISTER_HOLD => {
                    self.release_counter();
                    self.timers[T_DATA].adjust(0, HALF_SEC);
                    if was_testmode || !self.timers[T_TP].enabled {
                        self.timers[T_TP].adjust(0, TP_PERIOD[0]);
                    }
                }
                MODE_SHIFT => {
                    if self.counter_held {
                        self.release_counter();
                    } else if !self.testmode {
                        self.timers[T_CLOCK].enabled = true;
                    }
                    self.timers[T_DATA].enabled = false;
                    self.data_out = self.shift_reg[0] & 1 != 0;
                    if self.testmode {
                        self.timers[T_TP].adjust(0, TEST_TP_PERIOD);
                    }
                }
                MODE_TIME_SET => {
                    self.timers[T_CLOCK].enabled = false;
                    self.counter_held = true;
                    self.timers[T_DATA].enabled = false;
                    self.data_out = self.shift_reg[0] & 1 != 0;
                    self.time_counter.copy_from_slice(&self.shift_reg[..6]);
                    let tc = self.time_counter;
                    self.cal = Calendar {
                        year: from_bcd(tc[5]),
                        month: tc[4] >> 4,
                        day: from_bcd(tc[3]),
                        dow: (tc[4] & 0x0F) + 1,
                        hour: from_bcd(tc[2]),
                        minute: from_bcd(tc[1]),
                        second: from_bcd(tc[0]),
                    };
                    if self.testmode {
                        self.timers[T_TP].enabled = false;
                        self.tp = false;
                    }
                }
                MODE_TIME_READ => {
                    if self.counter_held {
                        self.release_counter();
                    } else if !self.testmode {
                        self.timers[T_CLOCK].enabled = true;
                    }
                    let tc = self.time_counter;
                    self.shift_reg[..6].copy_from_slice(&tc);
                    self.timers[T_DATA].adjust(0, HALF_SEC);
                    if self.testmode {
                        self.timers[T_TP].adjust(0, TEST_TP_PERIOD);
                    }
                }
                MODE_TP_64HZ..=7 => {
                    self.timers[T_TP].adjust(0, TP_PERIOD[(c - MODE_TP_64HZ) as usize]);
                }
                MODE_TP_1S_INT..=MODE_TP_60S_INT => {
                    const MUL: [u64; 4] = [1, 10, 30, 60];
                    self.timers[T_TP].adjust(0, HALF_SEC * MUL[(c - MODE_TP_1S_INT) as usize]);
                }
                MODE_TEST => {
                    self.timers[T_CLOCK].enabled = false;
                    self.timers[T_DATA].enabled = false;
                    self.testmode = true;
                    self.timers[T_TEST].adjust(0, TEST_PERIOD);
                }
                _ => {} // 12..14 : TODO dans MAME
            }
            // un délai nul s'exécute à la fin de la commande
            self.run_due();
        }
        self.stb = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock_bit(r: &mut Upd4990a, din: bool) {
        r.set_inputs(false, false, din);
        r.set_inputs(false, true, din);
        r.set_inputs(false, false, din);
    }

    fn strobe(r: &mut Upd4990a) {
        r.set_inputs(true, false, false);
        r.set_inputs(false, false, false);
    }

    /// Envoie une commande de 4 bits (LSB d'abord) puis un front montant de STB.
    fn command(r: &mut Upd4990a, c: u8) {
        for k in 0..4 {
            clock_bit(r, (c >> k) & 1 != 0);
        }
        strobe(r);
    }

    #[test]
    fn set_time_bcd_encoding() {
        let mut rtc = Upd4990a::new();
        rtc.set_time(2024, 2, 29, 4, 12, 34, 56);
        assert_eq!(&rtc.time_counter()[0..5], &[0x56, 0x34, 0x12, 0x29, 0x23]);
        assert_eq!(rtc.time_counter()[5], 0x24);
    }

    #[test]
    fn register_hold_data_out_toggle() {
        let mut rtc = Upd4990a::new();
        let initial = rtc.data_out();
        command(&mut rtc, MODE_REGISTER_HOLD);
        assert_ne!(rtc.data_out(), initial, "bascule immédiate");
        let after = rtc.data_out();
        rtc.advance(11_999_999);
        assert_eq!(rtc.data_out(), after);
        rtc.advance(1);
        assert_ne!(rtc.data_out(), after, "bascule à 12 000 000 mclk");
        rtc.advance(12_000_000);
        assert_eq!(rtc.data_out(), after, "rebascule");
    }

    #[test]
    fn tp_64hz_default() {
        let mut rtc = Upd4990a::new();
        command(&mut rtc, MODE_REGISTER_HOLD); // TP jamais réglé => 64 Hz
        assert!(rtc.tp(), "bascule immédiate");
        rtc.advance(187_499);
        assert!(rtc.tp());
        rtc.advance(1);
        assert!(!rtc.tp(), "demi-période = 24e6/128 mclk");
        rtc.advance(187_500);
        assert!(rtc.tp());
    }

    #[test]
    fn tp_mode_256hz_and_2048hz() {
        let mut rtc = Upd4990a::new();
        command(&mut rtc, 5); // 256 Hz : bascule toutes les 24e6/512 = 46 875 mclk
        assert!(rtc.tp());
        rtc.advance(46_874);
        assert!(rtc.tp());
        rtc.advance(1);
        assert!(!rtc.tp());
        command(&mut rtc, 6); // 2048 Hz : 24e6/4096 = 5859,375 mclk (exact en unités)
        let t0 = rtc.tp();
        rtc.advance(5859);
        assert_eq!(rtc.tp(), t0);
        rtc.advance(1);
        assert_ne!(rtc.tp(), t0);
    }

    #[test]
    fn seconds_and_leap_day_rollover() {
        let mut rtc = Upd4990a::new();
        rtc.set_time(2024, 2, 28, 3, 23, 59, 59);
        rtc.advance(23_999_999);
        assert_eq!(rtc.time_counter()[0], 0x59);
        rtc.advance(1);
        // 29 février 2024 (bissextile), mercredi->jeudi : dow 4 => 4-1 = 3
        assert_eq!(rtc.time_counter(), [0x00, 0x00, 0x00, 0x29, 0x23, 0x24]);
        rtc.advance(86_400 * 24_000_000);
        assert_eq!(rtc.time_counter()[3], 0x01);
        assert_eq!(rtc.time_counter()[4] >> 4, 3);
    }

    #[test]
    fn time_read_shifts_out_the_counter() {
        let mut rtc = Upd4990a::new();
        rtc.set_time(2024, 2, 29, 4, 12, 34, 56);
        command(&mut rtc, MODE_TIME_READ);
        command(&mut rtc, MODE_SHIFT);
        let mut out = [0u8; 6];
        for byte in out.iter_mut() {
            for bit in 0..8 {
                *byte |= (rtc.data_out() as u8) << bit;
                clock_bit(&mut rtc, false);
            }
        }
        assert_eq!(out, [0x56, 0x34, 0x12, 0x29, 0x23, 0x24]);
    }

    #[test]
    fn time_set_via_serial_then_release() {
        let mut rtc = Upd4990a::new();
        command(&mut rtc, MODE_SHIFT);
        // 48 bits de données puis 4 bits de commande (TIME_SET = 2) : 52 impulsions
        let data = [0x30u8, 0x45, 0x17, 0x09, 0x73, 0x26];
        for byte in data {
            for bit in 0..8 {
                clock_bit(&mut rtc, (byte >> bit) & 1 != 0);
            }
        }
        for k in 0..4 {
            clock_bit(&mut rtc, (MODE_TIME_SET >> k) & 1 != 0);
        }
        strobe(&mut rtc);
        assert_eq!(rtc.time_counter(), data);
        rtc.advance(3 * 24_000_000);
        assert_eq!(rtc.time_counter(), data, "compteur retenu par Time Set");
        command(&mut rtc, MODE_REGISTER_HOLD); // libère
        rtc.advance(24_000_000);
        assert_eq!(
            rtc.time_counter()[0],
            0x31,
            "une seconde exactement après la libération"
        );
        assert_eq!(rtc.time_counter()[1], 0x45);
    }
}
