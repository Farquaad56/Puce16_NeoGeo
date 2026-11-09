// BSD 3-Clause License
// Copyright (c) 2021, Aaron Giles. All rights reserved.
// Ported from ref/ymfm/src/ymfm_ssg.h and ref/ymfm/src/ymfm_ssg.cpp

use crate::common::{bitfield, AccessClass, Output, YmfmInterface};

/// SSG register map (3 tone channels + noise + envelope).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SsgRegisters {
    regdata: [u8; 0x10],
}

impl Default for SsgRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl SsgRegisters {
    pub const OUTPUTS: u32 = 3;
    pub const CHANNELS: u32 = 3;
    pub const REGISTERS: u32 = 0x10;
    pub const ALL_CHANNELS: u32 = (1 << Self::CHANNELS) - 1;

    pub fn new() -> Self {
        Self { regdata: [0; 0x10] }
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        for i in 0..self.regdata.len() {
            self.regdata[i] = 0;
        }
    }

    /// Direct read access to a register.
    pub fn read(&self, index: u32) -> u8 {
        self.regdata[index as usize]
    }

    /// Direct write access to a register.
    pub fn write(&mut self, index: u32, data: u8) {
        self.regdata[index as usize] = data;
    }

    // System-wide registers
    pub fn noise_period(&self) -> u32 {
        bitfield(self.regdata[0x06] as u32, 0, 5)
    }

    pub fn io_b_out(&self) -> bool {
        bitfield(self.regdata[0x07] as u32, 7, 1) != 0
    }

    pub fn io_a_out(&self) -> bool {
        bitfield(self.regdata[0x07] as u32, 6, 1) != 0
    }

    pub fn envelope_period(&self) -> u32 {
        (self.regdata[0x0b] as u32) | ((self.regdata[0x0c] as u32) << 8)
    }

    pub fn envelope_continue(&self) -> bool {
        bitfield(self.regdata[0x0d] as u32, 3, 1) != 0
    }

    pub fn envelope_attack(&self) -> bool {
        bitfield(self.regdata[0x0d] as u32, 2, 1) != 0
    }

    pub fn envelope_alternate(&self) -> bool {
        bitfield(self.regdata[0x0d] as u32, 1, 1) != 0
    }

    pub fn envelope_hold(&self) -> bool {
        bitfield(self.regdata[0x0d] as u32, 0, 1) != 0
    }

    pub fn io_a_data(&self) -> u8 {
        self.regdata[0x0e]
    }

    pub fn io_b_data(&self) -> u8 {
        self.regdata[0x0f]
    }

    // Per-channel registers
    pub fn ch_noise_enable_n(&self, choffs: u32) -> bool {
        bitfield(self.regdata[0x07] as u32, 3 + choffs as i32, 1) != 0
    }

    pub fn ch_tone_enable_n(&self, choffs: u32) -> bool {
        bitfield(self.regdata[0x07] as u32, choffs as i32, 1) != 0
    }

    pub fn ch_tone_period(&self, choffs: u32) -> u32 {
        let low = self.regdata[(2 * choffs) as usize] as u32;
        let high = bitfield(self.regdata[(0x01 + 2 * choffs) as usize] as u32, 0, 4);
        low | (high << 8)
    }

    pub fn ch_envelope_enable(&self, choffs: u32) -> bool {
        bitfield(self.regdata[(0x08 + choffs) as usize] as u32, 4, 1) != 0
    }

    pub fn ch_amplitude(&self, choffs: u32) -> u32 {
        bitfield(self.regdata[(0x08 + choffs) as usize] as u32, 0, 4)
    }
}

/// SSG engine (square wave generator with noise and envelope).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SsgEngine {
    #[cfg_attr(feature = "serde", serde(skip, default = "crate::common::null_intf"))]
    intf: Box<dyn YmfmInterface>,
    tone_count: [u32; 3],
    tone_state: [u32; 3],
    envelope_count: u32,
    envelope_state: u32,
    noise_count: u32,
    noise_state: u32,
    regs: SsgRegisters,
}

impl SsgEngine {
    pub const OUTPUTS: i32 = SsgRegisters::OUTPUTS as i32;
    pub const CHANNELS: i32 = SsgRegisters::CHANNELS as i32;
    pub const CLOCK_DIVIDER: i32 = 8;

    pub fn new(intf: Box<dyn YmfmInterface>) -> Self {
        Self {
            intf,
            tone_count: [0, 0, 0],
            tone_state: [0, 0, 0],
            envelope_count: 0,
            envelope_state: 0,
            noise_count: 0,
            noise_state: 1,
            regs: SsgRegisters::new(),
        }
    }

