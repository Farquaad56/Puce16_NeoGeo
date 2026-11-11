//! Filtres vidéo logiciels, calculés à la taille finale de l'image (après le facteur
//! d'agrandissement) : interpolation horizontale, scanlines (standard ou adaptatives), gamma et
//! masque d'écran.
//!
//! Les coefficients viennent des fichiers de filtres de la communauté MiSTer, regroupés dans
//! `data/fx.bin` par `tools/gen_fx_data.py` ; les masques sont dans `data/masks/`.

use std::sync::OnceLock;

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
    /// Netteté de l'interpolation horizontale en % (0 = aucune ; sinon multiple de 5, 5 à 100).
    pub interp: u8,
    pub scan_mode: ScanMode,
    /// Profondeur des scanlines standard, en % (0 à 95, pas de 5).
    pub scan: u8,
    /// Scanlines adaptatives : luminosité conservée au milieu de la ligne pour les pixels sombres,
    /// en % (multiple de 10, au plus `scan_br`).
    pub scan_dk: u8,
    /// Idem pour les pixels clairs, en % (multiple de 10).
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

/// Tables de coefficients (4 par phase, somme 128 pour les tables à 64 phases, 256 pour SLA).
fn data() -> &'static [i16] {
    static D: OnceLock<Vec<i16>> = OnceLock::new();
    D.get_or_init(|| {
        include_bytes!("../data/fx.bin")
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect()
    })
}

const N_SHARP: usize = 21;
const N_STD: usize = 20;
const STD_OFF: usize = N_SHARP * 256;
const DARK_OFF: usize = STD_OFF + N_STD * 256;
const BRIGHT_OFF: usize = DARK_OFF + 11 * 1024;

/// Table (64 phases) du filtre d'interpolation de netteté `sharp` (%).
fn sharp_table(sharp: u8) -> &'static [i16] {
    let i = ((usize::from(sharp) + 2) / 5).min(N_SHARP - 1);
    &data()[i * 256..(i + 1) * 256]
}

/// Table (64 phases) des scanlines standard de profondeur `depth` (%).
fn std_table(depth: u8) -> &'static [i16] {
    let i = (usize::from(depth.min(95)) / 5).min(N_STD - 1);
    &data()[STD_OFF + i * 256..STD_OFF + (i + 1) * 256]
}

/// Tables (256 phases) sombre et claire des scanlines adaptatives.
fn adaptive_tables(dk: u8, br: u8) -> (&'static [i16], &'static [i16]) {
    let dk10 = ((usize::from(dk) + 5) / 10).min(10);
    let br10 = ((usize::from(br) + 5) / 10).min(10).max(dk10);
    let dark = DARK_OFF + dk10 * 1024;
    let before: usize = (0..dk10).map(|d| 11 - d).sum();
    let bright = BRIGHT_OFF + (before + br10 - dk10) * 1024;
    (&data()[dark..dark + 1024], &data()[bright..bright + 1024])
}

/// Motif de masque : taille et gain RVB de chaque cellule.
struct Pattern {
    res: usize,
    w: usize,
    h: usize,
    cells: Vec<[f32; 3]>,
}

/// Lit un fichier de masque (format « v2 » : une cellule = couleur, intensité allumée, éteinte).
fn parse_mask(text: &str) -> Vec<Pattern> {
    let mut out: Vec<Pattern> = Vec::new();
    let mut res = 0usize;
    let mut lines = text.lines().map(str::trim).filter(|l| !l.starts_with('#'));
    while let Some(l) = lines.next() {
        if let Some(r) = l.strip_prefix("Resolution=") {
            res = r.parse().unwrap_or(0);
        } else if l == "v2" {
            let Some((w, h)) = lines
                .next()
                .and_then(|d| d.split_once(','))
                .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))
            else {
                break;
            };
            let mut cells = Vec::with_capacity(w * h);
            for _ in 0..h {
                for c in lines.next().unwrap_or("").split(',') {
                    let d: Vec<u32> = c.trim().chars().filter_map(|x| x.to_digit(16)).collect();
                    if d.len() != 3 {
                        cells.push([1.0; 3]);
                        continue;
                    }
                    let (on, off) = (1.0 + 0.0625 * d[1] as f32, 0.0625 * d[2] as f32);
                    // bits de couleur : bleu 1, vert 2, rouge 4
                    let g = |bit: u32| if d[0] & bit != 0 { on } else { off };
                    cells.push([g(4), g(2), g(1)]);
                }
            }
            cells.resize(w * h, [1.0; 3]);
            out.push(Pattern { res, w, h, cells });
        }
    }
    out
}

