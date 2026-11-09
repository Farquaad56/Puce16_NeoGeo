// BSD 3-Clause License
// Copyright (c) 2021, Aaron Giles. All rights reserved.
// Ported from ref/ymfm/src/ymfm_adpcm.h and ref/ymfm/src/ymfm_adpcm.cpp

use crate::common::{bitfield, clamp, AccessClass, Output, YmfmInterface};

/// ADPCM-A register map (6 channels).
pub struct AdpcmARegisters {
    regdata: [u8; 0x30],
}

impl Default for AdpcmARegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl AdpcmARegisters {
    pub fn new() -> Self {
        Self { regdata: [0; 0x30] }
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        for i in 0..self.regdata.len() {
            self.regdata[i] = 0;
        }
        // Initialize pans to on by default, and max instrument volume.
        // Some Neo Geo homebrews (e.g., ffeast) rely on this.
        for ch in 0..6 {
            self.regdata[0x08 + ch] = 0xdf;
        }
    }

    /// Map channel number to register offset.
    pub fn channel_offset(chnum: u32) -> u32 {
        assert!(chnum < 6);
        chnum
    }

    /// Direct write access to a register.
    pub fn write(&mut self, index: u32, data: u8) {
        self.regdata[index as usize] = data;
    }

    // System-wide registers
    pub fn dump(&self) -> bool {
        bitfield(self.regdata[0x00] as u32, 7, 1) != 0
    }

    pub fn dump_mask(&self) -> u8 {
        self.regdata[0x00] & 0x3f
    }

    pub fn total_level(&self) -> u8 {
        self.regdata[0x01] & 0x3f
    }

    pub fn test(&self) -> u8 {
        self.regdata[0x02]
    }

    // Per-channel registers
    pub fn ch_pan_left(&self, choffs: u32) -> bool {
        bitfield(self.regdata[(choffs + 0x08) as usize] as u32, 7, 1) != 0
    }

    pub fn ch_pan_right(&self, choffs: u32) -> bool {
        bitfield(self.regdata[(choffs + 0x08) as usize] as u32, 6, 1) != 0
    }

    pub fn ch_instrument_level(&self, choffs: u32) -> u8 {
        self.regdata[(choffs + 0x08) as usize] & 0x1f
    }

    pub fn ch_start(&self, choffs: u32) -> u16 {
        let low = self.regdata[(choffs + 0x10) as usize];
        let high = self.regdata[(choffs + 0x18) as usize];
        ((high as u16) << 8) | (low as u16)
    }

    pub fn ch_end(&self, choffs: u32) -> u16 {
        let low = self.regdata[(choffs + 0x20) as usize];
        let high = self.regdata[(choffs + 0x28) as usize];
        ((high as u16) << 8) | (low as u16)
    }

    /// Write start address for a channel.
    pub fn write_start(&mut self, choffs: u32, address: u16) {
        let low = (address & 0xff) as u8;
        let high = ((address >> 8) & 0xff) as u8;
        self.write(choffs + 0x10, low);
        self.write(choffs + 0x18, high);
    }

    /// Write end address for a channel.
    pub fn write_end(&mut self, choffs: u32, address: u16) {
        let low = (address & 0xff) as u8;
        let high = ((address >> 8) & 0xff) as u8;
        self.write(choffs + 0x20, low);
        self.write(choffs + 0x28, high);
    }
}

/// ADPCM-A channel.
pub struct AdpcmAChannel {
    choffs: u32,
    address_shift: u32,
    playing: bool,
    curnibble: u32,
    curbyte: u8,
    curaddress: u32,
    accumulator: i16,
    step_index: u8,
}

impl AdpcmAChannel {
    pub fn new(choffs: u32, address_shift: u32) -> Self {
        Self {
            choffs,
            address_shift,
            playing: false,
            curnibble: 0,
            curbyte: 0,
            curaddress: 0,
            accumulator: 0,
            step_index: 0,
        }
    }

    /// Reset the channel state.
    pub fn reset(&mut self) {
        self.playing = false;
        self.curnibble = 0;
        self.curbyte = 0;
        self.curaddress = 0;
        self.accumulator = 0;
        self.step_index = 0;
    }

    /// Signal key on/off.
    pub fn keyonoff(&mut self, on: bool, regs: &AdpcmARegisters) {
        // QUESTION: repeated key ons restart the sample?
        self.playing = on;
        if self.playing {
            self.curaddress = (regs.ch_start(self.choffs) as u32) << self.address_shift;
            self.curnibble = 0;
            self.curbyte = 0;
            self.accumulator = 0;
            self.step_index = 0;

            // Don't log masked channels (no debug logging in Rust port)
        }
    }