    /// Reset the engine state.
    pub fn reset(&mut self) {
        // Reset register state
        self.regs.reset();

        // Reset engine state
        for chan in 0..3 {
            self.tone_count[chan] = 0;
            self.tone_state[chan] = 0;
        }
        self.envelope_count = 0;
        self.envelope_state = 0;
        self.noise_count = 0;
        self.noise_state = 1;
    }

    /// Master clocking function.
    pub fn clock(&mut self) {
        // Clock tones; tone period units are clock/16 but since we run at clock/8
        // that works out for us to toggle the state (50% duty cycle) at twice the
        // programmed period
        for chan in 0..3 {
            self.tone_count[chan] = self.tone_count[chan].wrapping_add(1);
            if self.tone_count[chan] >= self.regs.ch_tone_period(chan as u32) {
                self.tone_state[chan] ^= 1;
                self.tone_count[chan] = 0;
            }
        }

        // Clock noise; noise period units are clock/16 but since we run at clock/8,
        // our counter needs a right shift prior to compare; note that a period of 0
        // should produce an identical result to a period of 1, so add a special
        // check against that case
        self.noise_count = self.noise_count.wrapping_add(1);
        if ((self.noise_count >> 1) >= self.regs.noise_period()) && (self.noise_count != 1) {
            let bit0 = bitfield(self.noise_state, 0, 1);
            let bit3 = bitfield(self.noise_state, 3, 1);
            let xor_bit = (bit0 ^ bit3) << 17;
            self.noise_state ^= xor_bit;
            self.noise_state >>= 1;
            self.noise_count = 0;
        }

        // Clock envelope; envelope period units are clock/8 (manual says clock/256
        // but that's for all 32 steps)
        self.envelope_count = self.envelope_count.wrapping_add(1);
        if self.envelope_count >= self.regs.envelope_period() {
            self.envelope_state = self.envelope_state.wrapping_add(1);
            self.envelope_count = 0;
        }
    }

    /// Compute sum of channel outputs.
    pub fn output<const N: usize>(&mut self, output: &mut Output<N>) {
        // Volume to amplitude table, taken from MAME's implementation but biased
        // so that 0 == 0
        let s_amplitudes = [
            0, 32, 78, 141, 178, 222, 262, 306, 369, 441, 509, 585, 701, 836, 965, 1112, 1334,
            1595, 1853, 2146, 2576, 3081, 3576, 4135, 5000, 6006, 7023, 8155, 9963, 11976, 14132,
            16382,
        ];

        // Compute the envelope volume
        let envelope_volume = if (self.regs.envelope_hold() || !self.regs.envelope_continue())
            && self.envelope_state >= 32
        {
            self.envelope_state = 32;
            let attack_xor_alt = self.regs.envelope_attack() ^ self.regs.envelope_alternate();
            if (attack_xor_alt) && self.regs.envelope_continue() {
                31
            } else {
                0
            }
        } else {
            let attack = self.regs.envelope_attack();
            let effective_attack = if self.regs.envelope_alternate() {
                attack ^ (bitfield(self.envelope_state, 5, 1) != 0)
            } else {
                attack
            };
            (self.envelope_state & 31) ^ (if effective_attack { 0 } else { 31 })
        };

        // Iterate over channels
        for chan in 0..3 {
            // Noise depends on the noise state, which is the LSB of m_noise_state
            let noise_on = (if self.regs.ch_noise_enable_n(chan as u32) {
                1
            } else {
                0
            }) | (self.noise_state & 1);

            // Tone depends on the current tone state
            let tone_on = (if self.regs.ch_tone_enable_n(chan as u32) {
                1
            } else {
                0
            }) | self.tone_state[chan];

            // If neither tone nor noise enabled, return 0
            let volume = if (noise_on & tone_on) == 0 {
                0
            } else if self.regs.ch_envelope_enable(chan as u32) {
                envelope_volume
            } else {
                let mut vol = self.regs.ch_amplitude(chan as u32) * 2;
                if vol != 0 {
                    vol |= 1;
                }
                vol
            };

            // Convert to amplitude
            output.data[chan] = s_amplitudes[volume as usize];
        }
    }

