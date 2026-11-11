//! Filtres vidéo logiciels : interpolation horizontale (noyau gaussien à 4 points et 64 phases),
//! scanlines (standard ou adaptatives), gamma, masque de phosphores / teinte. Tout est calculé à la
//! taille finale de l'image, après le facteur d'agrandissement.

use eframe::egui::Color32;

use crate::display::{FB_H, FB_W};

/// Scanlines.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScanMode {
    #[default]
    Off,
    /// Même profondeur sur toute l'image.
    Standard,
    /// Profondeur variable selon la luminosité du pixel.
    Adaptive,
}

/// Masque d'écran.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mask {
    #[default]
    None,
    /// Grille d'ouverture type Sony Trinitron.
    Trinitron,
    /// Tube fané : teinte jaunâtre.
    FadedYellow,
}

/// Réglages des filtres (tous les pourcentages sont des entiers).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fx {
    /// Netteté de l'interpolation horizontale en % (0 = aucune, pixels nets).
    pub interp: u8,
    pub scan_mode: ScanMode,
    /// Profondeur des scanlines standard, en % (0 à 95).
    pub scan: u8,
    /// Profondeur adaptative sur les pixels sombres, en %.
    pub scan_dk: u8,
    /// Profondeur adaptative sur les pixels clairs, en %.
    pub scan_br: u8,
    /// Gamma en centièmes (100 = désactivé, 145 = 1,45).
    pub gamma: u8,
    pub mask: Mask,
    /// Force du masque en %.
    pub mask_pct: u8,
}

impl Default for Fx {
    fn default() -> Self {
        Fx {
            interp: 0,
            scan_mode: ScanMode::Off,
            scan: 50,
            scan_dk: 40,
            scan_br: 80,
            gamma: 100,
            mask: Mask::None,
            mask_pct: 100,
        }
    }
}

/// Préréglages d'écran.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    None,
    SonyTrinitron,
    FadedTube,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::None, Preset::SonyTrinitron, Preset::FadedTube];

    pub fn label(self) -> &'static str {
        match self {
            Preset::None => "Aucun",
            Preset::SonyTrinitron => "Sony Trinitron",
            Preset::FadedTube => "Tube fané (Faded Tube)",
        }
    }
}

/// Netteté des trois interpolations proposées.
pub const SOFT: u8 = 55;
pub const MEDIUM: u8 = 70;
pub const SHARP: u8 = 85;

impl Fx {
    pub fn preset(p: Preset) -> Fx {
        let base = Fx::default();
        match p {
            Preset::None => base,
            Preset::SonyTrinitron => Fx {
                interp: 35,
                scan_mode: ScanMode::Adaptive,
                scan_dk: 40,
                scan_br: 80,
                gamma: 110,
                mask: Mask::Trinitron,
                ..base
            },
            Preset::FadedTube => Fx {
                interp: 15,
                scan_mode: ScanMode::Adaptive,
                scan_dk: 20,
                scan_br: 60,
                gamma: 145,
                mask: Mask::FadedYellow,
                ..base
            },
        }
    }

    /// Préréglage correspondant exactement aux réglages, s'il y en a un.
    pub fn detect(&self) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| {
            let q = Fx::preset(*p);
            // les curseurs inutilisés ne comptent pas
            self.norm() == q.norm()
        })
    }

    /// Réglages avec les valeurs sans effet ramenées à leur défaut (pour comparer).
    fn norm(&self) -> Fx {
        let d = Fx::default();
        let mut f = *self;
        if f.scan_mode != ScanMode::Standard {
            f.scan = d.scan;
        }
        if f.scan_mode != ScanMode::Adaptive {
            f.scan_dk = d.scan_dk;
            f.scan_br = d.scan_br;
        }
        if f.mask == Mask::None {
            f.mask_pct = d.mask_pct;
        }
        f
    }

    /// Au moins un filtre agit-il sur l'image ?
    pub fn is_active(&self) -> bool {
        self.interp > 0
            || self.scan_mode != ScanMode::Off
            || self.gamma != 100
            || self.mask != Mask::None
    }
}

/// Poids (somme 128) du noyau gaussien d'écart-type `s` pour les 4 points voisins, par phase.
fn kernel(sharp: u8) -> [[i32; 4]; 64] {
    // Netteté 70 -> s = 0,34 ; 85 -> 0,22 ; 55 -> 0,46.
    let s = (0.9 - 0.008 * f32::from(sharp)).max(0.1);
    let mut out = [[0i32; 4]; 64];
    for (p, row) in out.iter_mut().enumerate() {
        let t = p as f32 / 64.0;
        let w: Vec<f32> = (0..4)
            .map(|j| {
                let d = (j as f32 - 1.0) - t;
                (-d * d / (2.0 * s * s)).exp()
            })
            .collect();
        let sum: f32 = w.iter().sum();
        let mut tot = 0;
        for j in 0..4 {
            row[j] = (w[j] / sum * 128.0).round() as i32;
            tot += row[j];
        }
        // corrige l'arrondi sur le plus gros poids
        let big = (0..4).max_by_key(|&j| row[j]).unwrap_or(1);
        row[big] += 128 - tot;
    }
    out
}

