//! Video timing, IRQ generation, display timer, auto-animation scheduling.
//!
//! All times are in master clock ticks (24 MHz). One pixel = 4 mclk, one line = 1536 mclk.

use crate::Lspc;

/// Master clock ticks per pixel (MAME: NEOGEO_PIXEL_CLOCK = MCLK/4)
pub const MCLK_PER_PIXEL: u64 = 4;
/// Master clock ticks per scanline (384 pixels * 4)
pub const MCLK_PER_LINE: u64 = 1536;
/// Total lines per frame (MAME: NEOGEO_VTOTAL)
pub const LINES: u16 = 264;
/// Vertical blank start line (MAME: NEOGEO_VBSTART)
pub const VBSTART: u16 = 240;
/// Vertical sync start line (MAME: NEOGEO_VSSTART)
pub const VSSTART: u16 = 256;
/// Vblank IRQ offset from HSYNC rising edge (56+2 mclk, MAME: NEOGEO_VBLANK_IRQ_HTIM)
pub const VBLANK_IRQ_OFFSET: u64 = 58;
/// Autoload vblank reload offset from HSYNC rising edge (MAME: NEOGEO_VBLANK_RELOAD_HTIM)
pub const RELOAD_OFFSET: u64 = 1146;

impl Lspc {
    /// Current time in master clock ticks since reset.
    pub fn time(&self) -> u64 {
        self.time
    }

    /// Current IRQ level (highest priority pending).
    /// Returns 3 if irq3, 2 if raster, 1 if vblank, 0 otherwise.
    pub fn irq_level(&self) -> u8 {
        if self.irq3 {
            return 3;
        }
        if self.raster {
            return 2;
        }
        if self.vblank {
            return 1;
        }
        0
    }

    /// Advance time to `to_mclk`, processing all video events in chronological order.
    /// Stops at the first line start event and returns Some(vpos) for that line.
    /// If no line start is reached before to_mclk, advances to to_mclk and returns None.
    pub fn advance(&mut self, to_mclk: u64) -> Option<u16> {
        loop {
            // Determine next event deadline
            let mut next_deadline = to_mclk;

            if let Some(dl) = self.timer_deadline {
                if dl < next_deadline {
                    next_deadline = dl;
                }
            }
            if self.t_vbl_irq < next_deadline {
                next_deadline = self.t_vbl_irq;
            }
            if self.t_reload < next_deadline {
                next_deadline = self.t_reload;
            }
            if self.t_anim < next_deadline {
                next_deadline = self.t_anim;
            }
            if self.next_line < next_deadline {
                next_deadline = self.next_line;
            }

            // Advance time to the event deadline (or to_mclk)
            self.time = next_deadline;

            // Update vpos based on current time
            let line_num = ((self.time / MCLK_PER_LINE) % LINES as u64) as u16;
            if line_num != self.vpos {
                self.vpos = line_num;
            }

            // Process events at this deadline in priority order:
            // vblank_irq, reload, display timer, animation, line start

            // 1. Vblank IRQ
            if self.time == self.t_vbl_irq {
                self.vblank = true;
                self.t_vbl_irq += LINES as u64 * MCLK_PER_LINE;
            }

            // 2. Autoload vblank reload
            if self.time == self.t_reload {
                if (self.irq2_ctrl & 0x40) != 0 {
                    let period = ((self.display_counter + 1) as u64) * MCLK_PER_PIXEL;
                    self.timer_deadline = Some(self.t_reload + period);
                }
                self.t_reload += LINES as u64 * MCLK_PER_LINE;
            }

            // 3. Display position timer (raster IRQ)
            if let Some(dl) = self.timer_deadline {
                if self.time == dl {
                    if (self.irq2_ctrl & 0x10) != 0 {
                        self.raster = true;
                    }
                    if (self.irq2_ctrl & 0x80) != 0 {
                        let period = ((self.display_counter + 1) as u64) * MCLK_PER_PIXEL;
                        self.timer_deadline = Some(dl + period);
                    } else {
                        self.timer_deadline = None;
                    }
                }
            }

            // 4. Auto-animation timer
            if self.time == self.t_anim {
                self.auto_animation_tick();
                self.t_anim += LINES as u64 * MCLK_PER_LINE;
            }

            // 5. Line start - stop and report
            if self.time == self.next_line {
                let reported_vpos = self.vpos;
                self.next_line += MCLK_PER_LINE;
                return Some(reported_vpos);
            }

            // If we've reached to_mclk without a line start, exit
            if self.time >= to_mclk {
                return None;
            }
        }
    }

