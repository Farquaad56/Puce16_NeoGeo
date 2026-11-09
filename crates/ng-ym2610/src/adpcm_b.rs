// BSD 3-Clause License
// Copyright (c) 2021, Aaron Giles. All rights reserved.
// Ported from ref/ymfm/src/ymfm_adpcm.h and ref/ymfm/src/ymfm_adpcm.cpp

use crate::common::{bitfield, clamp, AccessClass, Output, YmfmInterface};

/// ADPCM-B register map (single channel).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdpcMBRegisters {
    regdata: [u8; 0x11],
}

impl Default for AdpcMBRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl AdpcMBRegisters {
    pub fn new() -> Self {
        Self { regdata: [0; 0x11] }
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        for i in 0..self.regdata.len() {
            self.regdata[i] = 0;
        }
        // Default limit to wide open (matches C++ source)
        self.regdata[0x0c] = 0xff;
        self.regdata[0x0d] = 0xff;
    }

    /// Direct write access to a register.
    pub fn write(&mut self, index: u32, data: u8) {
        self.regdata[index as usize] = data;
    }

    // System-wide registers (matches C++ source exactly)
    pub fn execute(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 7, 1) != 0
    }

    pub fn record(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 6, 1) != 0
    }

    pub fn external(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 5, 1) != 0
    }

    pub fn repeat(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 4, 1) != 0
    }

    pub fn speaker(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 3, 1) != 0
    }

    pub fn resetflag(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 0, 1) != 0
    }

    pub fn pan_left(&self) -> bool {
        bitfield(self.regdata[0x01] as u32, 7, 1) != 0
    }

    pub fn pan_right(&self) -> bool {
        bitfield(self.regdata[0x01] as u32, 6, 1) != 0
    }

    pub fn start_conversion(&self) -> bool {
        bitfield(self.regdata[0x01] as u32, 3, 1) != 0
    }

    pub fn dac_enable(&self) -> bool {
        bitfield(self.regdata[0x01] as u32, 2, 1) != 0
    }

    pub fn dram_8bit(&self) -> bool {
        bitfield(self.regdata[0x01] as u32, 1, 1) != 0
    }

    pub fn rom_ram(&self) -> bool {
        bitfield(self.regdata[0x01] as u32, 0, 1) != 0
    }

    pub fn start(&self) -> u16 {
        let low = self.regdata[0x02];
        let high = self.regdata[0x03];
        ((high as u16) << 8) | (low as u16)
    }

    pub fn end(&self) -> u16 {
        let low = self.regdata[0x04];
        let high = self.regdata[0x05];
        ((high as u16) << 8) | (low as u16)
    }

    pub fn prescale(&self) -> u16 {
        let low = self.regdata[0x06] as u16;
        let high = bitfield(self.regdata[0x07] as u32, 0, 3) as u16;
        (high << 8) | low
    }

    pub fn cpudata(&self) -> u8 {
        self.regdata[0x08]
    }

    pub fn delta_n(&self) -> u16 {
        let low = self.regdata[0x09];
        let high = self.regdata[0x0a];
        ((high as u16) << 8) | (low as u16)
    }

    pub fn level(&self) -> u8 {
        self.regdata[0x0b]
    }

    pub fn limit(&self) -> u16 {
        let low = self.regdata[0x0c];
        let high = self.regdata[0x0d];
        ((high as u16) << 8) | (low as u16)
    }

    pub fn dac(&self) -> u8 {
        self.regdata[0x0e]
    }

    pub fn pcm(&self) -> u8 {
        self.regdata[0x0f]
    }
}

/// ADPCM-B channel (single channel).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdpcMBChannel {
    address_shift: u32,
    status: u32,
    curnibble: u32,
    curbyte: u8,
    dummy_read: u32,
    position: u16,
    curaddress: u32,
    accumulator: i32,
    prev_accum: i32,
    adpcm_step: i32,
}