fn mask_patterns(m: Mask) -> Option<&'static [Pattern]> {
    static T: OnceLock<[Vec<Pattern>; 2]> = OnceLock::new();
    let t = T.get_or_init(|| {
        [
            parse_mask(include_str!("../data/masks/trinitron.txt")),
            parse_mask(include_str!("../data/masks/faded_tube_yellow.txt")),
        ]
    });
    match m {
        Mask::None => None,
        Mask::Trinitron => Some(&t[0]),
        Mask::FadedYellow => Some(&t[1]),
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
        let k = sharp_table(fx.interp);
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
                for (j, wgt) in k[ph * 4..ph * 4 + 4].iter().enumerate() {
                    let sx = (i - 1 + j as i32).clamp(0, sw as i32 - 1) as usize;
                    let c = px(sx, y);
                    for ch in 0..3 {
                        acc[ch] += c[ch] * i32::from(*wgt);
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
    let lut: Vec<u8> = (0..256)
        .map(|v| {
            if fx.gamma == 100 {
                v as u8
            } else {
                (255.0 * (v as f32 / 255.0).powf(g)).round() as u8
            }
        })
        .collect();

    // 3. masque (motif choisi selon la hauteur, comme le mode « 1x »)
    let mp = f32::from(fx.mask_pct) / 100.0;
    let pat =
        mask_patterns(fx.mask).and_then(|ps| ps.iter().rev().find(|p| p.res <= h).or(ps.first()));

    let std_t = std_table(fx.scan);
    let (dark_t, bright_t) = adaptive_tables(fx.scan_dk, fx.scan_br);
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        // 4. vertical : filtre à 4 lignes source selon la phase
        let pos = y as f32 * sh as f32 / h as f32;
        let fl = pos.floor();
        let frac = pos - fl;
        let i0 = fl as i32;
        let rows: [&[[u8; 3]]; 4] = std::array::from_fn(|j| {
            let r = (i0 - 1 + j as i32).clamp(0, sh as i32 - 1) as usize;
            &hrows[r * w..(r + 1) * w]
        });
        let near = if frac <= 0.5 { 1 } else { 2 };
        let ph64 = ((frac * 64.0) as usize).min(63);
        let ph256 = ((frac * 256.0) as usize).min(255);
        let plain = ((y * sh) / h).min(sh - 1);
        for x in 0..w {
            let mut c = [0i32; 3];
            match fx.scan_mode {
                ScanMode::Off => {
                    let p = hrows[plain * w + x];
                    c = [i32::from(p[0]), i32::from(p[1]), i32::from(p[2])];
                }
                ScanMode::Standard => {
                    for j in 0..4 {
                        let wgt = i32::from(std_t[ph64 * 4 + j]);
                        let p = rows[j][x];
                        for ch in 0..3 {
                            c[ch] += wgt * i32::from(p[ch]);
                        }
                    }
                    for v in &mut c {
                        *v = (*v + 64) >> 7;
                    }
                }
                ScanMode::Adaptive => {
                    let n = rows[near][x];
                    let l = (0.299 * f32::from(n[0])
                        + 0.587 * f32::from(n[1])
                        + 0.114 * f32::from(n[2]))
                        / 255.0;
                    let mut acc = [0f32; 3];
                    for j in 0..4 {
                        let (d, b) = (
                            f32::from(dark_t[ph256 * 4 + j]),
                            f32::from(bright_t[ph256 * 4 + j]),
                        );
                        let wgt = d + (b - d) * l;
                        let p = rows[j][x];
                        for ch in 0..3 {
                            acc[ch] += wgt * f32::from(p[ch]);
                        }
                    }
                    for ch in 0..3 {
                        c[ch] = (acc[ch] / 256.0 + 0.5) as i32;
                    }
                }
            }
            let mut o = [0f32; 3];
            for ch in 0..3 {
                o[ch] = f32::from(lut[c[ch].clamp(0, 255) as usize]);
            }
            if let Some(p) = pat {
                let cell = p.cells[(y % p.h) * p.w + x % p.w];
                for ch in 0..3 {
                    o[ch] *= 1.0 - mp * (1.0 - cell[ch]);
                }
            }
            let q = |v: f32| (v + 0.5).clamp(0.0, 255.0) as u8;
            out.push(Color32::from_rgb(q(o[0]), q(o[1]), q(o[2])));
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
    fn data_file_is_complete() {
        assert_eq!(data().len(), (N_SHARP + N_STD) * 256 + 77 * 1024);
        // interpolation, 64 phases : somme 128 ; premières lignes des trois filtres livrés
        for s in (5..=100).step_by(5) {
            for r in sharp_table(s).chunks(4) {
                assert_eq!(r.iter().map(|v| i32::from(*v)).sum::<i32>(), 128);
            }
        }
        assert_eq!(&sharp_table(MEDIUM)[..4], &[2, 124, 2, 0]);
        assert_eq!(&sharp_table(SHARP)[..4], &[0, 128, 0, 0]);
        assert_eq!(&sharp_table(SOFT)[..4], &[10, 108, 10, 0]);
        // scanlines standard 50 % : milieu = 64
        assert_eq!(&std_table(50)[32 * 4..32 * 4 + 4], &[0, 32, 32, 0]);
        // adaptatives 40/80 : milieu sombre 120, clair 204
        let (d, b) = adaptive_tables(40, 80);
        let sum = |t: &[i16]| {
            t[128 * 4..128 * 4 + 4]
                .iter()
                .map(|v| i32::from(*v))
                .sum::<i32>()
        };
        assert_eq!((sum(d), sum(b)), (120, 204));
    }

    #[test]
    fn masks_parse() {
        let t = mask_patterns(Mask::Trinitron).unwrap();
        assert_eq!(
            t.iter().map(|p| (p.res, p.w, p.h)).collect::<Vec<_>>(),
            [(0, 1, 1), (720, 3, 2), (1080, 3, 5), (1440, 3, 6)]
        );
        // 72a : blanc allumé à 112,5 % ; 00c : gris éteint à 75 %
        assert_eq!(t[1].cells[0], [1.0; 3]);
        assert_eq!(t[1].cells[3], [1.125; 3]);
        assert_eq!(t[1].cells[2], [0.75; 3]);
        let f = mask_patterns(Mask::FadedYellow).unwrap();
        assert_eq!(f.len(), 3);
        // 695 : jaune (R, V allumés à 156,25 %, B éteint à 31,25 %)
        assert_eq!(f[1].cells[3], [1.5625, 1.5625, 0.3125]);
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
        // blanc : 255 * 204 / 256 au milieu de la ligne
        let bright = render(&flat(0xFFFFFF), &fx, 320, 448);
        assert_eq!(bright[320].r(), 203);
        let grey = render(&flat(0x808080), &fx, 320, 448);
        assert!(grey[320].r() < 100 && grey[320].r() > 60);
    }

    #[test]
    fn gamma_and_mask() {
        let fx = Fx {
            gamma: 145,
            ..Fx::default()
        };
        let o = render(&flat(0x808080), &fx, 320, 224);
        // 255 * (128/255)^1.45 ~ 94
        assert_eq!(o[0].r(), 94);
        // tube fané à l'échelle x5 (1120 -> motif 1080) : le jaune réduit le bleu
        let fx = Fx {
            mask: Mask::FadedYellow,
            ..Fx::default()
        };
        let o = render(&flat(0x808080), &fx, 1600, 1120);
        assert!(o.iter().any(|c| c.b() < c.r()));
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
