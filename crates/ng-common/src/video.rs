//! Framebuffer vidéo Neo Geo (320x224 pixels RGBA8).
//!
//! Format des pixels : u32 en 0x00RRGGBB (canal alpha ignoré, toujours 0).

use std::io::{self, Write};
use std::path::Path;

use crate::crc::crc32_bytes;

/// Largeur de l'écran visible en pixels.
pub const SCREEN_W: usize = 320;
/// Hauteur de l'écran visible en pixels.
pub const SCREEN_H: usize = 224;

/// Framebuffer Neo Geo : tampon d'image 320x224 pixels RGBA8 (u32).
#[derive(Debug)]
pub struct Framebuffer {
    /// Pixels dans l'ordre ligne par ligne, format 0x00RRGGBB.
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    /// Crée un framebuffer vide (tous les pixels à zéro).
    pub fn new() -> Self {
        let size = SCREEN_W * SCREEN_H;
        Framebuffer {
            pixels: vec![0u32; size],
        }
    }

    /// Calcule le CRC-32 du framebuffer sérialisé en u32 little-endian.
    pub fn crc32(&self) -> u32 {
        let mut buf = Vec::with_capacity(self.pixels.len() * 4);
        for &px in &self.pixels {
            buf.extend_from_slice(&px.to_le_bytes());
        }
        crc32_bytes(&buf)
    }

    /// Écrit le framebuffer au format PPM binaire (P6).
    pub fn write_ppm(&self, path: &Path) -> io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        // En-tête PPM : "P6\n320 224\n255\n" (15 octets)
        write!(f, "P6\n{} {}\n255\n", SCREEN_W, SCREEN_H)?;
        // Données pixels : R=bits 16-23, G=bits 8-15, B=bits 0-7
        for &px in &self.pixels {
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let b = (px & 0xFF) as u8;
            f.write_all(&[r, g, b])?;
        }
        Ok(())
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ng-common-test-{}", name));
        // Clean up if exists from previous run
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn framebuffer_size() {
        let fb = Framebuffer::new();
        assert_eq!(fb.pixels.len(), 71680); // 320 * 224
    }

    #[test]
    fn framebuffer_initially_zero() {
        let fb = Framebuffer::new();
        for &px in &fb.pixels {
            assert_eq!(px, 0);
        }
    }

    #[test]
    fn framebuffer_crc32_empty() {
        // CRC of empty (all-zero) framebuffer serialized as u32 LE.
        let fb = Framebuffer::new();
        assert_eq!(fb.crc32(), 0xE7C953CF);
    }

    #[test]
    fn framebuffer_crc_changes_on_pixel_write() {
        let mut fb = Framebuffer::new();
        let crc_before = fb.crc32();
        fb.pixels[0] = 0x00112233;
        let crc_after = fb.crc32();
        assert_ne!(crc_before, crc_after);
    }

    #[test]
    fn write_ppm_file_size() {
        let path = temp_path("size.ppm");
        let fb = Framebuffer::new();
        fb.write_ppm(&path).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        // Header: "P6\n320 224\n255\n" = 15 bytes
        // Data: 320 * 224 * 3 = 215040 bytes
        assert_eq!(metadata.len(), 15 + 215040);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn write_ppm_header() {
        let path = temp_path("header.ppm");
        let fb = Framebuffer::new();
        fb.write_ppm(&path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("P6\n320 224\n255\n"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn write_ppm_pixel_colors() {
        // Write a framebuffer with known colors and verify PPM output.
        let mut fb = Framebuffer::new();
        // Top-left pixel: red (0x00FF0000)
        fb.pixels[0] = 0x00FF0000;
        // Second pixel: green (0x0000FF00)
        fb.pixels[1] = 0x0000FF00;
        // Third pixel: blue (0x000000FF)
        fb.pixels[2] = 0x000000FF;

        let path = temp_path("colors.ppm");
        fb.write_ppm(&path).unwrap();

        let mut data = fs::read(&path).unwrap();
        // Skip header (15 bytes)
        data.drain(..15);

        // First pixel: R=255, G=0, B=0
        assert_eq!(data[0], 255);
        assert_eq!(data[1], 0);
        assert_eq!(data[2], 0);
        // Second pixel: R=0, G=255, B=0
        assert_eq!(data[3], 0);
        assert_eq!(data[4], 255);
        assert_eq!(data[5], 0);
        // Third pixel: R=0, G=0, B=255
        assert_eq!(data[6], 0);
        assert_eq!(data[7], 0);
        assert_eq!(data[8], 255);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn framebuffer_default() {
        let fb: Framebuffer = Default::default();
        assert_eq!(fb.pixels.len(), 71680);
    }
}