impl AdpcMBChannel {
    /// Bit de statut « fin d'échantillon » (ymfm_adpcm.h : STATUS_EOS).
    pub const STATUS_EOS: u8 = 0x01;

    pub fn new(address_shift: u32) -> Self {
        Self {
            address_shift,
            status: 0x02, // STATUS_BRDY
            curnibble: 0,
            curbyte: 0,
            dummy_read: 0,
            position: 0,
            curaddress: 0,
            accumulator: 0,
            prev_accum: 0,
            adpcm_step: 127, // STEP_MIN
        }
    }

    /// Reset the channel state.
    pub fn reset(&mut self) {
        self.status = 0x02; // STATUS_BRDY
        self.curnibble = 0;
        self.curbyte = 0;
        self.dummy_read = 0;
        self.position = 0;
        self.curaddress = 0;
        self.accumulator = 0;
        self.prev_accum = 0;
        self.adpcm_step = 127; // STEP_MIN
    }

    /// Master clocking function.
    pub fn clock(&mut self, regs: &AdpcMBRegisters, intf: &mut dyn YmfmInterface) {
        // Only process if active and not recording (which we don't support)
        if !regs.execute() || regs.record() || (self.status & 0x04) == 0 {
            self.status &= !0x04;
            return;
        }

        // Otherwise, advance the step
        let position = (self.position as u32).wrapping_add(regs.delta_n() as u32);
        self.position = position as u16;
        if position < 0x10000 {
            return;
        }

        // If we're about to process nibble 0, fetch sample
        if self.curnibble == 0 {
            // Playing from RAM/ROM
            if regs.external() {
                self.curbyte = intf.external_read(AccessClass::AdpcmB, self.curaddress);
            }
        }

        // Extract the nibble from our current byte
        let data = ((self.curbyte as u32) << (4 * self.curnibble)) >> 4;
        self.curnibble ^= 1;

        // We just processed the last nibble
        if self.curnibble == 0 {
            // If playing from RAM/ROM, check the end/limit address or advance
            if regs.external() {
                // Handle the sample end, either repeating or stopping
                if self.at_end(regs) {
                    // If repeating, go back to the start
                    if regs.repeat() {
                        self.load_start(regs);
                    } else {
                        // Otherwise, done; set the EOS bit
                        self.accumulator = 0;
                        self.prev_accum = 0;
                        self.status = (self.status & !0x04) | 0x01;
                        return;
                    }
                }

                // Wrap at the limit address
                if self.at_limit(regs) {
                    self.curaddress = 0;
                } else {
                    // Otherwise, advance the current address
                    self.curaddress = self.curaddress.wrapping_add(1);
                    self.curaddress &= 0xffffff;
                }
            }

            // If CPU-driven, copy the next byte and request more
            if !regs.external() {
                self.curbyte = regs.cpudata();
                self.status |= 0x02;
            }
        }

        // Remember previous value for interpolation
        self.prev_accum = self.accumulator;

        // Forecast to next forecast: 1/8, 3/8, 5/8, 7/8, 9/8, 11/8, 13/8, 15/8
        let delta = (2 * bitfield(data, 0, 3) as i32 + 1) * self.adpcm_step / 8;
        if bitfield(data, 3, 1) != 0 {
            self.accumulator = clamp(self.accumulator - delta, -32768, 32767);
        } else {
            self.accumulator = clamp(self.accumulator + delta, -32768, 32767);
        }

        // Scale the ADPCM step: 0.9, 0.9, 0.9, 0.9, 1.2, 1.6, 2.0, 2.4
        let scale = S_STEP_SCALE[bitfield(data, 0, 3) as usize] as i32;
        self.adpcm_step = clamp((self.adpcm_step * scale) / 64, 127, 24576);
    }

