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

/// Données communes à toutes les rangées de sortie.
struct Ctx<'a> {
    src: &'a [[u8; 3]],
    fx: &'a Fx,
    w: usize,
    h: usize,
    /// Colonnes : indices source (4 points) et poids ; sans interpolation, un seul indice.
    cols: Vec<([u16; 4], [i32; 4])>,
    kernel_on: bool,
    lut: Vec<u32>,
    /// Gains du motif de masque en 8.8 (256 = inchangé).
    pw: usize,
    ph: usize,
    cells: Vec<[u32; 3]>,
    xmod: Vec<u16>,
    std_t: &'static [i16],
    dark_t: &'static [i16],
    bright_t: &'static [i16],
}

/// Calcule les rangées de sortie `y0..y0 + out.len() / w`.
fn render_rows(c: &Ctx, y0: usize, out: &mut [Color32]) {
    let (sw, sh) = (FB_W as usize, FB_H as usize);
    let (w, h) = (c.w, c.h);
    let mut vrow = vec![[0u8; 3]; sw];
    for (dy, orow) in out.chunks_mut(w).enumerate() {
        let y = y0 + dy;
        // 1. vertical, à la largeur de la source : filtre à 4 lignes selon la phase
        let pos = y as f32 * sh as f32 / h as f32;
        let fl = pos.floor();
        let frac = pos - fl;
        let i0 = fl as i32;
        let row = |j: usize| -> &[[u8; 3]] {
            let r = (i0 - 1 + j as i32).clamp(0, sh as i32 - 1) as usize;
            &c.src[r * sw..(r + 1) * sw]
        };
        match c.fx.scan_mode {
            ScanMode::Off => {
                let r = ((y * sh) / h).min(sh - 1);
                vrow.copy_from_slice(&c.src[r * sw..(r + 1) * sw]);
            }
            ScanMode::Standard => {
                let ph = ((frac * 64.0) as usize).min(63);
                let wg: [i32; 4] = std::array::from_fn(|j| i32::from(c.std_t[ph * 4 + j]));
                let rows: [&[[u8; 3]]; 4] = std::array::from_fn(row);
                for (x, o) in vrow.iter_mut().enumerate() {
                    for ch in 0..3 {
                        let mut a = 64;
                        for j in 0..4 {
                            a += wg[j] * i32::from(rows[j][x][ch]);
                        }
                        o[ch] = (a >> 7).clamp(0, 255) as u8;
                    }
                }
            }
            ScanMode::Adaptive => {
                let ph = ((frac * 256.0) as usize).min(255);
                let near = if frac <= 0.5 { 1 } else { 2 };
                let rows: [&[[u8; 3]]; 4] = std::array::from_fn(row);
                let d: [f32; 4] = std::array::from_fn(|j| f32::from(c.dark_t[ph * 4 + j]));
                let b: [f32; 4] = std::array::from_fn(|j| f32::from(c.bright_t[ph * 4 + j]));
                for (x, o) in vrow.iter_mut().enumerate() {
                    let n = rows[near][x];
                    let l = (0.299 * f32::from(n[0])
                        + 0.587 * f32::from(n[1])
                        + 0.114 * f32::from(n[2]))
                        / 255.0;
                    let wg: [f32; 4] = std::array::from_fn(|j| (d[j] + (b[j] - d[j]) * l) / 256.0);
                    for ch in 0..3 {
                        let mut a = 0.5;
                        for j in 0..4 {
                            a += wg[j] * f32::from(rows[j][x][ch]);
                        }
                        o[ch] = a.clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
        // 2. horizontal (interpolation ou répétition), gamma, masque
        let mrow = (y % c.ph) * c.pw;
        for (x, o) in orow.iter_mut().enumerate() {
            let (idx, wg) = &c.cols[x];
            let mut v = [0i32; 3];
            if c.kernel_on {
                for ch in 0..3 {
                    let mut a = 64;
                    for j in 0..4 {
                        a += wg[j] * i32::from(vrow[usize::from(idx[j])][ch]);
                    }
                    v[ch] = (a >> 7).clamp(0, 255);
                }
            } else {
                let p = vrow[usize::from(idx[0])];
                v = [i32::from(p[0]), i32::from(p[1]), i32::from(p[2])];
            }
            let g = &c.cells[mrow + usize::from(c.xmod[x])];
            let f = |ch: usize| ((c.lut[v[ch] as usize] * g[ch] + 128) >> 8).min(255) as u8;
            *o = Color32::from_rgb(f(0), f(1), f(2));
        }
    }
}

/// Applique les filtres : `src` = image 320x224 (0x00RRGGBB), sortie `w` x `h`.
pub fn render(src: &[u32], fx: &Fx, w: usize, h: usize) -> Vec<Color32> {
    let (sw, sh) = (FB_W as usize, FB_H as usize);
    assert!(src.len() >= sw * sh && w > 0 && h > 0);
    let src: Vec<[u8; 3]> = src[..sw * sh]
        .iter()
        .map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();

    // colonnes
    let kernel_on = fx.interp > 0;
    let k = sharp_table(fx.interp);
    let cols: Vec<([u16; 4], [i32; 4])> = (0..w)
        .map(|x| {
            if !kernel_on {
                return ([(x * sw / w).min(sw - 1) as u16, 0, 0, 0], [0; 4]);
            }
            let pos = (x as f32 + 0.5) * sw as f32 / w as f32 - 0.5;
            let i = pos.floor();
            let ph = (((pos - i) * 64.0) as usize).min(63);
            let idx =
                std::array::from_fn(|j| (i as i32 - 1 + j as i32).clamp(0, sw as i32 - 1) as u16);
            let wg = std::array::from_fn(|j| i32::from(k[ph * 4 + j]));
            (idx, wg)
        })
        .collect();

    // gamma
    let g = f32::from(fx.gamma) / 100.0;
    let lut: Vec<u32> = (0..256)
        .map(|v| {
            if fx.gamma == 100 {
                v as u32
            } else {
                (255.0 * (v as f32 / 255.0).powf(g)).round() as u32
            }
        })
        .collect();

    // masque (motif choisi selon la hauteur, comme le mode « 1x »), gains en 8.8
    let mp = f32::from(fx.mask_pct) / 100.0;
    let pat =
        mask_patterns(fx.mask).and_then(|ps| ps.iter().rev().find(|p| p.res <= h).or(ps.first()));
    let (pw, ph, cells) = match pat {
        Some(p) => (
            p.w,
            p.h,
            p.cells
                .iter()
                .map(|c| {
                    std::array::from_fn(|ch| ((1.0 - mp * (1.0 - c[ch])) * 256.0 + 0.5) as u32)
                })
                .collect(),
        ),
        None => (1, 1, vec![[256u32; 3]]),
    };
    let xmod = (0..w).map(|x| (x % pw) as u16).collect();

    let (dark_t, bright_t) = adaptive_tables(fx.scan_dk, fx.scan_br);
    let ctx = Ctx {
        src: &src,
        fx,
        w,
        h,
        cols,
        kernel_on,
        lut,
        pw,
        ph,
        cells,
        xmod,
        std_t: std_table(fx.scan),
        dark_t,
        bright_t,
    };

    let mut out = vec![Color32::BLACK; w * h];
    let threads = if w * h < 200_000 {
        1
    } else {
        std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(8)
    };
    if threads <= 1 {
        render_rows(&ctx, 0, &mut out);
    } else {
        let per = h.div_ceil(threads);
        std::thread::scope(|s| {
            for (i, chunk) in out.chunks_mut(per * w).enumerate() {
                let ctx = &ctx;
                s.spawn(move || render_rows(ctx, i * per, chunk));
            }
        });
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

    /// Banc du coût des filtres par trame, à toutes les échelles, ratios et résolutions plein écran :
    /// `cargo test --release -p ng-gui bench_fx -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_fx() {
        use crate::display::layout;
        use crate::prefs::{Aspect, ScaleMode};
        use eframe::egui::{pos2, vec2, Rect};
        use std::time::Instant;

        // image pseudo-aléatoire (pire cas : pas de zones plates)
        let mut seed = 12345u32;
        let src: Vec<u32> = (0..FB_W * FB_H)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                seed >> 8
            })
            .collect();
        let worst = Fx {
            interp: SHARP,
            scan_mode: ScanMode::Adaptive,
            mask: Mask::Trinitron,
            gamma: 145,
            ..Fx::default()
        };
        let presets = [
            (
                "Interp seule",
                Fx {
                    interp: MEDIUM,
                    ..Fx::default()
                },
            ),
            (
                "Scanl. std",
                Fx {
                    scan_mode: ScanMode::Standard,
                    ..Fx::default()
                },
            ),
            ("Trinitron", Fx::preset(Preset::SonyTrinitron)),
            ("Faded Tube", Fx::preset(Preset::FadedTube)),
            ("Tout", worst),
        ];
        let mut cases: Vec<(String, usize, usize)> = Vec::new();
        for (an, aspect) in [("1:1", Aspect::Square), ("4:3", Aspect::Tv4x3)] {
            for n in 1..=8 {
                let l = layout(
                    Rect::from_min_size(pos2(0.0, 0.0), vec2(8000.0, 8000.0)),
                    1.0,
                    ScaleMode::Fixed(n),
                    aspect,
                );
                cases.push((format!("{an} x{n}"), l.px.0 as usize, l.px.1 as usize));
            }
            for (rn, rw, rh) in [
                ("720p", 1280.0, 720.0),
                ("1080p", 1920.0, 1080.0),
                ("1440p", 2560.0, 1440.0),
                ("4K", 3840.0, 2160.0),
            ] {
                let l = layout(
                    Rect::from_min_size(pos2(0.0, 0.0), vec2(rw, rh)),
                    1.0,
                    ScaleMode::Auto,
                    aspect,
                );
                cases.push((
                    format!("{an} plein écran {rn}"),
                    l.px.0 as usize,
                    l.px.1 as usize,
                ));
            }
        }
        println!("budget total : 16,89 ms/trame ; temps de filtrage (ms) par trame");
        print!("{:28} {:>11}", "cas", "taille");
        for (n, _) in &presets {
            print!(" {n:>13}");
        }
        println!();
        for (name, w, h) in cases {
            print!("{name:28} {:>11}", format!("{w}x{h}"));
            for (_, fx) in &presets {
                let _ = render(&src, fx, w, h); // chauffe + tables
                let n = 8;
                let t0 = Instant::now();
                for _ in 0..n {
                    std::hint::black_box(render(&src, fx, w, h));
                }
                print!(
                    " {:13.2}",
                    t0.elapsed().as_secs_f64() * 1000.0 / f64::from(n)
                );
            }
            println!();
        }
    }
}
