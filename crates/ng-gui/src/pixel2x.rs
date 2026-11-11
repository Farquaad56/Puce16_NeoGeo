//! Filtres d'agrandissement ×2 pour le pixel art : chaque pixel de la source donne un bloc 2x2.
//!
//! Les règles de Scale2x, Eagle, Super Eagle, 2xSaI et Super 2xSaI suivent les implémentations de
//! référence (RetroArch, `gfx/video_filters`). HQ2x utilise la table de poids officielle
//! (`data/hq2x_lut.bin`, tirée de `tools/gen_hq2x_lut.py`) et la comparaison YUV de l'original.
//!
//! 2xPM et SuperScale sont des plugins de Kega Fusion dont le code n'est pas public : leurs
//! versions ici sont des approximations, signalées comme telles dans l'interface.

use std::sync::OnceLock;

/// Algorithme d'agrandissement.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scaler {
    #[default]
    Off,
    Hq2x,
    Pm2x,
    Eagle,
    SuperEagle,
    SuperScale,
    SuperScale75,
    Sai2x,
    SuperSai2x,
    Scale2x,
}

impl Scaler {
    pub const ALL: [Scaler; 10] = [
        Scaler::Off,
        Scaler::Hq2x,
        Scaler::Pm2x,
        Scaler::Eagle,
        Scaler::SuperEagle,
        Scaler::SuperScale,
        Scaler::SuperScale75,
        Scaler::Sai2x,
        Scaler::SuperSai2x,
        Scaler::Scale2x,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scaler::Off => "Aucun",
            Scaler::Hq2x => "HQ2X",
            Scaler::Pm2x => "2xPM (approx.)",
            Scaler::Eagle => "Eagle",
            Scaler::SuperEagle => "Super Eagle",
            Scaler::SuperScale => "SuperScale (approx.)",
            Scaler::SuperScale75 => "SuperScale75 (approx.)",
            Scaler::Sai2x => "2xSAI",
            Scaler::SuperSai2x => "Super 2xSaI",
            Scaler::Scale2x => "Scale2x",
        }
    }

    /// Identifiant stable pour `gui.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Scaler::Off => "off",
            Scaler::Hq2x => "hq2x",
            Scaler::Pm2x => "2xpm",
            Scaler::Eagle => "eagle",
            Scaler::SuperEagle => "supereagle",
            Scaler::SuperScale => "superscale",
            Scaler::SuperScale75 => "superscale75",
            Scaler::Sai2x => "2xsai",
            Scaler::SuperSai2x => "super2xsai",
            Scaler::Scale2x => "scale2x",
        }
    }

    pub fn from_key(s: &str) -> Option<Scaler> {
        Scaler::ALL.into_iter().find(|k| k.key() == s)
    }

    /// Est-ce une approximation faute de source publique ?
    pub fn approximate(self) -> bool {
        matches!(
            self,
            Scaler::Pm2x | Scaler::SuperScale | Scaler::SuperScale75
        )
    }
}

type Px = u32;

fn mix2(a: Px, b: Px) -> Px {
    ((a & 0xFEFE_FEFE) >> 1) + ((b & 0xFEFE_FEFE) >> 1) + (a & b & 0x0101_0101)
}

fn mix4(a: Px, b: Px, c: Px, d: Px) -> Px {
    ((a & 0xFCFC_FCFC) >> 2)
        + ((b & 0xFCFC_FCFC) >> 2)
        + ((c & 0xFCFC_FCFC) >> 2)
        + ((d & 0xFCFC_FCFC) >> 2)
        + ((((a & 0x0303_0303) + (b & 0x0303_0303) + (c & 0x0303_0303) + (d & 0x0303_0303)) >> 2)
            & 0x0303_0303)
}

fn res(a: Px, b: Px, c: Px, d: Px) -> i32 {
    i32::from(a != c || a != d) - i32::from(b != c || b != d)
}

/// Voisinage 4x4 autour du pixel (x, y), bords répliqués.
struct Src<'a> {
    p: &'a [Px],
    w: usize,
    h: usize,
}