    /// Read from the SSG registers.
    pub fn read(&mut self, regnum: u32) -> u8 {
        // Read from the I/O ports call the handlers if they are configured for input
        if regnum == 0x0e && !self.regs.io_a_out() {
            return self.intf.external_read(AccessClass::IoPort, 0);
        } else if regnum == 0x0f && !self.regs.io_b_out() {
            return self.intf.external_read(AccessClass::IoPort, 1);
        }

        // Otherwise just return the register value
        self.regs.read(regnum)
    }

    /// Write to the SSG registers.
    pub fn write(&mut self, regnum: u32, data: u8) {
        // Store the raw value to the register array; most writes are passive,
        // consumed only when needed
        self.regs.write(regnum, data);

        // Writes to the envelope shape register reset the state
        if regnum == 0x0d {
            self.envelope_state = 0;
        }
        // Writes to the I/O ports call the handlers if they are configured for output
        else if regnum == 0x0e && self.regs.io_a_out() {
            self.intf.external_write(AccessClass::IoPort, 0, data);
        } else if regnum == 0x0f && self.regs.io_b_out() {
            self.intf.external_write(AccessClass::IoPort, 1, data);
        }
    }

    /// Return a reference to our interface.
    pub fn intf(&self) -> &dyn YmfmInterface {
        &*self.intf
    }

    /// Return a reference to our registers.
    pub fn regs(&self) -> &SsgRegisters {
        &self.regs
    }
}

#[cfg(feature = "serde")]
impl SsgEngine {
    /// Échange les interfaces hôtes (restauration d'un savestate).
    pub(crate) fn swap_intf(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.intf, &mut other.intf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fake interface for testing
    struct TestInterface {}

    impl YmfmInterface for TestInterface {
        fn sync_mode_write(&mut self, _data: u8) {}
        fn sync_check_interrupts(&mut self) {}
        fn external_read(&mut self, _type: AccessClass, _address: u32) -> u8 {
            0
        }
    }

    #[test]
    fn test_ssg_reset_all_registers_zero() {
        let intf = Box::new(TestInterface {});
        let mut engine = SsgEngine::new(intf);
        engine.reset();

        // After reset, all registers should be 0
        for i in 0..16 {
            assert_eq!(engine.read(i), 0, "Register {} should be 0 after reset", i);
        }
    }

    #[test]
    fn test_ssg_tone_output_basic() {
        let intf = Box::new(TestInterface {});
        let mut engine = SsgEngine::new(intf);
        engine.reset();

        // Set channel A tone period to 1 (reg0=1)
        engine.write(0x00, 1);
        // Set channel A volume to 15 (reg8=15)
        engine.write(0x08, 15);
        // Mixer: enable tone for channel A only (reg7=0x3E = 0b00111110)
        // Bit 0 = tone A enable, bit 3 = noise A enable
        // We want tone A on, so set bit 0
        engine.write(0x07, 0x3E);

        // Clock and output several times to verify tone generation
        let mut outputs = Vec::new();
        for _ in 0..8 {
            engine.clock();
            let mut out = Output::<3> { data: [0, 0, 0] };
            engine.output(&mut out);
            outputs.push(out.data[0]);
        }

        // With period=1 and clock/8 rate, tone toggles every clock cycle
        // Volume 15 -> amplitude index 31 (max) -> 16382
        // Expected pattern: alternating between max and 0 due to 50% duty cycle
        assert_eq!(outputs[0], 16382);
        assert_eq!(outputs[1], 0);
        assert_eq!(outputs[2], 16382);
        assert_eq!(outputs[3], 0);
    }

    #[test]
    fn test_ssg_envelope_shape() {
        let intf = Box::new(TestInterface {});
        let mut engine = SsgEngine::new(intf);
        engine.reset();

        // Set envelope period to 1 (fast)
        engine.write(0x0b, 1);
        engine.write(0x0c, 0);
        // Envelope shape: attack/decay alternate (reg13=0x0E = 0b00001110)
        // Bit 2 = attack, bit 1 = alternate
        engine.write(0x0d, 0x0E);

        // Enable envelope for channel A
        engine.write(0x08, 0x1F); // envelope enable + max volume
                                  // Mixer: tone A on
        engine.write(0x07, 0x3E);

        // Clock and observe envelope progression
        let mut volumes = Vec::new();
        for _ in 0..4 {
            engine.clock();
            let mut out = Output::<3> { data: [0, 0, 0] };
            engine.output(&mut out);
            // Reverse-lookup amplitude to get volume index
            volumes.push(out.data[0]);
        }

        // Envelope should be progressing through its shape
        // With attack/alternate, it goes up then down
        assert_ne!(volumes[0], volumes[1], "Envelope should change over time");
    }
}
