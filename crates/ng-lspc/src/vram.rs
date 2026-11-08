//! VRAM access for LSPC (neogeo_spr.cpp l.76-110).
//! 0x0000-0x7FFF slow, 0x8000-0x87FF fast; index = address & 0x87FF.

pub struct Vram {
    pub data: Box<[u16; 0x8800]>,
    pub(crate) offset: u16,
    read_buffer: u16,
    modulo: u16,
}

impl Vram {
    pub fn new() -> Self {
        let mut v = Self {
            data: Box::new([0; 0x8800]),
            offset: 0,
            read_buffer: 0,
            modulo: 0,
        };
        // Initial read at offset 0
        v.read_buffer = v.data[0];
        v
    }

    pub fn set_offset(&mut self, data: u16) {
        if data & 0x8000 != 0 {
            self.offset = data & 0x87FF;
        } else {
            self.offset = data;
        }
        // Read happens immediately
        self.read_buffer = self.data[self.offset as usize];
    }

    pub fn get_data(&self) -> u16 {
        self.read_buffer
    }

    pub fn set_data(&mut self, data: u16) {
        self.data[self.offset as usize] = data;
        // Auto increment/decrement; A15 is NOT affected
        let new_offset =
            (self.offset & 0x8000) | ((self.offset as i32 + self.modulo as i32) as u16 & 0x7FFF);
        self.set_offset(new_offset);
    }

    pub fn set_modulo(&mut self, data: u16) {
        self.modulo = data;
    }

    pub fn get_modulo(&self) -> u16 {
        self.modulo
    }

    /// Direct access for tests and rendering (not through the offset mechanism).
    pub fn read_word(&self, addr: usize) -> u16 {
        let idx = if addr & 0x8000 != 0 {
            addr & 0x87FF
        } else {
            addr
        };
        self.data[idx]
    }

    pub fn write_word(&mut self, addr: usize, data: u16) {
        let idx = if addr & 0x8000 != 0 {
            addr & 0x87FF
        } else {
            addr
        };
        self.data[idx] = data;
    }
}

impl Default for Vram {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vram_basic_write_read() {
        let mut v = Vram::new();
        // Write at offset 0x1234, modulo 1
        v.set_offset(0x1234);
        assert_eq!(v.offset, 0x1234);
        v.set_modulo(1);
        assert_eq!(v.modulo, 1);
        // Direct write and read at same index
        let idx: usize = 0x1234;
        v.data[idx] = 0xBEEF;
        assert_eq!(v.data[idx], 0xBEEF);
        assert_eq!(v.read_word(idx), 0xBEEF);
    }

    #[test]
    fn vram_fast_zone_bit15_preserved() {
        let mut v = Vram::new();
        v.set_offset(0x8001);
        v.set_modulo(1);
        assert_eq!(v.offset, 0x8001); // bit 15 preserved via masking
        v.set_data(0x1111);
        assert_eq!(v.read_word(0x8001), 0x1111);
        // Offset should be 0x8002 (bit 15 still set)
        assert_eq!(v.offset, 0x8002);
    }

    #[test]
    fn vram_modulo_minus_one_wraps() {
        let mut v = Vram::new();
        v.set_offset(0x0000);
        v.set_modulo(0xFFFF); // -1 as i16
        v.set_data(0xAAAA);
        assert_eq!(v.read_word(0x0000), 0xAAAA);
        // (0 + (-1)) & 0x7FFF = 0x7FFF
        assert_eq!(v.offset, 0x7FFF);
    }

    #[test]
    fn vram_read_buffer_immediate() {
        let mut v = Vram::new();
        // Write a known value at offset 0
        v.set_offset(0);
        v.set_data(0xDEAD);
        // Now set offset to read it back
        v.set_offset(0);
        assert_eq!(v.get_data(), 0xDEAD);
    }

    #[test]
    fn vram_modulo_read_write() {
        let mut v = Vram::new();
        v.set_modulo(42);
        assert_eq!(v.get_modulo(), 42);
    }
}