impl Src<'_> {
    fn at(&self, x: isize, y: isize) -> Px {
        let xx = x.clamp(0, self.w as isize - 1) as usize;
        let yy = y.clamp(0, self.h as isize - 1) as usize;
        self.p[yy * self.w + xx]
    }
}

/// Bloc de sortie : haut-gauche, haut-droite, bas-gauche, bas-droite.
type Block = [Px; 4];

fn scale2x(s: &Src, x: isize, y: isize) -> Block {
    let c = s.at(x, y);
    let (a, b, d, e) = (
        s.at(x, y - 1),
        s.at(x - 1, y),
        s.at(x + 1, y),
        s.at(x, y + 1),
    );
    if b != d && a != e {
        [
            if a == b { a } else { c },
            if a == d { a } else { c },
            if e == b { e } else { c },
            if e == d { e } else { c },
        ]
    } else {
        [c; 4]
    }
}

fn eagle(s: &Src, x: isize, y: isize) -> Block {
    let c = s.at(x, y);
    let (t0, t1, t2) = (s.at(x - 1, y - 1), s.at(x, y - 1), s.at(x + 1, y - 1));
    let (l, r) = (s.at(x - 1, y), s.at(x + 1, y));
    let (b0, b1, b2) = (s.at(x - 1, y + 1), s.at(x, y + 1), s.at(x + 1, y + 1));
    [
        if t0 == t1 && t0 == l { t0 } else { c },
        if t1 == t2 && t2 == r { t2 } else { c },
        if l == b0 && b0 == b1 { b0 } else { c },
        if r == b1 && b1 == b2 { b2 } else { c },
    ]
}

fn super_eagle(s: &Src, x: isize, y: isize) -> Block {
    let b1 = s.at(x, y - 1);
    let b2 = s.at(x + 1, y - 1);
    let c4 = s.at(x - 1, y);
    let c5 = s.at(x, y);
    let c6 = s.at(x + 1, y);
    let s2 = s.at(x + 2, y);
    let c1 = s.at(x - 1, y + 1);
    let c2 = s.at(x, y + 1);
    let c3 = s.at(x + 1, y + 1);
    let s1 = s.at(x + 2, y + 1);
    let a1 = s.at(x, y + 2);
    let a2 = s.at(x + 1, y + 2);
    let (p1a, p1b, p2a, p2b);
    if c2 == c6 && c5 != c3 {
        p1b = c2;
        p2a = c2;
        p1a = if c1 == c2 || c6 == b2 {
            mix2(c2, mix2(c2, c5))
        } else {
            mix2(c5, c6)
        };
        p2b = if c6 == s2 || c2 == a1 {
            mix2(c2, mix2(c2, c3))
        } else {
            mix2(c2, c3)
        };
    } else if c5 == c3 && c2 != c6 {
        p2b = c5;
        p1a = c5;
        p1b = if b1 == c5 || c3 == s1 {
            mix2(c5, mix2(c5, c6))
        } else {
            mix2(c5, c6)
        };
        p2a = if c3 == a2 || c4 == c5 {
            mix2(c5, mix2(c5, c2))
        } else {
            mix2(c2, c3)
        };
    } else if c5 == c3 && c2 == c6 {
        if c5 == c6 {
            return [c5; 4];
        }
        let r =
            res(c6, c5, c1, a1) + res(c6, c5, c4, b1) + res(c6, c5, a2, s1) + res(c6, c5, b2, s2);
        if r > 0 {
            p1b = c2;
            p2a = c2;
            p1a = mix2(c5, c6);
            p2b = p1a;
        } else if r < 0 {
            p2b = c5;
            p1a = c5;
            p1b = mix2(c5, c6);
            p2a = p1b;
        } else {
            p2b = c5;
            p1a = c5;
            p1b = c2;
            p2a = c2;
        }
    } else {
        let m26 = mix2(c2, c6);
        p2b = mix4(c3, c3, c3, m26);
        p1a = mix4(c5, c5, c5, m26);
        let m53 = mix2(c5, c3);
        p2a = mix4(c2, c2, c2, m53);
        p1b = mix4(c6, c6, c6, m53);
    }
    [p1a, p1b, p2a, p2b]
}