    /// Set display timer low word ($3C000A). Arms relative timer if bit 5 set.
    pub fn set_timer_low(&mut self, d: u16) {
        self.display_counter = (self.display_counter & 0xFFFF0000) | (d as u32);
        if (self.irq2_ctrl & 0x20) != 0 {
            let period = ((self.display_counter + 1) as u64) * MCLK_PER_PIXEL;
            self.timer_deadline = Some(self.time + period);
        }
    }

    /// Acknowledge interrupts ($3C000C). Bits: 1=irq3, 2=raster, 4=vblank.
    pub fn acknowledge(&mut self, d: u16) {
        if (d & 1) != 0 {
            self.irq3 = false;
        }
        if (d & 2) != 0 {
            self.raster = false;
        }
        if (d & 4) != 0 {
            self.vblank = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: run advance() until to_mclk, consuming all line-start returns.
    fn run_to(lspc: &mut Lspc, to_mclk: u64) {
        while lspc.advance(to_mclk).is_some() {}
    }

    #[test]
    fn irq3_at_reset() {
        let mut lspc = Lspc::new();
        assert_eq!(lspc.irq_level(), 3, "IRQ3 should be pending at reset");
        lspc.acknowledge(1);
        assert_eq!(lspc.irq_level(), 0, "After acking IRQ3, level should be 0");
    }

    #[test]
    fn vblank_irq_timing() {
        let mut lspc = Lspc::new();
        lspc.acknowledge(1); // Clear IRQ3 from reset
                             // Before vblank IRQ time (240*1536+58)
        run_to(&mut lspc, VBSTART as u64 * MCLK_PER_LINE + 57);
        assert!(!lspc.vblank, "vblank should not be set before IRQ time");

        // At vblank IRQ time
        run_to(
            &mut lspc,
            VBSTART as u64 * MCLK_PER_LINE + VBLANK_IRQ_OFFSET,
        );
        assert_eq!(
            lspc.irq_level(),
            1,
            "IRQ level should be 1 (vblank) after vblank IRQ"
        );

        // Acknowledge and verify it clears
        lspc.acknowledge(4);
        assert_eq!(
            lspc.irq_level(),
            0,
            "After acking vblank, level should be 0"
        );

        // Next frame's vblank IRQ (periodicity)
        run_to(
            &mut lspc,
            VBSTART as u64 * MCLK_PER_LINE + VBLANK_IRQ_OFFSET + LINES as u64 * MCLK_PER_LINE,
        );
        assert_eq!(
            lspc.irq_level(),
            1,
            "vblank IRQ should fire again next frame"
        );
    }

    #[test]
    fn display_timer_relative() {
        let mut lspc = Lspc::new();
        lspc.acknowledge(1); // Clear IRQ3 from reset
                             // Enable relative timer (bit 5), enable raster IRQ (bit 4)
        lspc.irq2_ctrl = 0x30;
        // Set display counter to 99, arm at time=1000
        run_to(&mut lspc, 1000);
        lspc.set_timer_low(99);

        // Timer should fire at 1000 + (99+1)*4 = 1400
        run_to(&mut lspc, 1399);
        assert!(!lspc.raster, "raster IRQ should not fire before deadline");

        run_to(&mut lspc, 1400);
        assert_eq!(
            lspc.irq_level(),
            2,
            "IRQ level should be 2 (raster) after timer fires"
        );

        // Without bit 7 (autoload repeat), timer should not re-arm
        let deadline_after = lspc.timer_deadline;
        assert!(
            deadline_after.is_none(),
            "Timer should not repeat without bit 7"
        );
    }

    #[test]
    fn display_timer_autoload_repeat() {
        let mut lspc = Lspc::new();
        lspc.acknowledge(1); // Clear IRQ3 from reset
                             // Enable relative timer (bit 5), enable raster IRQ (bit 4), autoload repeat (bit 7)
        lspc.irq2_ctrl = 0xB0;
        run_to(&mut lspc, 1000);
        lspc.set_timer_low(99);

        // First fire at 1400
        run_to(&mut lspc, 1400);
        assert_eq!(lspc.irq_level(), 2, "First raster IRQ should fire");

        // Acknowledge and wait for second fire (should be at 1800)
        lspc.acknowledge(2);
        run_to(&mut lspc, 1799);
        assert!(!lspc.raster, "Second raster IRQ should not fire yet");

        run_to(&mut lspc, 1800);
        assert_eq!(
            lspc.irq_level(),
            2,
            "Second raster IRQ should fire (autoload repeat)"
        );
    }

    #[test]
    fn autoload_vblank() {
        let mut lspc = Lspc::new();
        // Enable autoload vblank (bit 6), enable raster IRQ (bit 4)
        lspc.irq2_ctrl = 0x50;
        lspc.display_counter = 9999;

        // At reload time (240*1536+1146), timer should be armed for +40000 mclk
        let reload_time = VBSTART as u64 * MCLK_PER_LINE + RELOAD_OFFSET;
        run_to(&mut lspc, reload_time);

        assert!(
            lspc.timer_deadline.is_some(),
            "Timer should be armed by autoload vblank"
        );
        let expected = reload_time + (9999 + 1) as u64 * MCLK_PER_PIXEL;
        assert_eq!(
            lspc.timer_deadline,
            Some(expected),
            "Timer deadline should match display_counter+1 period"
        );
    }

    #[test]
    fn line_start_events() {
        let mut lspc = Lspc::new();

        // First advance returns Some(0) at time=0
        assert_eq!(lspc.advance(0), Some(0));

        // Next line start at 1536 mclk
        assert_eq!(lspc.advance(MCLK_PER_LINE), Some(1));

        // Advance to just before line 263, then check the event
        run_to(&mut lspc, (LINES as u64 - 2) * MCLK_PER_LINE);
        assert_eq!(lspc.advance((LINES as u64 - 1) * MCLK_PER_LINE), Some(263));

        // Wrap to line 0 at 264*1536
        let frame_mclk = LINES as u64 * MCLK_PER_LINE;
        assert_eq!(lspc.advance(frame_mclk), Some(0));
    }

    #[test]
    fn auto_animation_timing() {
        let mut lspc = Lspc::new();
        // Speed 0 means increment every tick (frame_counter reloads to 0 each time)
        lspc.anim_speed = 0;

        // First animation tick at VSSTART line (256*1536)
        run_to(&mut lspc, VSSTART as u64 * MCLK_PER_LINE);
        assert_eq!(
            lspc.anim_counter, 1,
            "anim_counter should be 1 after first tick"
        );

        // Second tick next frame
        let frame_mclk = LINES as u64 * MCLK_PER_LINE;
        run_to(&mut lspc, VSSTART as u64 * MCLK_PER_LINE + frame_mclk);
        assert_eq!(
            lspc.anim_counter, 2,
            "anim_counter should be 2 after second tick"
        );
    }

    #[test]
    fn time_advances_correctly() {
        let mut lspc = Lspc::new();
        run_to(&mut lspc, 1000);
        assert_eq!(lspc.time(), 1000);

        run_to(&mut lspc, 2000);
        assert_eq!(lspc.time(), 2000);
    }
}
