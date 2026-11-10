#![forbid(unsafe_code)]
//! ng-common : types et constantes partagées par tous les crates de l'émulateur Neo Geo.
//! Interfaces GELÉES — toute modification exige un ADR dans docs/adr/.

pub mod bitswap;
pub mod clock;
pub mod crc;
pub mod input;
pub mod video;

// Ré-exports pour usage direct sans qualification de module.
pub use clock::{
    Tick, LINES_PER_FRAME, MCLK_HZ_AES, MCLK_HZ_MVS, MCLK_PER_68K, MCLK_PER_FRAME, MCLK_PER_LINE,
    MCLK_PER_PIXEL, MCLK_PER_YM, MCLK_PER_Z80, PIXELS_PER_LINE,
};
pub use crc::crc32_bytes;
pub use input::{InputProvider, PadState, SystemButtons};
pub use video::{Framebuffer, SCREEN_H, SCREEN_W};

#[cfg(test)]
mod smoke {
    #[test]
    fn it_builds() {}
}