fn sai2x(s: &Src, x: isize, y: isize) -> Block {
    let ci = s.at(x - 1, y - 1);
    let ce = s.at(x, y - 1);
    let cf = s.at(x + 1, y - 1);
    let cj = s.at(x + 2, y - 1);
    let cg = s.at(x - 1, y);
    let ca = s.at(x, y);
    let cb = s.at(x + 1, y);
    let ck = s.at(x + 2, y);
    let ch = s.at(x - 1, y + 1);
    let cc = s.at(x, y + 1);
    let cd = s.at(x + 1, y + 1);
    let cl = s.at(x + 2, y + 1);
    let cm = s.at(x - 1, y + 2);
    let cn = s.at(x, y + 2);
    let co = s.at(x + 1, y + 2);
    let (product, product1, product2);
    if ca == cd && cb != cc {
        product = if (ca == ce && cb == cl) || (ca == cc && ca == cf && cb != ce && cb == cj) {
            ca
        } else {
            mix2(ca, cb)
        };
        product1 = if (ca == cg && cc == co) || (ca == cb && ca == ch && cg != cc && cc == cm) {
            ca
        } else {
            mix2(ca, cc)
        };
        product2 = ca;
    } else if cb == cc && ca != cd {
        product = if (cb == cf && ca == ch) || (cb == ce && cb == cd && ca != cf && ca == ci) {
            cb
        } else {
            mix2(ca, cb)
        };
        product1 = if (cc == ch && ca == cf) || (cc == cg && cc == cd && ca != ch && ca == ci) {
            cc
        } else {
            mix2(ca, cc)
        };
        product2 = cb;
    } else if ca == cd && cb == cc {
        if ca == cb {
            return [ca; 4];
        }
        product1 = mix2(ca, cc);
        product = mix2(ca, cb);
        let r =
            res(ca, cb, cg, ce) + res(cb, ca, ck, cf) + res(cb, ca, ch, cn) + res(ca, cb, cl, co);
        product2 = match r.cmp(&0) {
            std::cmp::Ordering::Greater => ca,
            std::cmp::Ordering::Less => cb,
            std::cmp::Ordering::Equal => mix4(ca, cb, cc, cd),
        };
    } else {
        product2 = mix4(ca, cb, cc, cd);
        product = if ca == cc && ca == cf && cb != ce && cb == cj {
            ca
        } else if cb == ce && cb == cd && ca != cf && ca == ci {
            cb
        } else {
            mix2(ca, cb)
        };
        product1 = if ca == cb && ca == ch && cg != cc && cc == cm {
            ca
        } else if cc == cg && cc == cd && ca != ch && ca == ci {
            cc
        } else {
            mix2(ca, cc)
        };
    }
    [ca, product, product1, product2]
}

