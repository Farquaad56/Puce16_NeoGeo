//! Placement de l'image 320x224 dans la fenêtre : facteur entier, pixels nets, image centrée sur
//! un pixel physique entier.

use eframe::egui::{pos2, vec2, Rect};

use crate::prefs::{Aspect, ScaleMode};

pub const FB_W: u32 = 320;
pub const FB_H: u32 = 224;
/// Hauteur d'affichage en 4:3 : 320x240 par facteur (pixels légèrement plus hauts que larges).
const TV_H: u32 = 240;

/// Résultat de `layout`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayRect {
    /// Rectangle de l'image, en points egui.
    pub rect: Rect,
    /// Facteur d'agrandissement entier.
    pub factor: u32,
    /// Taille de l'image en pixels physiques.
    pub px: (u32, u32),
}

/// Taille (pixels physiques) de l'image au facteur 1.
fn base_size(aspect: Aspect) -> (u32, u32) {
    match aspect {
        Aspect::Square => (FB_W, FB_H),
        Aspect::Tv4x3 => (FB_W, TV_H),
    }
}

/// Place l'image dans `avail` (points egui) ; `ppp` = pixels physiques par point.
///
/// Tout est calculé en pixels physiques : facteur entier (au moins 1), coin supérieur gauche sur un
/// pixel entier. Si la zone est trop petite, l'image dépasse (elle est rognée par l'appelant).
pub fn layout(avail: Rect, ppp: f32, mode: ScaleMode, aspect: Aspect) -> DisplayRect {
    let ppp = if ppp > 0.0 { ppp } else { 1.0 };
    let to_px = |v: f32| (v * ppp + 0.001).floor() as i64;
    let (aw, ah) = (to_px(avail.width()).max(0), to_px(avail.height()).max(0));
    let (bw, bh) = base_size(aspect);
    let factor = match mode {
        ScaleMode::Auto => ((aw / i64::from(bw)).min(ah / i64::from(bh))).max(1) as u32,
        ScaleMode::Fixed(n) => n.max(1),
    };
    let (w, h) = (bw * factor, bh * factor);
    let x = (avail.left() * ppp).round() as i64 + (aw - i64::from(w)).div_euclid(2);
    let y = (avail.top() * ppp).round() as i64 + (ah - i64::from(h)).div_euclid(2);
    let min = pos2(x as f32 / ppp, y as f32 / ppp);
    DisplayRect {
        rect: Rect::from_min_size(min, vec2(w as f32 / ppp, h as f32 / ppp)),
        factor,
        px: (w, h),
    }
}

/// Texte court de l'échelle courante : « x3 (960x672) ».
pub fn scale_label(d: &DisplayRect) -> String {
    format!("x{} ({}x{})", d.factor, d.px.0, d.px.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))
    }

    #[test]
    fn auto_picks_the_largest_integer_factor() {
        let d = layout(zone(1920.0, 1080.0), 1.0, ScaleMode::Auto, Aspect::Square);
        assert_eq!((d.factor, d.px), (4, (1280, 896)));
        assert_eq!(d.rect.min, pos2(320.0, 92.0));
        let d = layout(zone(1280.0, 720.0), 1.0, ScaleMode::Auto, Aspect::Square);
        assert_eq!((d.factor, d.px), (3, (960, 672)));
        assert_eq!(scale_label(&d), "x3 (960x672)");
    }

    #[test]
    fn hidpi_counts_physical_pixels() {
        let d = layout(zone(960.0, 540.0), 2.0, ScaleMode::Auto, Aspect::Square);
        assert_eq!((d.factor, d.px), (4, (1280, 896)));
        assert_eq!(d.rect.size(), vec2(640.0, 448.0));
    }

    #[test]
    fn too_small_zone_keeps_factor_one() {
        let d = layout(zone(200.0, 100.0), 1.0, ScaleMode::Auto, Aspect::Square);
        assert_eq!((d.factor, d.px), (1, (320, 224)));
        let d = layout(zone(0.0, 0.0), 0.0, ScaleMode::Auto, Aspect::Square);
        assert_eq!(d.factor, 1);
    }

    #[test]
    fn position_is_on_a_physical_pixel() {
        for ppp in [1.0f32, 1.25, 1.5, 2.0] {
            let avail = Rect::from_min_size(pos2(0.0, 23.2), vec2(1001.3, 707.7));
            let d = layout(avail, ppp, ScaleMode::Auto, Aspect::Square);
            for v in [d.rect.left(), d.rect.top(), d.rect.right(), d.rect.bottom()] {
                let px = v * ppp;
                assert!((px - px.round()).abs() < 1e-3, "{v} à ppp={ppp}");
            }
        }
    }

    #[test]
    fn fixed_factor_is_respected_even_when_cropped() {
        let d = layout(zone(640.0, 480.0), 1.0, ScaleMode::Fixed(5), Aspect::Square);
        assert_eq!((d.factor, d.px), (5, (1600, 1120)));
        assert_eq!(d.rect.min, pos2(-480.0, -320.0));
        let d = layout(zone(640.0, 480.0), 1.0, ScaleMode::Fixed(0), Aspect::Square);
        assert_eq!(d.factor, 1);
    }

    #[test]
    fn tv_aspect_is_320x240_per_factor() {
        let d = layout(zone(1280.0, 720.0), 1.0, ScaleMode::Auto, Aspect::Tv4x3);
        assert_eq!((d.factor, d.px), (3, (960, 720)));
    }
}