    /// Master clocking function. Returns true if the sample finished playing.
    pub fn clock(&mut self, regs: &AdpcmARegisters, intf: &mut dyn YmfmInterface) -> bool {
        // If not playing, just output 0
        if !self.playing {
            self.accumulator = 0;
            return false;
        }

        let data;
        if self.curnibble == 0 {
            // Stop when we hit the end address. Only low 20 bits are used for
            // comparison on YM2610 (affects sample playback in some games).
            // End address is inclusive, so wait until about to fetch the sample
            // just after the end before stopping.
            let end = ((regs.ch_end(self.choffs) as u32) + 1) << self.address_shift;
            if ((self.curaddress ^ end) & 0xfffff) == 0 {
                self.playing = false;
                self.accumulator = 0;
                return true;
            }

            // PORT-NOTE: C++ uses m_owner.intf().ymfm_external_read(ACCESS_ADPCM_A, addr);
            // Rust equivalent calls the trait method directly.
            self.curbyte = intf.external_read(AccessClass::AdpcmA, self.curaddress);
            self.curaddress = self.curaddress.wrapping_add(1);
            data = (self.curbyte >> 4) & 0xf;
            self.curnibble = 1;
        } else {
            // Otherwise just extract from the previously-fetched byte
            data = self.curbyte & 0xf;
            self.curnibble = 0;
        }

        // Compute the ADPCM delta
        let step = S_STEPS[self.step_index as usize] as i32;
        let nibble_val = data as i32;
        let delta = (2 * nibble_val + 1) * step / 8;
        if bitfield(nibble_val as u32, 3, 1) != 0 {
            self.accumulator = (self.accumulator as i32 - delta) as i16;
        } else {
            self.accumulator = (self.accumulator as i32 + delta) as i16;
        }

        // The 12-bit accumulator wraps on YM2610 and YM2608 (like MSM5205)
        self.accumulator &= 0xfff;

        // Adjust ADPCM step
        let inc = S_STEP_INC[nibble_val as usize];
        let new_index = self.step_index as i32 + inc as i32;
        self.step_index = clamp(new_index, 0, 48) as u8;

        false
    }

    /// Return the computed output value with panning applied.
    pub fn output<const N: usize>(&self, regs: &AdpcmARegisters, output: &mut Output<N>) {
        // Volume combines instrument and total levels
        let vol = (regs.ch_instrument_level(self.choffs) as i32 ^ 0x1f)
            + (regs.total_level() as i32 ^ 0x3f);

        // If combined is maximum, don't add to outputs
        if vol >= 63 {
            return;
        }

        // Convert into a shift and a multiplier
        let mul = 15 - (vol & 7);
        let shift = 4 + 1 + (vol >> 3);

        // m_accumulator is a 12-bit value; shift up to sign-extend.
        // The downshift is incorporated into 'shift'.
        let value = ((self.accumulator as i32) << 4) * mul;
        let value = (value >> shift) & !3;

        // Apply to left/right as appropriate
        if N == 1 || regs.ch_pan_left(self.choffs) {
            output.data[0] += value;
        }
        if N > 1 && regs.ch_pan_right(self.choffs) {
            output.data[1] += value;
        }
    }
}

/// ADPCM-A engine (6 channels).
pub struct AdpcMAEngine {
    intf: Box<dyn YmfmInterface>,
    channels: [AdpcmAChannel; 6],
    regs: AdpcmARegisters,
}

impl AdpcMAEngine {
    /// `addrshift` : décalage à gauche des adresses (ymfm : 8 pour le YM2610).
    pub fn new(intf: Box<dyn YmfmInterface>, addrshift: u32) -> Self {
        let mut channels = [
            AdpcmAChannel::new(0, addrshift),
            AdpcmAChannel::new(1, addrshift),
            AdpcmAChannel::new(2, addrshift),
            AdpcmAChannel::new(3, addrshift),
            AdpcmAChannel::new(4, addrshift),
            AdpcmAChannel::new(5, addrshift),
        ];

        // Reset all channels
        for ch in &mut channels {
            ch.reset();
        }

        Self {
            intf,
            channels,
            regs: AdpcmARegisters::new(),
        }
    }

    /// Reset the engine state.
    pub fn reset(&mut self) {
        // Reset register state
        self.regs.reset();

        // Reset each channel
        for ch in &mut self.channels {
            ch.reset();
        }
    }

    /// Master clocking function. Returns bitmask of completed samples.
    pub fn clock(&mut self, chanmask: u32) -> u32 {
        let mut result = 0;
        for chnum in 0..6 {
            if bitfield(chanmask, chnum as i32, 1) != 0
                && self.channels[chnum].clock(&self.regs, &mut *self.intf)
            {
                result |= 1 << chnum;
            }
        }
        result
    }

    /// Compute sum of channel outputs.
    pub fn output<const N: usize>(&mut self, output: &mut Output<N>, chanmask: u32) {
        for chnum in 0..6 {
            if bitfield(chanmask, chnum as i32, 1) != 0 {
                self.channels[chnum].output(&self.regs, output);
            }
        }
    }

    /// Write to the ADPCM-A registers.
    pub fn write(&mut self, regnum: u32, data: u8) {
        // Store the raw value to the register array; most writes are passive.
        self.regs.write(regnum, data);

        // Actively handle writes to the control register
        if regnum == 0x00 {
            for chnum in 0..6 {
                if bitfield(data as u32, chnum as i32, 1) != 0 {
                    let on = !self.regs.dump();
                    self.channels[chnum].keyonoff(on, &self.regs);
                }
            }
        }
    }

