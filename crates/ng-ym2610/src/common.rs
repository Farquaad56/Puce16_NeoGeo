// BSD 3-Clause License
//
// Copyright (c) 2021, Aaron Giles
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this
//    list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its
//    contributors may be used to endorse or promote products derived from
//    this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Ported from: ref/ymfm/src/ymfm.h (lines 86-317)

/// Extract a bitfield from the given value, starting at bit 'start' for a length of 'length' bits.
#[inline]
pub fn bitfield(value: u32, start: i32, length: i32) -> u32 {
    (value >> start) & ((1 << length) - 1)
}

/// Extract a single bit from the given value at position 'b'.
#[inline]
pub fn bit(value: u32, b: i32) -> u32 {
    (value >> b) & 1
}

/// Clamp between the minimum and maximum values provided.
#[inline]
pub fn clamp(value: i32, minval: i32, maxval: i32) -> i32 {
    if value < minval {
        return minval;
    }
    if value > maxval {
        return maxval;
    }
    value
}

/// Count the number of leading zeros in a 32-bit value.
#[inline]
pub fn count_leading_zeros_u32(value: u32) -> u8 {
    if value == 0 {
        return 32;
    }
    // Use Rust's built-in clz (equivalent to __builtin_clz on GCC or _BitScanReverse on MSVC)
    value.leading_zeros() as u8
}

/// Compute the result of a round trip through the encode/decode process.
/// This is equivalent to encoding a 32-bit signed value to 3.10 floating point,
/// then decoding it back to a 16-bit signed value.
#[inline]
pub fn roundtrip_fp(value: i32) -> i32 {
    // Handle overflows first
    if value < -32768 {
        return -32768;
    }
    if value > 32767 {
        return 32767;
    }

    // We need to count the number of leading sign bits after the sign.
    // We can use count_leading_zeros if we invert negative values.
    let scanvalue = value ^ (value >> 31);

    // Exponent is related to the number of leading bits starting from bit 14.
    let exponent = 7 - count_leading_zeros_u32((scanvalue as u32) << 17) as i32;

    // Smallest exponent value allowed is 1.
    let exponent = if exponent < 1 { 1 } else { exponent };

    // Apply the shift back and forth to zero out bits that are lost.
    let mask = (1 << (exponent - 1)) - 1;
    value & !mask
}

// External I/O access classes (from ymfm.h enum access_class)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessClass {
    IoPort = 0,
    AdpcmA = 1,
    AdpcmB = 2,
    Pcm = 3,
}

/// Interface between the FM engine and the outside world.
/// Provides hooks for timers, synchronization, and I/O.
pub trait YmfmInterface {
    /// Called when a write happens to the mode register.
    fn sync_mode_write(&mut self, data: u8);

    /// Called when the chip's status has changed.
    fn sync_check_interrupts(&mut self);

    /// Called when one of the two internal timers has changed state.
    fn set_timer(&mut self, _tnum: u32, _duration_in_clocks: i32) {}

    /// Called to indicate that the chip should be considered in a busy state.
    fn set_busy_end(&mut self, _clocks: u32) {}

    /// Called to see if the chip is still currently in a busy state.
    fn is_busy(&self) -> bool {
        false
    }

    /// Called when the state of the IRQ signal has changed.
    fn update_irq(&mut self, _asserted: bool) {}

    /// Called whenever data is read from outside of the chip.
    fn external_read(&mut self, _type: AccessClass, _address: u32) -> u8 {
        0
    }

    /// Called whenever data is written outside of the chip.
    fn external_write(&mut self, _type: AccessClass, _address: u32, _data: u8) {}
}

/// Struct containing an array of output values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Output<const N: usize> {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_arr"))]
    pub data: [i32; N],
}

impl<const N: usize> Output<N> {
    /// Clear all outputs to 0.
    #[inline]
    pub fn clear(&mut self) {
        for i in 0..N {
            self.data[i] = 0;
        }
    }

    /// Clamp all outputs to a 16-bit signed value.
    #[inline]
    pub fn clamp16(&mut self) {
        for i in 0..N {
            self.data[i] = clamp(self.data[i], -32768, 32767);
        }
    }

    /// Run each output value through the floating-point processor.
    #[inline]
    pub fn roundtrip_fp(&mut self) {
        for i in 0..N {
            self.data[i] = roundtrip_fp(self.data[i]);
        }
    }

    /// Add another Output to this one (element-wise).
    #[inline]
    pub fn add(&mut self, other: &Output<N>) {
        for i in 0..N {
            self.data[i] += other.data[i];
        }
    }
}

// Envelope states (ymfm.h `envelope_state`).
/// OPLL only.
pub const EG_DEPRESS: usize = 0;
pub const EG_ATTACK: usize = 1;
pub const EG_DECAY: usize = 2;
pub const EG_SUSTAIN: usize = 3;
pub const EG_RELEASE: usize = 4;
/// OPQ/OPZ only.
pub const EG_REVERB: usize = 5;
pub const EG_STATES: usize = 6;

/// Interface inerte posée à la désérialisation, remplacée par `Ym2610::restore`.
#[cfg(feature = "serde")]
pub(crate) struct NullIntf;

#[cfg(feature = "serde")]
impl YmfmInterface for NullIntf {
    fn sync_mode_write(&mut self, _data: u8) {}
    fn sync_check_interrupts(&mut self) {}
}

#[cfg(feature = "serde")]
pub(crate) fn null_intf() -> Box<dyn YmfmInterface> {
    Box::new(NullIntf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitfield() {
        assert_eq!(bitfield(0b1011_0100, 2, 3), 0b101);
    }

    #[test]
    fn test_bit() {
        assert_eq!(bit(0x80, 7), 1);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(300, -128, 127), 127);
    }

    #[test]
    fn test_roundtrip_fp_zero() {
        // value=0: scanvalue=0, clz(0<<17)=32, exponent=7-32=-25 -> max(-25,1)=1, mask=(1<<0)-1=0, result=0&~0=0
        assert_eq!(roundtrip_fp(0), 0);
    }

    #[test]
    fn test_roundtrip_fp_one() {
        // value=1: scanvalue=1, clz(1<<17)=clz(0x20000)=15, exponent=7-15=-8 -> max(-8,1)=1, mask=(1<<0)-1=0, result=1&~0=1
        assert_eq!(roundtrip_fp(1), 1);
    }

    #[test]
    fn test_roundtrip_fp_511() {
        // value=0x1FF (511): scanvalue=511, clz(511<<17)=6, exponent=7-6=1, mask=(1<<0)-1=0, result=511&~0=511
        assert_eq!(roundtrip_fp(0x1FF), 511);
    }

    #[test]
    fn test_roundtrip_fp_32767() {
        // value=0x7FFF (32767): scanvalue=32767, clz(32767<<17)=0 (overflow wraps to 0xFFFE0000), exponent=7-0=7, mask=(1<<6)-1=63, result=32767&~63=32704
        assert_eq!(roundtrip_fp(0x7FFF), 32704);
    }

    #[test]
    fn test_roundtrip_fp_overflow() {
        // value=0x12345 (74565) > 32767, so it overflows and returns 32767.
        assert_eq!(roundtrip_fp(0x12345), 32767);
    }

    #[test]
    fn test_output_clear_and_add() {
        let mut out = Output::<2> { data: [0, 0] };
        out.clear();
        assert_eq!(out.data[0], 0);
        assert_eq!(out.data[1], 0);

        let other = Output::<2> { data: [10, 20] };
        out.add(&other);
        assert_eq!(out.data[0], 10);
        assert_eq!(out.data[1], 20);
    }
}
