// T403 — ng-lspc : couche fix (une ligne), banking Garou/KOF2000
// Port 1:1 de draw_fixed_layer() de MAME neogeo_spr.cpp

pub enum FixBankType {
    Std,
    Garou,
    Kof2000,
}

/// Dessine une ligne de la couche fix (background fixe).
/// vpos ∈ [16, 239] — position verticale dans l'écran visible.
pub fn draw_fixed_line(
    vram: &[u16],
    gfx: &[u8],
    from_cart: bool,
    bank: FixBankType,
    pens: &[u32],
    vpos: u16,
    out: &mut [u32; 320],
) {
    let addr_mask = (gfx.len() - 1) as u32;
    let banked = from_cart && addr_mask > 0x1FFFF;

    // Index dans VRAM pour la rangée de tuiles de cette ligne
    let mut video_data = 0x7000 | (vpos >> 3);

    // Table de banking Garou : construite une fois par ligne
    let mut garouoffsets: [u8; 34] = [0; 34];
    if banked && matches!(bank, FixBankType::Garou) {
        let mut garoubank = 0u8;
        let mut k = 0usize;
        let mut y = 0usize;
        while y < 32 {
            if vram[0x7500 + k] == 0x0200 && (vram[0x7580 + k] & 0xFF00) == 0xFF00 {
                garoubank = (vram[0x7580 + k] & 3) as u8;
                garouoffsets[y] = garoubank;
                y += 1;
            }
            garouoffsets[y] = garoubank;
            y += 1;
            k += 2;
        }
    }

    // Dessiner les 40 colonnes de tuiles (8 pixels chacune)
    let mut px = 0usize;
    for x in 0..40 {
        let cp = vram[video_data as usize];
        let mut code = cp & 0xFFF;
        let palette = (cp >> 12) as usize;

        // Banking : ajuster le code de tuile selon la banque
        if banked {
            match bank {
                FixBankType::Garou => {
                    let y = vpos >> 3;
                    let idx = ((y as i32 - 2) & 31) as usize;
                    let bank_offset = garouoffsets[idx] ^ 3;
                    code += (bank_offset as u16) * 0x1000;
                }
                FixBankType::Kof2000 => {
                    let y = vpos >> 3;
                    let idx = (((y as i32 - 1) & 31) + 32 * (x / 6)) as usize;
                    let shift = (5 - (x % 6)) * 2;
                    let bank_offset = ((vram[0x7500usize + idx] >> shift) & 3) ^ 3;
                    code += bank_offset * 0x1000;
                }
                FixBankType::Std => {}
            }
        }

        // Calculer l'offset dans la ROM gfx
        let gfx_offset = (((code as u32) << 5) | (vpos & 7) as u32) & addr_mask;
        let char_pens = &pens[palette * 16..];

        // Dessiner les 8 pixels de la tuile (4 octets × 2 pixels)
        for off in [0x10u32, 0x18, 0x00, 0x08] {
            let d = gfx[(gfx_offset + off) as usize];
            if d & 0x0F != 0 {
                out[px] = char_pens[d as usize & 0x0F];
            }
            px += 1;
            if d & 0xF0 != 0 {
                out[px] = char_pens[(d >> 4) as usize];
            }
            px += 1;
        }

        // Passer à la colonne suivante dans VRAM (pas de +1, pas de +2)
        video_data += 0x20;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pens() -> [u32; 0x4000] {
        let mut pens = [0u32; 0x4000];
        for (i, p) in pens.iter_mut().enumerate() {
            *p = i as u32;
        }
        pens
    }

    #[test]
    fn test_standard_tile_addressing() {
        // ROM gfx de 0x20000 octets, tuile 1 avec données séquentielles
        let mut gfx = vec![0u8; 0x20000];
        for i in 0..32 {
            gfx[32 + i] = i as u8;
        }

        // VRAM : code_and_palette = 0x1001 (palette 1, code 1) pour la rangée 2
        let mut vram = vec![0u16; 0x8800];
        vram[0x7000 + 2] = 0x1001;

        let pens = make_pens();
        let mut out = [0xDEADu32; 320];

        draw_fixed_line(&vram, &gfx, false, FixBankType::Std, &pens, 16, &mut out);

        // Tuile 1 à gfx_offset=32, ligne 0 (vpos&7=0)
        // Offsets lus : 32+0x10=48(16), 32+0x18=56(24), 32+0x00=32(0), 32+0x08=40(8)
        // Pixels attendus : [DEAD,17,24,17,DEAD,DEAD,24,DEAD]
        assert_eq!(out[0], 0xDEAD); // d=16: low nibble 0 → transparent
        assert_eq!(out[1], 17); // d=16: high nibble 1 → pens[17]
        assert_eq!(out[2], 24); // d=24: low nibble 8 → pens[24]
        assert_eq!(out[3], 17); // d=24: high nibble 1 → pens[17]
        assert_eq!(out[4], 0xDEAD); // d=0: transparent
        assert_eq!(out[5], 0xDEAD); // d=0: transparent
        assert_eq!(out[6], 24); // d=8: low nibble 8 → pens[24]
        assert_eq!(out[7], 0xDEAD); // d=8: high nibble 0 → transparent

        // Les autres colonnes doivent être transparentes (code 0)
        for (i, &p) in out.iter().enumerate().skip(8) {
            assert_eq!(p, 0xDEAD, "pixel {} should be transparent", i);
        }
    }

    #[test]
    fn test_transparent_pixel_unchanged() {
        let gfx = vec![0u8; 0x20000];
        let vram = vec![0u16; 0x8800]; // code 0 → tuile transparente
        let pens = make_pens();
        let mut out = [0xDEADu32; 320];

        draw_fixed_line(&vram, &gfx, false, FixBankType::Std, &pens, 16, &mut out);

        for (i, &p) in out.iter().enumerate() {
            assert_eq!(p, 0xDEAD, "pixel {} should be unchanged", i);
        }
    }

    #[test]
    fn test_column_vram_addressing() {
        // Vérifier que la colonne 1 lit vram[0x7000+0x20+...] (pas de +1)
        let mut gfx = vec![0u8; 0x20000];
        for i in 0..32 {
            gfx[32 + i] = i as u8; // tuile 1 à offset 32
        }
        for i in 0..32 {
            gfx[64 + i] = i as u8; // tuile 2 à offset 64
        }

        let mut vram = vec![0u16; 0x8800];
        vram[0x7000 + 2] = 0x1001; // colonne 0 : code 1
        vram[0x7000 + 0x20 + 2] = 0x1002; // colonne 1 : code 2

        let pens = make_pens();
        let mut out = [0xDEADu32; 320];

        draw_fixed_line(&vram, &gfx, false, FixBankType::Std, &pens, 16, &mut out);

        // Colonne 0 : tuile 1 (données séquentielles)
        assert_eq!(out[1], 17); // pens[17] de la tuile 1

        // Colonne 1 : tuile 2 (mêmes données, donc mêmes pixels)
        assert_eq!(out[9], 17); // pens[17] de la tuile 2
    }

    #[test]
    fn test_garou_banking() {
        // ROM gfx banked (0x40000 octets) : banques séparées par 64 Ko
        let mut gfx = vec![0u8; 0x40000];
        for i in 0..32 {
            gfx[32 + i] = i as u8; // tuile 1 dans banque 0 (offset 32)
        }
        // Tuile 1 dans banque 1 : code devient 0x1001 → gfx_offset = 0x20020
        for i in 0..32 {
            gfx[0x20000 + 32 + i] = (i + 1) as u8; // tuile 1 dans banque 1
        }

        let mut vram = vec![0u16; 0x8800];
        vram[0x7000 + 2] = 0x1001; // code 1, palette 1

        // Configurer le banking Garou : rangée 0 → banque 2
        vram[0x7500] = 0x0200;
        vram[0x7580] = 0xFF02;

        let pens = make_pens();
        let mut out = [0xDEADu32; 320];

        draw_fixed_line(&vram, &gfx, true, FixBankType::Garou, &pens, 16, &mut out);

        // Pour vpos=16 (rangée 2), (y-2)&31 = 0 → garouoffsets[0] = 2
        // code += 0x1000 * (2 ^ 3) = 0x1000 → code final = 0x1001
        // gfx_offset = ((0x1001 << 5) | 0) & 0x3FFFF = 0x20020
        // d = gfx[0x20020 + 0x10] = gfx[0x20030] = 17 (i=16, i+1=17)
        assert_eq!(out[1], 17); // pens[17] de la tuile bankée (d=17: high nibble 1 → pens[17])
    }

    #[test]
    fn test_non_banked_no_offset() {
        // ROM gfx non banked (0x20000 octets) : aucun décalage même si bank==Garou
        let mut gfx = vec![0u8; 0x20000];
        for i in 0..32 {
            gfx[32 + i] = i as u8;
        }

        let mut vram = vec![0u16; 0x8800];
        vram[0x7000 + 2] = 0x1001;
        vram[0x7500] = 0x0200;
        vram[0x7580] = 0xFF02;

        let pens = make_pens();
        let mut out = [0xDEADu32; 320];

        draw_fixed_line(&vram, &gfx, true, FixBankType::Garou, &pens, 16, &mut out);

        // Pas de banking → tuile standard à gfx_offset=32
        assert_eq!(out[1], 17); // pens[17] de la tuile non bankée
    }
}