fn super_sai2x(s: &Src, x: isize, y: isize) -> Block {
    let b0 = s.at(x - 1, y - 1);
    let b1 = s.at(x, y - 1);
    let b2 = s.at(x + 1, y - 1);
    let b3 = s.at(x + 2, y - 1);
    let c4 = s.at(x - 1, y);
    let c5 = s.at(x, y);
    let c6 = s.at(x + 1, y);
    let s2 = s.at(x + 2, y);
    let c1 = s.at(x - 1, y + 1);
    let c2 = s.at(x, y + 1);
    let c3 = s.at(x + 1, y + 1);
    let s1 = s.at(x + 2, y + 1);
    let a0 = s.at(x - 1, y + 2);
    let a1 = s.at(x, y + 2);
    let a2 = s.at(x + 1, y + 2);
    let a3 = s.at(x + 2, y + 2);
    let (p1a, p1b, p2a, p2b);
    if c2 == c6 && c5 != c3 {
        p2b = c2;
        p1b = c2;
    } else if c5 == c3 && c2 != c6 {
        p2b = c5;
        p1b = c5;
    } else if c5 == c3 && c2 == c6 {
        let r =
            res(c6, c5, c1, a1) + res(c6, c5, c4, b1) + res(c6, c5, a2, s1) + res(c6, c5, b2, s2);
        let v = match r.cmp(&0) {
            std::cmp::Ordering::Greater => c6,
            std::cmp::Ordering::Less => c5,
            std::cmp::Ordering::Equal => mix2(c5, c6),
        };
        p2b = v;
        p1b = v;
    } else {
        p2b = if c6 == c3 && c3 == a1 && c2 != a2 && c3 != a0 {
            mix4(c3, c3, c3, c2)
        } else if c5 == c2 && c2 == a2 && a1 != c3 && c2 != a3 {
            mix4(c2, c2, c2, c3)
        } else {
            mix2(c2, c3)
        };
        p1b = if c6 == c3 && c6 == b1 && c5 != b2 && c6 != b0 {
            mix4(c6, c6, c6, c5)
        } else if c5 == c2 && c5 == b2 && b1 != c6 && c5 != b3 {
            mix4(c6, c5, c5, c5)
        } else {
            mix2(c5, c6)
        };
    }
    p2a = if (c5 == c3 && c2 != c6 && c4 == c5 && c5 != a2)
        || (c5 == c1 && c6 == c5 && c4 != c2 && c5 != a0)
    {
        mix2(c2, c5)
    } else {
        c2
    };
    p1a = if (c2 == c6 && c5 != c3 && c1 == c2 && c2 != b2)
        || (c4 == c2 && c3 == c2 && c1 != c5 && c2 != b0)
    {
        mix2(c2, c5)
    } else {
        c5
    };
    [p1a, p1b, p2a, p2b]
}

/// Approximation de 2xPM : lissage vers les voisins du quadrant, sans toucher aux contours
/// francs (là où Scale2x ne trouve rien à adoucir).
fn pm2x(s: &Src, x: isize, y: isize) -> Block {
    let c = s.at(x, y);
    let quad = |sx: isize, sy: isize| {
        let h = s.at(x + sx, y);
        let v = s.at(x, y + sy);
        mix2(mix2(c, h), mix2(c, v))
    };
    [quad(-1, -1), quad(1, -1), quad(-1, 1), quad(1, 1)]
}

/// Approximation de SuperScale : géométrie Scale2x, seconde ligne de chaque paire assombrie.
fn dim(p: Px, pct: u32) -> Px {
    let ch = |sh: u32| ((((p >> sh) & 0xFF) * pct + 50) / 100) << sh;
    ch(16) | ch(8) | ch(0)
}

fn superscale(s: &Src, x: isize, y: isize, pct: u32) -> Block {
    let b = scale2x(s, x, y);
    [b[0], b[1], dim(b[2], pct), dim(b[3], pct)]
}

/// Table de poids de HQ2x : 64 lignes (croisements x 4 quarts) de 256 motifs, 4 poids chacun
/// (centre, diagonale, horizontal, vertical).
fn hq_lut() -> &'static [u8] {
    static L: OnceLock<&'static [u8]> = OnceLock::new();
    L.get_or_init(|| include_bytes!("../data/hq2x_lut.bin"))
}

/// Luminance et chrominances (échelle 1024) pour le test de différence de HQ2x.
fn yuv(p: Px) -> [i32; 3] {
    let (r, g, b) = (
        ((p >> 16) & 0xFF) as i32,
        ((p >> 8) & 0xFF) as i32,
        (p & 0xFF) as i32,
    );
    [
        306 * r + 601 * g + 117 * b,
        -173 * r - 339 * g + 512 * b,
        512 * r - 429 * g - 83 * b,
    ]
}

fn differs(a: &[i32; 3], b: &[i32; 3]) -> bool {
    (a[0] - b[0]).abs() > 48 * 1024
        || (a[1] - b[1]).abs() > 7 * 1024
        || (a[2] - b[2]).abs() > 6 * 1024
}