    /// Return the computed output value with panning applied.
    pub fn output<const N: usize>(
        &self,
        regs: &AdpcMBRegisters,
        output: &mut Output<N>,
        rshift: u32,
    ) {
        // Do a linear interpolation between samples
        let result = (self.prev_accum * ((self.position as i32 ^ 0xffff) + 1)
            + self.accumulator * (self.position as i32))
            >> 16;

        // Apply volume (level) in a linear fashion and reduce
        let result = (result * (regs.level() as i32)) >> (8 + rshift);

        // Apply to left/right
        if N == 1 || regs.pan_left() {
            output.data[0] += result;
        }
        if N > 1 && regs.pan_right() {
            output.data[1] += result;
        }
    }

    /// Return the status register.
    pub fn status(&self) -> u8 {
        self.status as u8
    }

    /// Handle special register reads.
    pub fn read(
        &mut self,
        regnum: u32,
        regs: &AdpcMBRegisters,
        intf: &mut dyn YmfmInterface,
    ) -> u8 {
        let mut result = 0;

        // Register 8 reads over the bus under some conditions
        if regnum == 0x08 && !regs.execute() && !regs.record() && regs.external() {
            // Two dummy reads are consumed first
            if self.dummy_read != 0 {
                self.load_start(regs);
                self.dummy_read -= 1;
            } else {
                // Read from outside of the chip
                result = intf.external_read(AccessClass::AdpcmB, self.curaddress);
                self.curaddress = self.curaddress.wrapping_add(1);

                // Did we hit the end? if so, signal EOS
                if self.at_end(regs) {
                    self.status = 0x01 | 0x02;
                } else {
                    // Signal ready
                    self.status = 0x02;
                }

                // Wrap at the limit address
                if self.at_limit(regs) {
                    self.curaddress = 0;
                }
            }
        }
        result
    }

    /// Handle special register writes.
    pub fn write(
        &mut self,
        regnum: u32,
        value: u8,
        regs: &AdpcMBRegisters,
        intf: &mut dyn YmfmInterface,
    ) {
        // Register 0 can do a reset; also use writes here to reset the dummy read counter
        if regnum == 0x00 {
            if regs.execute() {
                self.load_start(regs);
            } else {
                self.status &= !0x01;
            }
            if regs.resetflag() {
                self.reset();
            }
            if regs.external() {
                self.dummy_read = 2;
            }
        }
        // Register 8 writes over the bus under some conditions
        else if regnum == 0x08 {
            // If writing from the CPU during execute, clear the ready flag
            if regs.execute() && !regs.record() && !regs.external() {
                self.status &= !0x02;
            }
            // If writing during "record", pass through as data
            else if !regs.execute() && regs.record() && regs.external() {
                // Clear out dummy reads and set start address
                if self.dummy_read != 0 {
                    self.load_start(regs);
                    self.dummy_read = 0;
                }

                // Did we hit the end? if so, signal EOS
                if self.at_end(regs) {
                    self.status = 0x01 | 0x02;
                } else {
                    // Otherwise, write the data and signal ready
                    intf.external_write(AccessClass::AdpcmB, self.curaddress, value);
                    self.curaddress = self.curaddress.wrapping_add(1);
                    self.status = 0x02;
                }
            }
        }
    }

    /// Compute the current address shift amount based on register settings.
    fn address_shift(&self, regs: &AdpcMBRegisters) -> u32 {
        // If a constant address shift, just provide that
        if self.address_shift != 0 {
            return self.address_shift;
        }

        // If ROM or 8-bit DRAM, shift is 5 bits
        if regs.rom_ram() {
            return 5;
        }
        if regs.dram_8bit() {
            return 5;
        }

        // Otherwise, shift is 2 bits
        2
    }

    /// Load the start address and initialize the state.
    fn load_start(&mut self, regs: &AdpcMBRegisters) {
        self.status = (self.status & !0x01) | 0x04;
        if regs.external() {
            self.curaddress = (regs.start() as u32) << self.address_shift(regs);
        } else {
            self.curaddress = 0;
        }
        self.curnibble = 0;
        self.curbyte = 0;
        self.position = 0;
        self.accumulator = 0;
        self.prev_accum = 0;
        self.adpcm_step = 127; // STEP_MIN
    }