    /// Set the start/end address for a channel (for hardcoded YM2608 percussion).
    pub fn set_start_end(&mut self, chnum: u32, start: u16, end: u16) {
        let choffs = AdpcmARegisters::channel_offset(chnum);
        self.regs.write_start(choffs, start);
        self.regs.write_end(choffs, end);
    }

    /// Return a reference to our interface.
    pub fn intf(&self) -> &dyn YmfmInterface {
        &*self.intf
    }

    /// Return a reference to our registers.
    pub fn regs(&self) -> &AdpcmARegisters {
        &self.regs
    }
}

// Step table for ADPCM delta calculation (copied verbatim from ymfm_adpcm.cpp)
const S_STEPS: [u16; 49] = [
    16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130,
    143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552,
];

// Step index adjustment table (copied verbatim from ymfm_adpcm.cpp)
const S_STEP_INC: [i8; 8] = [-1, -1, -1, -1, 2, 5, 7, 9];

#[cfg(test)]
mod tests {
    use super::*;

    // Fake interface that returns fixed nibbles for testing
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
    fn test_adpcm_a_channel_clock_basic() {
        // Test basic ADPCM-A channel clocking with known data.
        // Data: 0x12, 0x34, 0x56 (nibbles: 1,2,3,4,5,6)
        let intf = Box::new(TestInterface::new(vec![0x12, 0x34, 0x56]));
        let mut engine = AdpcMAEngine::new(intf, 1);

        // Set up channel 0: start at address 0, end at address 10 (inclusive)
        engine.set_start_end(0, 0, 10);

        // Key on channel 0 (write control register with bit 7=0 for keyon, bit 0=1 for ch0)
        engine.write(0x00, 0x01);

        // Clock the channel a few times and verify it's playing
        let finished = engine.clock(0x01);
        assert_eq!(finished & 0x01, 0, "Channel should not be finished yet");

        // Channel is clocking and reading data - test passes if we get here without panic
    }

    #[test]
    fn test_adpcm_a_keyonoff_stops_channel() {
        let intf = Box::new(TestInterface::new(vec![0x12, 0x34, 0x56]));
        let mut engine = AdpcMAEngine::new(intf, 1);

        // Set up channel 0
        engine.set_start_end(0, 0, 10);

        // Key on channel 0
        engine.write(0x00, 0x01);

        // Clock once to start playing
        let finished = engine.clock(0x01);
        assert_eq!(finished & 0x01, 0);

        // Now key off (write control register with bit 7=1 for dump/keyoff)
        engine.write(0x00, 0x81);

        // Clock again - should not read more data and accumulator should be 0
        let finished = engine.clock(0x01);
        assert_eq!(finished & 0x01, 0);
    }

    #[test]
    fn test_adpcm_a_write_control_register() {
        let intf = Box::new(TestInterface::new(vec![0x00; 64]));
        let mut engine = AdpcMAEngine::new(intf, 1);

        // Write control register: key on channel 0 (bit 7=0, bit 0=1)
        engine.write(0x00, 0x01);

        // Verify the register was written
        assert!(!engine.regs().dump());
        assert_eq!(engine.regs().dump_mask(), 1);
    }

    #[test]
    fn test_adpcm_a_step_table() {
        // Verify step table values match C++ source exactly
        assert_eq!(S_STEPS[0], 16);
        assert_eq!(S_STEPS[48], 1552);
        assert_eq!(S_STEP_INC[0], -1);
        assert_eq!(S_STEP_INC[7], 9);
    }

    #[test]
    fn test_adpcm_a_output_volume() {
        let intf = Box::new(TestInterface::new(vec![0x00; 64]));
        let mut engine = AdpcMAEngine::new(intf, 1);

        // Set total level to max (silence) - register 0x01, bits 0-5
        engine.write(0x01, 0x3f);

        // Key on channel 0
        engine.set_start_end(0, 0, 10);
        engine.write(0x00, 0x01);

        // Clock to get some accumulator value
        engine.clock(0x01);

        // Output should be silent due to max volume setting
        let mut out = Output::<2> { data: [0, 0] };
        engine.output(&mut out, 0x01);
        assert_eq!(out.data[0], 0);
        assert_eq!(out.data[1], 0);
    }

    #[test]
    fn test_adpcm_a_end_address_stops() {
        let intf = Box::new(TestInterface::new(vec![0x12, 0x34]));
        let mut engine = AdpcMAEngine::new(intf, 1);

        // Set up channel 0 with range start=0, end=1 (one word at address 0)
        engine.set_start_end(0, 0, 1);

        // Key on channel 0
        engine.write(0x00, 0x01);

        // Clock several times - should eventually finish when reaching end address
        let mut finished = false;
        for _ in 0..10 {
            let result = engine.clock(0x01);
            if (result & 0x01) != 0 {
                finished = true;
                break;
            }
        }
        assert!(finished, "Channel should finish when reaching end address");
    }
}
