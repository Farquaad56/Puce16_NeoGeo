#![forbid(unsafe_code)]
//! ng-lspc : LSPC (vidéo). Voir tasks/T400.
#[rustfmt::skip]
pub mod dac_table;
#[rustfmt::skip]
pub mod zoom_table;

pub mod fix;
pub mod palette;
pub mod regs;
pub mod sprites;
pub mod timing;
pub mod vram;

/// LSPC state machine.
pub struct Lspc {
    pub vram: vram::Vram,
    /// Auto-animation speed (frames per tick).
    pub anim_speed: u8,
    /// Auto-animation disabled flag.
    pub anim_disabled: bool,
    /// Current animation counter (lower 3 bits used by renderer).
    pub anim_counter: u8,
    /// Internal frame counter for auto-animation timing.
    anim_frame_counter: u8,
    /// IRQ2 control register ($3C0006 & 0x00F0).
    pub irq2_ctrl: u16,
    /// Display position timer reload value (32-bit).
    pub display_counter: u32,
    /// Current vertical position (MAME line 0..263), updated by timing.
    pub vpos: u16,

    // Timing state (T401)
    time: u64,
    irq3: bool,
    raster: bool,
    vblank: bool,
    timer_deadline: Option<u64>,
    t_vbl_irq: u64,
    t_reload: u64,
    t_anim: u64,
    next_line: u64,
}

impl Lspc {
    pub fn new() -> Self {
        use timing::{MCLK_PER_LINE, RELOAD_OFFSET, VBLANK_IRQ_OFFSET, VBSTART, VSSTART};
        Self {
            vram: vram::Vram::new(),
            anim_speed: 0,
            anim_disabled: false,
            anim_counter: 0,
            anim_frame_counter: 0,
            irq2_ctrl: 0,
            display_counter: 0,
            vpos: 0,
            time: 0,
            irq3: true, // MAME: start with IRQ3 pending at reset
            raster: false,
            vblank: false,
            timer_deadline: None,
            t_vbl_irq: VBSTART as u64 * MCLK_PER_LINE + VBLANK_IRQ_OFFSET,
            t_reload: VBSTART as u64 * MCLK_PER_LINE + RELOAD_OFFSET,
            t_anim: VSSTART as u64 * MCLK_PER_LINE,
            next_line: 0,
        }
    }

    /// Read a video register (word offset 0..7).
    /// Returns None for unmapped accesses (lower byte only reads).
    pub fn read_reg(&mut self, offset: u32, upper: bool, lower: bool) -> Option<u16> {
        // Lower byte only reads are not mapped
        if !upper && lower {
            return None;
        }
        match offset {
            0 | 1 => Some(self.vram.get_data()),
            2 => Some(self.vram.get_modulo()),
            3 => Some(self.get_video_control()),
            _ => Some(self.vram.get_data()), // default case from C++ switch
        }
    }

    /// Write a video register (word offset 0..7).
    pub fn write_reg(&mut self, offset: u32, data: u16, upper: bool, lower: bool) {
        // Lower byte only writes are ignored
        if !upper && lower {
            return;
        }
        // Upper byte only: duplicate to both bytes
        let mut d = data;
        if upper && !lower {
            d = (d & 0xFF00) | (d >> 8);
        }
        match offset {
            0 => self.vram.set_offset(d),
            1 => self.vram.set_data(d),
            2 => self.vram.set_modulo(d),
            3 => self.set_video_control(d),
            4 => self.set_timer_high(d),
            5 => self.set_timer_low(d),
            6 => self.acknowledge(d),
            _ => {} // d0: pause timer for 32 lines (LSPC2 only) or unmapped
        }
    }

    /// Get video control register value ($3C0006 read).
    fn get_video_control(&self) -> u16 {
        let mut v = self.vpos + 0x100;
        if v >= 0x200 {
            v -= 264; // NEOGEO_VTOTAL
        }
        (v << 7) | ((self.anim_counter & 7) as u16)
    }

    /// Set video control register ($3C0006 write).
    fn set_video_control(&mut self, data: u16) {
        self.anim_speed = (data >> 8) as u8;
        self.anim_disabled = (data & 8) != 0;
        self.irq2_ctrl = data & 0x00F0;
    }

    /// Timer high word write ($3C0008).
    fn set_timer_high(&mut self, d: u16) {
        self.display_counter = (self.display_counter & 0xFFFF) | ((d as u32) << 16);
    }

    /// Auto-animation tick (called by timing at vpos=256).
    pub fn auto_animation_tick(&mut self) {
        if self.anim_frame_counter == 0 {
            self.anim_frame_counter = self.anim_speed;
            self.anim_counter = self.anim_counter.wrapping_add(1);
        } else {
            self.anim_frame_counter -= 1;
        }
    }
}

impl Default for Lspc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod smoke {
    #[test]
    fn it_builds() {}
}