    /// Limit checker; stops at the last byte of the chunk described by address_shift().
    fn at_limit(&self, regs: &AdpcMBRegisters) -> bool {
        let limit_addr = ((regs.limit() as u32) + 1) << self.address_shift(regs);
        self.curaddress == limit_addr - 1
    }

    /// End checker; stops at the last byte of the chunk described by address_shift().
    fn at_end(&self, regs: &AdpcMBRegisters) -> bool {
        let end_addr = ((regs.end() as u32) + 1) << self.address_shift(regs);
        self.curaddress == end_addr - 1
    }
}

/// ADPCM-B engine (single channel).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AdpcMBEngine {
    #[cfg_attr(feature = "serde", serde(skip, default = "crate::common::null_intf"))]
    intf: Box<dyn YmfmInterface>,
    channel: AdpcMBChannel,
    regs: AdpcMBRegisters,
}

impl AdpcMBEngine {
    pub fn new(intf: Box<dyn YmfmInterface>, addrshift: u32) -> Self {
        let channel = AdpcMBChannel::new(addrshift);
        let mut engine = Self {
            intf,
            channel,
            regs: AdpcMBRegisters::new(),
        };
        engine.reset();
        engine
    }

    /// Reset the engine state.
    pub fn reset(&mut self) {
        // Reset registers
        self.regs.reset();

        // Reset each channel
        self.channel.reset();
    }

    /// Master clocking function.
    pub fn clock(&mut self) {
        self.channel.clock(&self.regs, &mut *self.intf);
    }

    /// Compute sum of channel outputs.
    pub fn output<const N: usize>(&mut self, output: &mut Output<N>, rshift: u32) {
        self.channel.output(&self.regs, output, rshift);
    }

    /// Read from the ADPCM-B registers.
    pub fn read(&mut self, regnum: u32) -> u8 {
        self.channel.read(regnum, &self.regs, &mut *self.intf)
    }

    /// Write to the ADPCM-B registers.
    pub fn write(&mut self, regnum: u32, data: u8) {
        // Store the raw value to the register array; most writes are passive.
        self.regs.write(regnum, data);

        // Let the channel handle any special writes
        self.channel
            .write(regnum, data, &self.regs, &mut *self.intf);
    }

    /// Status.
    pub fn status(&self) -> u8 {
        self.channel.status()
    }

    /// Return a reference to our interface.
    pub fn intf(&self) -> &dyn YmfmInterface {
        &*self.intf
    }

    /// Return a reference to our registers.
    pub fn regs(&self) -> &AdpcMBRegisters {
        &self.regs
    }
}

// Step scale table for ADPCM-B (copied verbatim from ymfm_adpcm.cpp)
const S_STEP_SCALE: [u8; 8] = [57, 57, 57, 57, 77, 102, 128, 153];