fn hq2x(s: &Src, x: isize, y: isize) -> Block {
    let n = |dx: isize, dy: isize| s.at(x + dx, y + dy);
    // voisins dans l'ordre des bits du motif : HG, H, HD, G, D, BG, B, BD
    let px = [
        n(-1, -1),
        n(0, -1),
        n(1, -1),
        n(-1, 0),
        n(1, 0),
        n(-1, 1),
        n(0, 1),
        n(1, 1),
    ];
    let c = n(0, 0);
    let yc = yuv(c);
    let yn: [[i32; 3]; 8] = std::array::from_fn(|i| yuv(px[i]));
    let mut pat = 0usize;
    for (i, v) in yn.iter().enumerate() {
        if differs(&yc, v) {
            pat |= 1 << i;
        }
    }
    // croisements : gauche/haut, haut/droite, bas/gauche, droite/bas
    let cross = usize::from(differs(&yn[3], &yn[1]))
        | usize::from(differs(&yn[1], &yn[4])) << 1
        | usize::from(differs(&yn[6], &yn[3])) << 2
        | usize::from(differs(&yn[4], &yn[6])) << 3;
    let lut = hq_lut();
    let mut out = [c; 4];
    for (quad, o) in out.iter_mut().enumerate() {
        let (sx, sy) = (
            if quad & 1 == 1 { 1 } else { -1 },
            if quad & 2 == 2 { 1 } else { -1 },
        );
        let tap = [c, n(sx, sy), n(sx, 0), n(0, sy)];
        let at = ((cross * 4 + quad) * 256 + pat) * 4;
        let wg: [u32; 4] = std::array::from_fn(|i| u32::from(lut[at + i]));
        let sum = wg.iter().sum::<u32>().max(1);
        let ch = |sh: u32| {
            let a: u32 = (0..4).map(|i| wg[i] * ((tap[i] >> sh) & 0xFF)).sum();
            ((a + sum / 2) / sum) << sh
        };
        *o = ch(16) | ch(8) | ch(0);
    }
    out
}

