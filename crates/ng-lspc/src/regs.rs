//! Register access tests for LSPC ($3C0000-$3C000E).

#[cfg(test)]
mod tests {
    use crate::Lspc;

    #[test]
    fn reg_vram_write_read_offset() {
        let mut lspc = Lspc::new();
        // Set offset to 0x1234, modulo 1
        lspc.write_reg(0, 0x1234, true, true);
        lspc.write_reg(2, 1, true, true);
        // Write data at that offset
        lspc.write_reg(1, 0xBEEF, true, true);
        // Verify VRAM was written
        assert_eq!(lspc.vram.read_word(0x1234), 0xBEEF);
        // Offset should have advanced to 0x1235 (modulo 1)
        // Read back by setting offset to 0x1235 and reading data
        lspc.write_reg(0, 0x1235, true, true);
        let val = lspc.read_reg(1, true, true).unwrap();
        assert_eq!(val, lspc.vram.read_word(0x1235));
    }

    #[test]
    fn reg_fast_zone_bit15_preserved() {
        let mut lspc = Lspc::new();
        // Write to fast zone (bit 15 set)
        lspc.write_reg(0, 0x8001, true, true);
        lspc.write_reg(2, 1, true, true);
        lspc.write_reg(1, 0x1111, true, true);
        assert_eq!(lspc.vram.read_word(0x8001), 0x1111);
        // Offset should be 0x8002 (bit 15 preserved)
        lspc.write_reg(0, 0x8002, true, true);
        let val = lspc.read_reg(1, true, true).unwrap();
        assert_eq!(val, lspc.vram.read_word(0x8002));
    }

    #[test]
    fn reg_modulo_minus_one_wraps() {
        let mut lspc = Lspc::new();
        lspc.write_reg(0, 0x0000, true, true);
        lspc.write_reg(2, 0xFFFF, true, true); // -1 as i16
        lspc.write_reg(1, 0xAAAA, true, true);
        assert_eq!(lspc.vram.read_word(0x0000), 0xAAAA);
        // (0 + (-1)) & 0x7FFF = 0x7FFF
        lspc.write_reg(0, 0x7FFF, true, true);
        let val = lspc.read_reg(1, true, true).unwrap();
        assert_eq!(val, lspc.vram.read_word(0x7FFF));
    }

    #[test]
    fn reg_video_control_read_vpos0() {
        let mut lspc = Lspc::new();
        lspc.vpos = 0;
        lspc.anim_counter = 5;
        // v = 0 + 0x100 = 0x100; (0x100 << 7) | 5 = 0x8000 | 5 = 0x8005
        let val = lspc.read_reg(3, true, true).unwrap();
        assert_eq!(val, 0x8005);
    }

    #[test]
    fn reg_video_control_read_vpos263() {
        let mut lspc = Lspc::new();
        lspc.vpos = 263;
        lspc.anim_counter = 5;
        // v = 263 + 0x100 = 0x207 >= 0x200, so v -= 264 => 0x0FF
        // (0x0FF << 7) | 5 = 0x7F80 | 5 = 0x7F85
        let val = lspc.read_reg(3, true, true).unwrap();
        assert_eq!(val, 0x7F85);
    }

    #[test]
    fn reg_upper_byte_only_write() {
        let mut lspc = Lspc::new();
        // Upper byte only: data is duplicated to both bytes
        lspc.write_reg(0, 0xAB00, true, false);
        // Expected offset: 0xABAB & 0x87FF (bit 15 set) = 0x83AB
        assert_eq!(lspc.vram.offset, 0x83AB);
    }

    #[test]
    fn reg_lower_byte_only_write_ignored() {
        let mut lspc = Lspc::new();
        // Lower byte only writes are ignored
        lspc.write_reg(0, 0x00FF, false, true);
        assert_eq!(lspc.vram.offset, 0); // unchanged
    }

    #[test]
    fn reg_lower_byte_only_read_unmapped() {
        let mut lspc = Lspc::new();
        let val = lspc.read_reg(0, false, true);
        assert_eq!(val, None);
    }

    #[test]
    fn reg_video_control_write_anim_speed() {
        let mut lspc = Lspc::new();
        // anim_speed = data >> 8
        lspc.write_reg(3, 0x0500, true, true);
        assert_eq!(lspc.anim_speed, 5);
    }

    #[test]
    fn reg_video_control_write_anim_disabled() {
        let mut lspc = Lspc::new();
        // anim_disabled = bit 3
        lspc.write_reg(3, 0x0008, true, true);
        assert!(lspc.anim_disabled);
    }

    #[test]
    fn reg_video_control_write_irq2_ctrl() {
        let mut lspc = Lspc::new();
        // irq2_ctrl = data & 0x00F0
        lspc.write_reg(3, 0x00A5, true, true);
        assert_eq!(lspc.irq2_ctrl, 0xA0);
    }

    #[test]
    fn reg_timer_high_write() {
        let mut lspc = Lspc::new();
        lspc.write_reg(4, 0x1234, true, true);
        assert_eq!(lspc.display_counter & 0xFFFF0000, 0x12340000);
    }

    #[test]
    fn reg_timer_low_write() {
        let mut lspc = Lspc::new();
        lspc.write_reg(5, 0x5678, true, true);
        assert_eq!(lspc.display_counter & 0xFFFF, 0x5678);
    }

    #[test]
    fn reg_auto_animation_tick_speed2() {
        let mut lspc = Lspc::new();
        lspc.anim_speed = 2;
        // First tick: frame_counter 0 -> reload to 2, counter 1
        lspc.auto_animation_tick();
        assert_eq!(lspc.anim_frame_counter, 2);
        assert_eq!(lspc.anim_counter, 1);
        // Second tick: fc 2->1
        lspc.auto_animation_tick();
        assert_eq!(lspc.anim_frame_counter, 1);
        assert_eq!(lspc.anim_counter, 1);
        // Third tick: fc 1->0
        lspc.auto_animation_tick();
        assert_eq!(lspc.anim_frame_counter, 0);
        assert_eq!(lspc.anim_counter, 1);
        // Fourth tick: fc 0 -> reload to 2, counter 2
        lspc.auto_animation_tick();
        assert_eq!(lspc.anim_frame_counter, 2);
        assert_eq!(lspc.anim_counter, 2);
    }

    #[test]
    fn reg_modulo_read() {
        let mut lspc = Lspc::new();
        lspc.write_reg(2, 0x1234, true, true);
        let val = lspc.read_reg(2, true, true).unwrap();
        assert_eq!(val, 0x1234);
    }

    #[test]
    fn reg_default_case_returns_read_buffer() {
        let mut lspc = Lspc::new();
        // Write some data to VRAM at offset 0
        lspc.write_reg(0, 0, true, true);
        lspc.write_reg(1, 0xCAFE, true, true);
        // Read register 4 (default case) should return read_buffer
        let val = lspc.read_reg(4, true, true).unwrap();
        assert_eq!(val, 0xCAFE);
    }
}
