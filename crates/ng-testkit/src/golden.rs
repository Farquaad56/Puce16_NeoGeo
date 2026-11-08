//! Comparateur d'images golden : PNG de MAME contre trame émulateur.
//!
//! Les snapshots MAME (`screen:snapshot()`) font 320x224 pour le Neo Geo.

use std::io::{BufReader, Write};
use std::path::Path;

use ng_common::{Framebuffer, SCREEN_H, SCREEN_W};

/// Résultat d'une comparaison de frames.
#[derive(Debug)]
pub struct Diff {
    /// Nombre de pixels différents.
    pub different: usize,
    /// Nombre total de pixels comparés.
    pub total: usize,
    /// Position (x,y) du premier pixel différent, ou None si identiques.
    pub first: Option<(usize, usize)>,
}

/// Charge un PNG et retourne (largeur, hauteur, pixels en 0x00RRGGBB).
pub fn load_png_rgb(path: &Path) -> Result<(usize, usize, Vec<u32>), String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("ouverture de {} : {}", path.display(), e))?;
    let buf_reader = BufReader::new(file);
    let decoder = png::Decoder::new(buf_reader);

    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("lecture PNG de {} : {}", path.display(), e))?;
    let info = reader.info();
    let width = info.width as usize;
    let height = info.height as usize;

    // Déterminer le nombre d'octets par pixel selon le type de couleur
    let bytes_per_pixel = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => return Err(format!("type de couleur PNG non supporté : {:?}", other)),
    };

    let row_bytes = width * bytes_per_pixel;
    let mut pixels = Vec::with_capacity(width * height);
    let mut row_buf = vec![0u8; row_bytes];

    for _ in 0..height {
        reader
            .read_row(&mut row_buf)
            .map_err(|e| format!("lecture ligne PNG : {}", e))?;
        for x in 0..width {
            let offset = x * bytes_per_pixel;
            let r = row_buf[offset] as u32;
            let g = row_buf[offset + 1] as u32;
            let b = row_buf[offset + 2] as u32;
            // Alpha ignoré (offset+3 pour RGBA)
            pixels.push((r << 16) | (g << 8) | b);
        }
    }

    Ok((width, height, pixels))
}

/// Compare une trame émulateur avec un golden. Retourne Err si taille incorrecte.
pub fn diff_frame(fb: &Framebuffer, golden: &[u32], w: usize, h: usize) -> Result<Diff, String> {
    if w != SCREEN_W || h != SCREEN_H {
        return Err(format!("taille golden {}x{}, attendu 320x224", w, h));
    }

    let total = w * h;
    if fb.pixels.len() != total || golden.len() != total {
        return Err("longueur de pixels incorrecte".to_string());
    }

    let mut different = 0usize;
    let mut first: Option<(usize, usize)> = None;

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if fb.pixels[idx] != golden[idx] {
                different += 1;
                if first.is_none() {
                    first = Some((x, y));
                }
            }
        }
    }

    Ok(Diff {
        different,
        total,
        first,
    })
}

/// Écrit une image de différence au format PPM : pixels identiques en gris, différents en rouge.
pub fn write_diff_ppm(fb: &Framebuffer, golden: &[u32], path: &Path) -> std::io::Result<()> {
    let w = SCREEN_W;
    let h = SCREEN_H;

    let mut f = std::fs::File::create(path)?;
    writeln!(f, "P6")?;
    writeln!(f, "{} {}", w, h)?;
    writeln!(f, "255")?;

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if fb.pixels[idx] == golden[idx] {
                // Identique : gris (128,128,128)
                f.write_all(&[128u8, 128, 128])?;
            } else {
                // Différent : rouge (255,0,0)
                f.write_all(&[255u8, 0, 0])?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ng-testkit-{}", name));
        let _ = fs::remove_file(&path);
        path
    }

    /// Écrit un PNG 2x2 avec des couleurs connues.
    fn write_test_png(path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, 2, 2);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();

        // Pixel (0,0): rouge, (1,0): vert, (0,1): bleu, (1,1): blanc
        let row = [255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        writer.write_image_data(&row).unwrap();
    }

    #[test]
    fn load_png_roundtrip() {
        let path = temp_path("roundtrip.png");
        write_test_png(&path);

        let (w, h, pixels) = load_png_rgb(&path).expect("load_png_rgb failed");
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(pixels.len(), 4);

        // Pixel (0,0): rouge
        assert_eq!(pixels[0], 0x00FF0000);
        // Pixel (1,0): vert
        assert_eq!(pixels[1], 0x0000FF00);
        // Pixel (0,1): bleu
        assert_eq!(pixels[2], 0x000000FF);
        // Pixel (1,1): blanc
        assert_eq!(pixels[3], 0x00FFFFFF);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn diff_frame_identical() {
        let fb = Framebuffer::new();
        let golden = fb.pixels.clone();
        let result = diff_frame(&fb, &golden, SCREEN_W, SCREEN_H).expect("diff failed");
        assert_eq!(result.different, 0);
        assert_eq!(result.total, 71680);
        assert_eq!(result.first, None);
    }

    #[test]
    fn diff_frame_single_pixel() {
        let mut fb = Framebuffer::new();
        let golden = fb.pixels.clone();

        // Modifier un pixel spécifique : (50, 100)
        let idx = 100 * SCREEN_W + 50;
        fb.pixels[idx] = 0x00FF0000;

        let result = diff_frame(&fb, &golden, SCREEN_W, SCREEN_H).expect("diff failed");
        assert_eq!(result.different, 1);
        assert_eq!(result.first, Some((50, 100)));
    }

    #[test]
    fn diff_frame_wrong_size() {
        let fb = Framebuffer::new();
        let golden = vec![0u32; 100 * 100];
        let result = diff_frame(&fb, &golden, 100, 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("320x224"));
    }

    #[test]
    fn write_diff_ppm_creates_file() {
        let fb = Framebuffer::new();
        let golden = fb.pixels.clone();
        let path = temp_path("diff.ppm");

        write_diff_ppm(&fb, &golden, &path).expect("write failed");
        assert!(path.exists());

        // Vérifier la taille : header + 320*224*3 octets de données
        let metadata = fs::metadata(&path).unwrap();
        // Header "P6\n320 224\n255\n" = 15 bytes
        assert_eq!(metadata.len(), 15 + (SCREEN_W * SCREEN_H * 3) as u64);

        let _ = fs::remove_file(&path);
    }
}
