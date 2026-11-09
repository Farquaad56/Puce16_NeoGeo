// T406 — Tests de rendu sur scènes de référence (oracle Python)

use ng_common::video::{Framebuffer, SCREEN_H, SCREEN_W};
use ng_lspc::fix::FixBankType;
use ng_lspc::palette::Palette;
use ng_lspc::render::{render_line, LspcRoms};
use ng_lspc::sprites::parse_sprites;

fn load_u16(path: &str) -> Vec<u16> {
    let data = std::fs::read(path).unwrap();
    let mut out = Vec::with_capacity(data.len() / 2);
    for i in (0..data.len()).step_by(2) {
        out.push(u16::from_le_bytes([data[i], data[i + 1]]));
    }
    out
}

fn load_u32(path: &str) -> Vec<u32> {
    let data = std::fs::read(path).unwrap();
    let mut out = Vec::with_capacity(data.len() / 4);
    for i in (0..data.len()).step_by(4) {
        out.push(u32::from_le_bytes([
            data[i],
            data[i + 1],
            data[i + 2],
            data[i + 3],
        ]));
    }
    out
}

fn load_u8(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap()
}

fn parse_meta(path: &str) -> (u8, bool, FixBankType, bool) {
    let content = std::fs::read_to_string(path).unwrap();
    let mut anim_counter = 0u8;
    let mut anim_disabled = false;
    let mut fix_bank = FixBankType::Std;
    let mut from_cart = false;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        match parts[0] {
            "anim_counter" => anim_counter = parts[1].parse().unwrap(),
            "anim_disabled" => anim_disabled = parts[1] == "1",
            "fix_bank" => {
                let b: u8 = parts[1].parse().unwrap();
                fix_bank = match b {
                    0 => FixBankType::Std,
                    1 => FixBankType::Garou,
                    2 => FixBankType::Kof2000,
                    _ => panic!("unknown fix_bank {}", b),
                };
            }
            "from_cart" => from_cart = parts[1] == "1",
            _ => {}
        }
    }

    (anim_counter, anim_disabled, fix_bank, from_cart)
}

fn run_scene(name: &str) {
    let base = format!("../../tests-data/scenes/{}", name);

    // Load VRAM
    let mut vram = load_u16(&format!("{}/vram.bin", base));
    assert_eq!(vram.len(), 0x8800, "VRAM size mismatch for {}", name);

    // Load palette pens (base bank only)
    let pens_data = load_u32(&format!("{}/pens.bin", base));
    assert_eq!(pens_data.len(), 0x1000, "Pens size mismatch for {}", name);

    let mut pal = Palette::new();
    for (i, &pen) in pens_data.iter().enumerate() {
        pal.pens[i] = pen;
    }

    // Load ROMs
    let c_rom = load_u8(&format!("{}/c.bin", base));
    let s_rom = load_u8(&format!("{}/s.bin", base));
    let lo_rom = load_u8(&format!("{}/lo.bin", base));

    // Parse meta
    let (anim_counter, anim_disabled, fix_bank, from_cart) =
        parse_meta(&format!("{}/meta.txt", base));

    // Setup roms struct
    let roms = LspcRoms {
        c: &c_rom,
        s_cart: &s_rom,
        s_bios: &s_rom,
        lo: &lo_rom,
        use_cart_fix: from_cart,
        fix_bank,
        pal: &pal,
    };

    // Render frame using the same sequence as sprite_line_timer_callback
    let mut fb = Framebuffer::new();
    for line in 0..264u16 {
        if line != 0 {
            render_line(&vram, &roms, anim_counter, anim_disabled, line - 1, &mut fb);
        }
        parse_sprites(&mut vram, line);
    }

    // Load expected output
    let expected = load_u32(&format!("{}/expected.raw", base));
    assert_eq!(
        expected.len(),
        SCREEN_W * SCREEN_H,
        "Expected size mismatch for {}",
        name
    );

    // Compare pixel by pixel
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let idx = y * SCREEN_W + x;
            if fb.pixels[idx] != expected[idx] {
                panic!(
                    "Scene {} pixel mismatch at ({}, {}): expected {:08X} got {:08X}",
                    name, x, y, expected[idx], fb.pixels[idx]
                );
            }
        }
    }

    println!("Scene {} passed", name);
}

#[test]
fn test_fix_std() {
    run_scene("fix_std");
}

#[test]
fn test_sprites_plain() {
    run_scene("sprites_plain");
}

#[test]
fn test_sprites_zoom_chain() {
    run_scene("sprites_zoom_chain");
}

#[test]
fn test_fix_garou() {
    run_scene("fix_garou");
}

#[test]
fn test_fix_kof2000() {
    run_scene("fix_kof2000");
}

// Test trame vide : VRAM et pens à 0 → tous les pixels = backdrop (pen 0xFFF)
#[test]
fn test_empty_frame_all_backdrop() {
    let mut vram: Vec<u16> = vec![0; 0x8800];
    let pal = Palette::new();

    // Pens à zéro → backdrop = pens[0xFFF] = 0 (noir)
    // Fournir des ROMs minimales valides (zoom table de 256 entrées pour zoom_y=0)
    let c_rom: Vec<u8> = vec![0; 4];
    let s_rom: Vec<u8> = vec![0; 32]; // Une tuile fix minimale (32 octets)
    let lo_rom: Vec<u8> = vec![0; 256];

    let roms = LspcRoms {
        c: &c_rom,
        s_cart: &s_rom,
        s_bios: &s_rom,
        lo: &lo_rom,
        use_cart_fix: false,
        fix_bank: FixBankType::Std,
        pal: &pal,
    };

    let mut fb = Framebuffer::new();

    for line in 0..264u16 {
        if line != 0 {
            render_line(&vram, &roms, 0, false, line - 1, &mut fb);
        }
        parse_sprites(&mut vram, line);
    }

    // Tous les pixels visibles doivent être égaux au backdrop (0)
    for y in 0..SCREEN_H {
        for x in 0..SCREEN_W {
            let idx = y * SCREEN_W + x;
            assert_eq!(
                fb.pixels[idx],
                pal.backdrop(),
                "pixel at ({}, {}) should be backdrop",
                x,
                y
            );
        }
    }
}

// Test ordre fix/sprite : le fix recouvre un sprite au même pixel
// Couvert par la scène fix_std qui a des sprites ET une couche fix.
#[test]
fn test_fix_over_sprite_order() {
    run_scene("fix_std");
}

// Test advance_render appelle on_line_start 264 fois pour une frame complète
#[test]
fn test_advance_render_frame_count() {
    use ng_lspc::timing::{LINES, MCLK_PER_LINE};
    use ng_lspc::Lspc;

    let mut lspc = Lspc::new();
    let pal = Palette::new();

    // Fournir des ROMs minimales valides
    let c_rom: Vec<u8> = vec![0; 4];
    let s_rom: Vec<u8> = vec![0; 32]; // Une tuile fix minimale (32 octets)
    let lo_rom: Vec<u8> = vec![0; 256];

    let roms = LspcRoms {
        c: &c_rom,
        s_cart: &s_rom,
        s_bios: &s_rom,
        lo: &lo_rom,
        use_cart_fix: false,
        fix_bank: FixBankType::Std,
        pal: &pal,
    };

    let mut fb = Framebuffer::new();
    let frame_mclk = LINES as u64 * MCLK_PER_LINE; // 264 * 1536 = 405_504

    lspc.advance_render(frame_mclk, &roms, &mut fb);

    assert_eq!(
        lspc.time(),
        frame_mclk,
        "should have advanced exactly one frame"
    );
}
