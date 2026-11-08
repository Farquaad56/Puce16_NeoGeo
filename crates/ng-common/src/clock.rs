//! Horloge maître et constantes de timing Neo Geo (MVS/AES).
//!
//! Convention : 1 tick = 1 mclk (horloge maître 24 MHz MVS / 24.167829 MHz AES).
//! Toutes les fréquences dérivées sont exprimées en ticks par cycle d'opération.

/// Un tick de l'horloge maître (mclk).
pub type Tick = u64;

// Diviseurs d'horloge : cycles mclk par cycle d'opération du composant.
/// 68000 à 12 MHz = mclk / 2 → 2 ticks mclk par cycle CPU.
pub const MCLK_PER_68K: u64 = 2;
/// Z80 à 4 MHz = mclk / 6 → 6 ticks mclk par cycle CPU.
pub const MCLK_PER_Z80: u64 = 6;
/// YM2610 à 8 MHz = mclk / 3 → 3 ticks mclk par cycle audio.
pub const MCLK_PER_YM: u64 = 3;
/// Pixel LSPC à 6 MHz = mclk / 4 → 4 ticks mclk par pixel.
pub const MCLK_PER_PIXEL: u64 = 4;

// Géométrie vidéo (compteur de pixels sur 9 bits).
/// Pixels par ligne (inclut blanking horizontal : sync + porches).
pub const PIXELS_PER_LINE: u64 = 384;
/// Ticks mclk par ligne vidéo.
pub const MCLK_PER_LINE: u64 = 1536;
/// Lignes par trame NTSC (compteur de lignes sur 9 bits).
pub const LINES_PER_FRAME: u64 = 264;
/// Ticks mclk par trame complète.
pub const MCLK_PER_FRAME: u64 = 405_504;

// Fréquences d'horloge maître réelles (Hz).
/// Horloge maître MVS : 24 MHz exacts.
pub const MCLK_HZ_MVS: u64 = 24_000_000;
/// Horloge maître AES : 24.167829 MHz (quartz différent).
pub const MCLK_HZ_AES: u64 = 24_167_829;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_consistency() {
        // Vérifie la cohérence des constantes de timing.
        assert_eq!(MCLK_PER_LINE, PIXELS_PER_LINE * MCLK_PER_PIXEL);
        assert_eq!(MCLK_PER_FRAME, MCLK_PER_LINE * LINES_PER_FRAME);
    }

    #[test]
    fn clock_dividers() {
        // Vérifie les diviseurs d'horloge.
        assert_eq!(MCLK_PER_68K, 2);
        assert_eq!(MCLK_PER_Z80, 6);
        assert_eq!(MCLK_PER_YM, 3);
        assert_eq!(MCLK_PER_PIXEL, 4);
    }

    #[test]
    fn frame_geometry() {
        // Vérifie la géométrie de trame.
        assert_eq!(PIXELS_PER_LINE, 384);
        assert_eq!(LINES_PER_FRAME, 264);
        assert_eq!(MCLK_PER_FRAME, 405_504);
    }
}