/// Agrandit `src` (`w` x `h`, 0x00RRGGBB) en `2w` x `2h`. Sans filtre, renvoie `None`.
pub fn upscale(src: &[u32], w: usize, h: usize, kind: Scaler) -> Option<Vec<u32>> {
    assert!(src.len() >= w * h && w > 0 && h > 0);
    let f: fn(&Src, isize, isize) -> Block = match kind {
        Scaler::Off => return None,
        Scaler::Hq2x => hq2x,
        Scaler::Pm2x => pm2x,
        Scaler::Eagle => eagle,
        Scaler::SuperEagle => super_eagle,
        Scaler::SuperScale => |s, x, y| superscale(s, x, y, 50),
        Scaler::SuperScale75 => |s, x, y| superscale(s, x, y, 75),
        Scaler::Sai2x => sai2x,
        Scaler::SuperSai2x => super_sai2x,
        Scaler::Scale2x => scale2x,
    };
    let s = Src {
        p: &src[..w * h],
        w,
        h,
    };
    let mut out = vec![0u32; 4 * w * h];
    let w2 = 2 * w;
    let rows = |y0: usize, chunk: &mut [u32]| {
        for (dy, pair) in chunk.chunks_mut(2 * w2).enumerate() {
            let y = (y0 + dy) as isize;
            let (top, bot) = pair.split_at_mut(w2);
            for x in 0..w {
                let b = f(&s, x as isize, y);
                top[2 * x] = b[0];
                top[2 * x + 1] = b[1];
                bot[2 * x] = b[2];
                bot[2 * x + 1] = b[3];
            }
        }
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(8);
    if threads <= 1 || h < 64 {
        rows(0, &mut out);
    } else {
        let per = h.div_ceil(threads);
        std::thread::scope(|sc| {
            for (i, chunk) in out.chunks_mut(per * 2 * w2).enumerate() {
                let rows = &rows;
                sc.spawn(move || rows(i * per, chunk));
            }
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 16;
    const H: usize = 12;

    /// Motif de test : fond uni, un carré, une diagonale.
    fn pattern() -> Vec<u32> {
        let mut v = vec![0x0010_2030u32; W * H];
        for y in 3..8 {
            for x in 3..9 {
                v[y * W + x] = 0x00E0_8040;
            }
        }
        for i in 0..6 {
            v[(5 + i) * W + 8 + i] = 0x0030_C070;
        }
        v
    }

    fn mirror(v: &[u32]) -> Vec<u32> {
        let mut o = v.to_vec();
        for y in 0..H {
            for x in 0..W {
                o[y * W + x] = v[y * W + (W - 1 - x)];
            }
        }
        o
    }

    #[test]
    fn flat_image_stays_flat() {
        let src = vec![0x0012_3456u32; W * H];
        for k in Scaler::ALL.into_iter().skip(1) {
            let o = upscale(&src, W, H, k).unwrap();
            assert_eq!(o.len(), 4 * W * H);
            let want = match k {
                Scaler::SuperScale | Scaler::SuperScale75 => None,
                _ => Some(0x0012_3456),
            };
            if let Some(w) = want {
                assert!(o.iter().all(|p| *p == w), "{k:?}");
            }
        }
        assert!(upscale(&src, W, H, Scaler::Off).is_none());
    }

    #[test]
    fn colors_stay_in_source_range() {
        // aucun filtre ne sort de l'enveloppe des couleurs d'entrée (par canal)
        let src = pattern();
        for k in Scaler::ALL.into_iter().skip(1) {
            let o = upscale(&src, W, H, k).unwrap();
            for sh in [0, 8, 16] {
                let lo = src.iter().map(|p| (p >> sh) & 0xFF).min().unwrap();
                let hi = src.iter().map(|p| (p >> sh) & 0xFF).max().unwrap();
                assert!(
                    o.iter().all(|p| {
                        let c = (p >> sh) & 0xFF;
                        // SuperScale assombrit : seul le plafond compte
                        c <= hi
                            && (c >= lo || matches!(k, Scaler::SuperScale | Scaler::SuperScale75))
                    }),
                    "{k:?}"
                );
            }
        }
    }

    #[test]
    fn mirror_symmetry() {
        // Scale2x, Eagle et HQ2x sont symétriques : miroir avant == miroir après
        let src = pattern();
        let m = mirror(&src);
        for k in [Scaler::Scale2x, Scaler::Eagle, Scaler::Hq2x, Scaler::Pm2x] {
            let a = upscale(&src, W, H, k).unwrap();
            let b = upscale(&m, W, H, k).unwrap();
            for y in 0..2 * H {
                for x in 0..2 * W {
                    assert_eq!(
                        a[y * 2 * W + x],
                        b[y * 2 * W + (2 * W - 1 - x)],
                        "{k:?} ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn scale2x_rounds_a_diagonal_corner() {
        // coin d'un carré : le pixel d'angle garde sa couleur, les voisins de coin sont adoucis
        let mut src = vec![0u32; 5 * 5];
        src[2 * 5 + 2] = 0x00FF_FFFF;
        src[2 * 5 + 3] = 0x00FF_FFFF;
        src[3 * 5 + 2] = 0x00FF_FFFF;
        let o = upscale(&src, 5, 5, Scaler::Scale2x).unwrap();
        // pixel (2,2) blanc, haut = noir, gauche = noir : son coin haut-gauche devient noir
        assert_eq!(o[(2 * 2) * 10 + 2 * 2], 0);
        // le coin bas-droit reste blanc
        assert_eq!(o[(2 * 2 + 1) * 10 + 2 * 2 + 1], 0x00FF_FFFF);
    }

    #[test]
    fn hq2x_table_is_complete() {
        assert_eq!(hq_lut().len(), 256 * 64 * 4);
    }

    #[test]
    fn labels_and_keys_round_trip() {
        for k in Scaler::ALL {
            assert_eq!(Scaler::from_key(k.key()), Some(k));
            assert!(!k.label().is_empty());
        }
        assert_eq!(Scaler::from_key("x"), None);
    }

    #[test]
    #[ignore]
    fn bench_upscale() {
        let src: Vec<u32> = (0..320 * 224u32)
            .map(|i| i.wrapping_mul(2_654_435_761) & 0x00F0_F0F0)
            .collect();
        for k in Scaler::ALL.into_iter().skip(1) {
            let t = std::time::Instant::now();
            for _ in 0..20 {
                std::hint::black_box(upscale(&src, 320, 224, k));
            }
            eprintln!(
                "{:<24} {:.2} ms",
                k.label(),
                t.elapsed().as_secs_f64() * 50.0
            );
        }
    }
}