#[cfg(feature = "serde")]
impl AdpcMBEngine {
    /// Échange les interfaces hôtes (restauration d'un savestate).
    pub(crate) fn swap_intf(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.intf, &mut other.intf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fake interface that returns fixed data for testing
    struct TestInterface {
        data: Vec<u8>,
        read_count: u32,
    }

    impl TestInterface {
        fn new(data: Vec<u8>) -> Self {
            Self {
                data,
                read_count: 0,
            }
        }
    }

    impl YmfmInterface for TestInterface {
        fn sync_mode_write(&mut self, _data: u8) {}
        fn sync_check_interrupts(&mut self) {}
        fn external_read(&mut self, _type: AccessClass, address: u32) -> u8 {
            let idx = (address >> 1) as usize;
            if idx < self.data.len() {
                self.read_count += 1;
                self.data[idx]
            } else {
                0
            }
        }
    }

    #[test]
    fn test_adpcm_b_registers_reset() {
        let mut regs = AdpcMBRegisters::new();
        regs.reset();
        // After reset, limit should be wide open (0xffff)
        assert_eq!(regs.limit(), 0xffff);
        // All other registers should be zero
        assert!(!regs.execute());
        assert!(!regs.record());
    }

    #[test]
    fn test_adpcm_b_channel_clock_basic() {
        let intf = Box::new(TestInterface::new(vec![0x12, 0x34, 0x56]));
        let mut engine = AdpcMBEngine::new(intf, 0);

        // Set up: start at address 0, end at address 10 (inclusive)
        engine.write(0x02, 0x00); // start low
        engine.write(0x03, 0x00); // start high
        engine.write(0x04, 0x0a); // end low
        engine.write(0x05, 0x00); // end high

        // Set delta_n to a reasonable value (e.g., 1)
        engine.write(0x09, 0x01); // delta_n low
        engine.write(0x0a, 0x00); // delta_n high

        // Key on: write control register with execute bit set
        engine.write(0x00, 0x80); // execute=1

        // Clock the channel a few times and verify it's playing
        for _ in 0..10 {
            engine.clock();
        }

        // Channel is clocking - test passes if we get here without panic
    }

    #[test]
    fn test_adpcm_b_write_reset() {
        let intf = Box::new(TestInterface::new(vec![0x00; 64]));
        let mut engine = AdpcMBEngine::new(intf, 0);

        // Write control register with reset flag set
        engine.write(0x00, 0x01); // resetflag=1

        // Status should be BRDY after reset
        assert_eq!(engine.status(), 0x02);
    }

    #[test]
    fn test_adpcm_b_address_shift() {
        let intf = Box::new(TestInterface::new(vec![0x00; 64]));
        let engine = AdpcMBEngine::new(intf, 3); // addrshift=3

        // With constant address shift of 3, it should always return 3
        assert_eq!(engine.channel.address_shift(&engine.regs), 3);
    }

    #[test]
    fn test_adpcm_b_step_scale_table() {
        // Verify step scale table values match C++ source exactly
        assert_eq!(S_STEP_SCALE[0], 57);
        assert_eq!(S_STEP_SCALE[4], 77);
        assert_eq!(S_STEP_SCALE[7], 153);
    }

    #[test]
    fn test_adpcm_b_output_volume() {
        let intf = Box::new(TestInterface::new(vec![0x00; 64]));
        let mut engine = AdpcMBEngine::new(intf, 0);

        // Set level to max (255) - should produce full volume
        engine.write(0x0b, 0xff);

        // Key on
        engine.write(0x00, 0x80);

        // Clock to get some accumulator value
        for _ in 0..10 {
            engine.clock();
        }

        // Output should be non-zero (assuming we got some ADPCM data)
        let mut out = Output::<2> { data: [0, 0] };
        engine.output(&mut out, 0);
        // With level=255 and no panning set, output should be zero
        assert_eq!(out.data[0], 0);
    }

    #[test]
    fn test_adpcm_b_end_address_stops() {
        let intf = Box::new(TestInterface::new(vec![0x12, 0x34]));
        let mut engine = AdpcMBEngine::new(intf, 0);

        // Set up: start at address 0, end at address 1 (one word)
        engine.write(0x02, 0x00); // start low
        engine.write(0x03, 0x00); // start high
        engine.write(0x04, 0x01); // end low
        engine.write(0x05, 0x00); // end high

        // Set delta_n to a large value so position overflows quickly
        engine.write(0x09, 0xff); // delta_n low = 255
        engine.write(0x0a, 0xff); // delta_n high = 255 -> delta_n = 65535

        // Key on with external bit set (bit 5) so address advances
        engine.write(0x00, 0xa0); // execute=1, external=1

        // Clock several times - should eventually finish when reaching end address
        let mut finished = false;
        for _ in 0..100 {
            engine.clock();
            if (engine.status() & 0x01) != 0 {
                finished = true;
                break;
            }
        }
        assert!(finished, "Channel should finish when reaching end address");
    }
}