fn smoothstep(e: f32) -> f32 {
    e * e * (3.0 - 2.0 * e)
}

/// Atténuation (1 = rien) du masque Trinitron en (x, y), selon la hauteur de l'image.
fn trinitron_cell(x: usize, y: usize, h: usize) -> f32 {
    // 70f -> 1, 72a -> 0,85, 00c -> 0,7
    const A: f32 = 1.0;
    const B: f32 = 0.85;
    const C: f32 = 0.7;
    const P720: [[f32; 3]; 2] = [[A, A, C], [B, C, C]];
    const P1080: [[f32; 3]; 5] = [[A, A, C], [A, A, C], [A, A, C], [A, B, C], [B, C, C]];
    const P1440: [[f32; 3]; 6] = [
        [A, A, C],
        [A, A, C],
        [A, A, C],
        [A, A, C],
        [A, B, C],
        [B, C, C],
    ];
    if h < 900 {
        P720[y % 2][x % 3]
    } else if h < 1300 {
        P1080[y % 5][x % 3]
    } else {
        P1440[y % 6][x % 3]
    }
}

/// Applique les filtres : `src` = image 320x224 (0x00RRGGBB), sortie `w` x `h`.
pub fn render(src: &[u32], fx: &Fx, w: usize, h: usize) -> Vec<Color32> {
    let (sw, sh) = (FB_W as usize, FB_H as usize);
    assert!(src.len() >= sw * sh && w > 0 && h > 0);
    let px = |x: usize, y: usize| -> [i32; 3] {
        let p = src[y * sw + x];
        [
            ((p >> 16) & 255) as i32,
            ((p >> 8) & 255) as i32,
            (p & 255) as i32,
        ]
    };

    // 1. horizontal : une rangée filtrée par ligne source
    let mut hrows = vec![[0u8; 3]; sh * w];
    if fx.interp == 0 {
        for y in 0..sh {
            for x in 0..w {
                let c = px((x * sw / w).min(sw - 1), y);
                hrows[y * w + x] = [c[0] as u8, c[1] as u8, c[2] as u8];
            }
        }
    } else {
        let k = kernel(fx.interp);
        // (index du premier point, phase) par colonne de sortie
        let cols: Vec<(i32, usize)> = (0..w)
            .map(|x| {
                let pos = (x as f32 + 0.5) * sw as f32 / w as f32 - 0.5;
                let i = pos.floor();
                (i as i32, (((pos - i) * 64.0) as usize).min(63))
            })
            .collect();
        for y in 0..sh {
            for (x, &(i, ph)) in cols.iter().enumerate() {
                let mut acc = [64i32; 3];
                for (j, wgt) in k[ph].iter().enumerate() {
                    let sx = (i - 1 + j as i32).clamp(0, sw as i32 - 1) as usize;
                    let c = px(sx, y);
                    for ch in 0..3 {
                        acc[ch] += c[ch] * wgt;
                    }
                }
                hrows[y * w + x] = [
                    (acc[0] >> 7).clamp(0, 255) as u8,
                    (acc[1] >> 7).clamp(0, 255) as u8,
                    (acc[2] >> 7).clamp(0, 255) as u8,
                ];
            }
        }
    }

    // 2. gamma
    let g = f32::from(fx.gamma) / 100.0;
    let lut: Vec<f32> = (0..256)
        .map(|v| {
            if fx.gamma == 100 {
                v as f32
            } else {
                255.0 * (v as f32 / 255.0).powf(g)
            }
        })
        .collect();

    // 3. teinte du masque
    let mp = f32::from(fx.mask_pct) / 100.0;
    let tint = if fx.mask == Mask::FadedYellow {
        [1.0, 1.0 - 0.04 * mp, 1.0 - 0.20 * mp]
    } else {
        [1.0; 3]
    };

    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        // 4. vertical : ligne source et profondeur de scanline
        let (row, sm) = if fx.scan_mode == ScanMode::Off {
            (((y * sh) / h).min(sh - 1), 0.0)
        } else {
            let pos = y as f32 * sh as f32 / h as f32;
            let fl = pos.floor();
            let t = pos - fl;
            let row = if t <= 0.5 {
                fl as usize
            } else {
                fl as usize + 1
            };
            (row.min(sh - 1), smoothstep(2.0 * t.min(1.0 - t)))
        };
        let line = &hrows[row * w..(row + 1) * w];
        for (x, p) in line.iter().enumerate() {
            let depth = match fx.scan_mode {
                ScanMode::Off => 0.0,
                ScanMode::Standard => f32::from(fx.scan) / 100.0,
                ScanMode::Adaptive => {
                    let l = (0.299 * f32::from(p[0])
                        + 0.587 * f32::from(p[1])
                        + 0.114 * f32::from(p[2]))
                        / 255.0;
                    let (dk, br) = (f32::from(fx.scan_dk), f32::from(fx.scan_br));
                    (dk + (br - dk) * l) / 100.0
                }
            };
            let mut gain = 1.0 - depth * sm;
            if fx.mask == Mask::Trinitron {
                gain *= 1.0 - mp * (1.0 - trinitron_cell(x, y, h));
            }
            let c = |ch: usize| -> u8 {
                (lut[p[ch] as usize] * gain * tint[ch] + 0.5).clamp(0.0, 255.0) as u8
            };
            out.push(Color32::from_rgb(c(0), c(1), c(2)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: u32) -> Vec<u32> {
        vec![v; (FB_W * FB_H) as usize]
    }

    #[test]
    fn inactive_by_default() {
        assert!(!Fx::default().is_active());
        assert!(Fx {
            interp: 10,
            ..Fx::default()
        }
        .is_active());
    }

    #[test]
    fn identity_without_filters() {
        let mut src = flat(0);
        src[5] = 0x102030;
        let o = render(&src, &Fx::default(), FB_W as usize, FB_H as usize);
        assert_eq!(o[5], Color32::from_rgb(0x10, 0x20, 0x30));
        assert_eq!(o[6], Color32::from_rgb(0, 0, 0));
    }

    #[test]
    fn kernels_sum_to_128() {
        for s in [15u8, 35, 55, 70, 85] {
            for row in kernel(s) {
                assert_eq!(row.iter().sum::<i32>(), 128);
            }
        }
        // valeurs connues du noyau « moyen » (netteté 70)
        let k = kernel(MEDIUM);
        assert_eq!(k[0], [2, 124, 2, 0]);
        assert_eq!(k[32], [0, 64, 64, 0]);
    }

    #[test]
    fn interpolation_keeps_flat_colors() {
        let src = flat(0x808080);
        let fx = Fx {
            interp: SOFT,
            ..Fx::default()
        };
        for c in render(&src, &fx, 640, 448) {
            assert_eq!(c, Color32::from_rgb(128, 128, 128));
        }
    }

    #[test]
    fn standard_scanlines_alternate_at_x2() {
        let src = flat(0xFFFFFF);
        let fx = Fx {
            scan_mode: ScanMode::Standard,
            scan: 50,
            ..Fx::default()
        };
        let o = render(&src, &fx, 320, 448);
        assert_eq!(o[0], Color32::from_rgb(255, 255, 255));
        assert_eq!(o[320], Color32::from_rgb(128, 128, 128));
        assert_eq!(o[640], Color32::from_rgb(255, 255, 255));
    }

    #[test]
    fn adaptive_scanlines_depend_on_brightness() {
        let fx = Fx {
            scan_mode: ScanMode::Adaptive,
            scan_dk: 40,
            scan_br: 80,
            ..Fx::default()
        };
        let dark = render(&flat(0x000000), &fx, 320, 448);
        assert_eq!(dark[320], Color32::from_rgb(0, 0, 0));
        let bright = render(&flat(0xFFFFFF), &fx, 320, 448);
        assert_eq!(bright[320].r(), 51); // 255 * (1 - 0.8)
        let grey = render(&flat(0x808080), &fx, 320, 448);
        assert!(grey[320].r() < 128 && grey[320].r() > 50);
    }

    #[test]
    fn gamma_and_tint() {
        let fx = Fx {
            gamma: 145,
            ..Fx::default()
        };
        let o = render(&flat(0x808080), &fx, 320, 224);
        // 255 * (128/255)^1.45 ~ 94
        assert_eq!(o[0].r(), 94);
        let fx = Fx {
            mask: Mask::FadedYellow,
            ..Fx::default()
        };
        let o = render(&flat(0xFFFFFF), &fx, 320, 224);
        assert!(o[0].b() < o[0].r());
    }

    #[test]
    fn presets_round_trip() {
        for p in Preset::ALL {
            assert_eq!(Fx::preset(p).detect(), Some(p));
        }
        assert_eq!(
            Fx {
                gamma: 123,
                ..Fx::default()
            }
            .detect(),
            None
        );
    }
}
