use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::types::*;

#[derive(Debug)]
pub enum CartError {
    Io(std::io::Error),
    Zip(String),
    MissingFile(String),
    BadCrc {
        file: String,
        expected: u32,
        got: u32,
    },
    TooBig {
        file: String,
    },
}

impl From<std::io::Error> for CartError {
    fn from(e: std::io::Error) -> Self {
        CartError::Io(e)
    }
}

#[derive(Debug)]
pub struct Cart {
    pub def: &'static CartDef,
    pub p: Vec<u8>,
    pub s: Vec<u8>,
    pub m: Vec<u8>,
    pub m_crypt: Vec<u8>,
    pub v: Vec<u8>,
    pub vb: Vec<u8>,
    pub c: Vec<u8>,
}

pub enum Source {
    Zip(zip::ZipArchive<fs::File>),
    Dir(PathBuf),
}

impl Source {
    pub fn open(path: &Path) -> Result<Source, CartError> {
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            let file = fs::File::open(path)?;
            match zip::ZipArchive::new(file) {
                Ok(archive) => Ok(Source::Zip(archive)),
                Err(e) => Err(CartError::Zip(format!("{}: {}", path.display(), e))),
            }
        } else if path.is_dir() {
            Ok(Source::Dir(path.to_path_buf()))
        } else {
            Err(CartError::MissingFile(path.display().to_string()))
        }
    }

    /// Lit `name` ; si l'archive ne contient pas ce nom, cherche l'entrée dont le CRC32 vaut `crc`
    /// (MAME identifie aussi les ROM par CRC : ex. `neo-sma` pour `green.neo-sma`).
    pub fn read_or_crc(&mut self, name: &str, crc: u32) -> Result<Vec<u8>, CartError> {
        match self.read(name) {
            Err(CartError::MissingFile(_)) => {}
            other => return other,
        }
        if let Source::Zip(archive) = self {
            for i in 0..archive.len() {
                let mut file = archive
                    .by_index(i)
                    .map_err(|e| CartError::Zip(e.to_string()))?;
                if file.crc32() == crc {
                    let mut data = Vec::with_capacity(file.size() as usize);
                    file.read_to_end(&mut data)?;
                    return Ok(data);
                }
            }
        }
        Err(CartError::MissingFile(name.to_string()))
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, CartError> {
        match self {
            Source::Zip(archive) => {
                // Case-insensitive search for zip entry
                let idx = archive
                    .file_names()
                    .position(|n| n.as_ref().is_ok_and(|s| s.eq_ignore_ascii_case(name)));
                match idx {
                    Some(i) => {
                        let mut file = archive
                            .by_index(i)
                            .map_err(|e| CartError::Zip(e.to_string()))?;
                        let mut data = Vec::with_capacity(file.size() as usize);
                        file.read_to_end(&mut data)?;
                        Ok(data)
                    }
                    None => Err(CartError::MissingFile(name.to_string())),
                }
            }
            Source::Dir(dir) => {
                let path = dir.join(name);
                fs::read(path).map_err(CartError::Io)
            }
        }
    }
}

/// Jeux de type « split set » (ex. `puzzldpr` qui réutilise les ROM de `puzzledp`) :
/// quand une ROM manque dans le zip, on la cherche par CRC32 dans les autres zips du
/// même dossier (comme le fait MAME avec les jeux parents).
fn read_from_sibling_zip(path: &Path, crc: u32) -> Option<Vec<u8>> {
    let dir = path.parent()?;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p == path || !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            continue;
        }
        let Ok(file) = fs::File::open(&p) else {
            continue;
        };
        let Ok(mut archive) = zip::ZipArchive::new(file) else {
            continue;
        };
        for i in 0..archive.len() {
            let Ok(mut f) = archive.by_index(i) else {
                continue;
            };
            if f.crc32() == crc {
                let mut data = Vec::with_capacity(f.size() as usize);
                if f.read_to_end(&mut data).is_ok() {
                    return Some(data);
                }
            }
        }
    }
    None
}

pub fn load_cart(path: &Path, def: &'static CartDef, check_crc: bool) -> Result<Cart, CartError> {
    let sizes = def.sizes;
    // Step 1: Allocate regions
    let mut p = vec![0u8; sizes.p as usize];
    let mut s = vec![0u8; sizes.s as usize];
    let mut v = vec![0u8; sizes.v as usize];
    let mut vb = vec![0u8; sizes.vb as usize];
    let mut c = vec![0u8; sizes.c as usize];
    let m_fill = if sizes.m_crypt > 0 { 0xFF } else { 0 };
    let mut m = vec![m_fill; sizes.m as usize];
    let mut m_crypt = vec![0u8; sizes.m_crypt as usize];

    // Step 2: Load each part
    let mut source = Source::open(path)?;
    for part in def.parts {
        let data = match source.read_or_crc(part.file, part.crc) {
            Err(CartError::MissingFile(_)) => read_from_sibling_zip(path, part.crc)
                .ok_or_else(|| CartError::MissingFile(part.file.to_string()))?,
            other => other?,
        };

        if check_crc {
            let got = ng_common::crc::crc32_bytes(&data);
            if got != part.crc {
                return Err(CartError::BadCrc {
                    file: part.file.to_string(),
                    expected: part.crc,
                    got,
                });
            }
        }

        // Extract slice from source data at src_off
        let src_start = part.src_off as usize;
        let src_end = src_start + part.size as usize;
        if src_end > data.len() {
            return Err(CartError::MissingFile(part.file.to_string()));
        }
        let slice = &data[src_start..src_end];

        // Write to destination region based on load type
        let dest = part.dest as usize;
        match part.region {
            Region::P => write_region(&mut p, dest, slice, part.load)?,
            Region::S => write_region(&mut s, dest, slice, part.load)?,
            Region::M => write_region(&mut m, dest, slice, part.load)?,
            Region::MCrypt => write_region(&mut m_crypt, dest, slice, part.load)?,
            Region::V => write_region(&mut v, dest, slice, part.load)?,
            Region::VB => write_region(&mut vb, dest, slice, part.load)?,
            Region::C => write_region(&mut c, dest, slice, part.load)?,
        }
    }

    // Step 3: ROM_RELOAD (m1_reload)
    if let Some((dest, size)) = def.m1_reload {
        m.copy_within(0..size as usize, dest as usize);
    }

    Ok(Cart {
        def,
        p,
        s,
        m,
        m_crypt,
        v,
        vb,
        c,
    })
}

fn write_region(dst: &mut [u8], dest: usize, slice: &[u8], load: Load) -> Result<(), CartError> {
    match load {
        Load::Plain => {
            let end = dest + slice.len();
            if end > dst.len() {
                return Err(CartError::TooBig {
                    file: "".to_string(),
                });
            }
            dst[dest..end].copy_from_slice(slice);
        }
        Load::WordSwap => {
            // Swap bytes of each 16-bit word
            let size = slice.len();
            if !size.is_multiple_of(2) {
                return Err(CartError::TooBig {
                    file: "".to_string(),
                });
            }
            for k in 0..size / 2 {
                dst[dest + 2 * k] = slice[2 * k + 1];
                dst[dest + 2 * k + 1] = slice[2 * k];
            }
        }
        Load::Byte => {
            // Byte i of file -> dest + 2*i (interleaved)
            for (i, &byte) in slice.iter().enumerate() {
                let pos = dest + 2 * i;
                if pos >= dst.len() {
                    return Err(CartError::TooBig {
                        file: "".to_string(),
                    });
                }
                dst[pos] = byte;
            }
        }
        Load::Word32Swap => {
            // 16-bit word k of file (bytes swapped) -> dest + 4*k
            let size = slice.len();
            if !size.is_multiple_of(2) {
                return Err(CartError::TooBig {
                    file: "".to_string(),
                });
            }
            for k in 0..size / 2 {
                dst[dest + 4 * k] = slice[2 * k + 1];
                dst[dest + 4 * k + 1] = slice[2 * k];
            }
        }
    }
    Ok(())
}

pub fn find_def(name: &str) -> Option<&'static CartDef> {
    crate::defs::ALL_CARTS
        .iter()
        .find(|d| d.name == name)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    // Helper: create a CartDef for testing with Box::leak (as specified)
    fn make_test_def(
        name: &'static str,
        parts: Vec<RomPart>,
        sizes: RegionSizes,
        m1_reload: Option<(u32, u32)>,
    ) -> &'static CartDef {
        let def = Box::new(CartDef {
            name,
            title: "Test",
            parts: Box::leak(parts.into_boxed_slice()),
            sizes,
            m1_reload,
            prot: PROT_NONE,
        });
        Box::leak(def)
    }

    #[test]
    fn test_wordswap() {
        // P WordSwap : fichier [0x12,0x34,0x56,0x78] -> P[0..4] = [0x34,0x12,0x78,0x56]
        let parts = vec![RomPart {
            file: "test.p1",
            region: Region::P,
            dest: 0,
            size: 4,
            src_off: 0,
            crc: 0,
            load: Load::WordSwap,
        }];
        let sizes = RegionSizes {
            p: 8,
            s: 0,
            m: 0,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 0,
        };
        let def = make_test_def("test", parts, sizes, None);

        let dir = tempdir().unwrap();
        let mut f = fs::File::create(dir.path().join("test.p1")).unwrap();
        f.write_all(&[0x12, 0x34, 0x56, 0x78]).unwrap();

        let cart = load_cart(dir.path(), def, false).unwrap();
        assert_eq!(&cart.p[0..4], &[0x34, 0x12, 0x78, 0x56]);
    }

    #[test]
    fn test_byte_interleaved() {
        // Byte : c1=[1,2], c2=[9,8] avec dest 0 et 1 -> C[0..4]=[1,9,2,8]
        let parts = vec![
            RomPart {
                file: "c1.c1",
                region: Region::C,
                dest: 0,
                size: 2,
                src_off: 0,
                crc: 0,
                load: Load::Byte,
            },
            RomPart {
                file: "c2.c2",
                region: Region::C,
                dest: 1,
                size: 2,
                src_off: 0,
                crc: 0,
                load: Load::Byte,
            },
        ];
        let sizes = RegionSizes {
            p: 0,
            s: 0,
            m: 0,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 8,
        };
        let def = make_test_def("test", parts, sizes, None);

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("c1.c1"), [1u8, 2]).unwrap();
        fs::write(dir.path().join("c2.c2"), [9u8, 8]).unwrap();

        let cart = load_cart(dir.path(), def, false).unwrap();
        assert_eq!(&cart.c[0..4], &[1, 9, 2, 8]);
    }

    #[test]
    fn test_word32swap() {
        // Word32Swap : p1=[0x11,0x22,0x33,0x44] dest 0 ; p2=[0xAA,0xBB,0xCC,0xDD] dest 2
        // -> P[0..8]=[0x22,0x11,0xBB,0xAA,0x44,0x33,0xDD,0xCC]
        let parts = vec![
            RomPart {
                file: "p1.p1",
                region: Region::P,
                dest: 0,
                size: 4,
                src_off: 0,
                crc: 0,
                load: Load::Word32Swap,
            },
            RomPart {
                file: "p2.p2",
                region: Region::P,
                dest: 2,
                size: 4,
                src_off: 0,
                crc: 0,
                load: Load::Word32Swap,
            },
        ];
        let sizes = RegionSizes {
            p: 16,
            s: 0,
            m: 0,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 0,
        };
        let def = make_test_def("test", parts, sizes, None);

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("p1.p1"), [0x11u8, 0x22, 0x33, 0x44]).unwrap();
        fs::write(dir.path().join("p2.p2"), [0xAAu8, 0xBB, 0xCC, 0xDD]).unwrap();

        let cart = load_cart(dir.path(), def, false).unwrap();
        assert_eq!(
            &cart.p[0..8],
            &[0x22, 0x11, 0xBB, 0xAA, 0x44, 0x33, 0xDD, 0xCC]
        );
    }

    #[test]
    fn test_rom_continue() {
        // ROM_CONTINUE (mslug) : fichier de 8 octets, parts (dest 4,size 4,src_off 0) et (dest 0,size 4,src_off 4) WordSwap
        let parts = vec![
            RomPart {
                file: "test.p1",
                region: Region::P,
                dest: 4,
                size: 4,
                src_off: 0,
                crc: 0,
                load: Load::WordSwap,
            },
            RomPart {
                file: "test.p1",
                region: Region::P,
                dest: 0,
                size: 4,
                src_off: 4,
                crc: 0,
                load: Load::WordSwap,
            },
        ];
        let sizes = RegionSizes {
            p: 8,
            s: 0,
            m: 0,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 0,
        };
        let def = make_test_def("test", parts, sizes, None);

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.p1"), [1u8, 2, 3, 4, 5, 6, 7, 8]).unwrap();

        let cart = load_cart(dir.path(), def, false).unwrap();
        // First part: swap bytes of [1,2,3,4] -> [2,1,4,3] at dest 4
        assert_eq!(&cart.p[4..8], &[2, 1, 4, 3]);
        // Second part: swap bytes of [5,6,7,8] -> [6,5,8,7] at dest 0
        assert_eq!(&cart.p[0..4], &[6, 5, 8, 7]);
    }

    #[test]
    fn test_m_reload() {
        // M reload : m1_reload Some((8,4)) avec M[0..4]=[1,2,3,4] -> M[8..12]=[1,2,3,4]
        let parts = vec![RomPart {
            file: "m1.m1",
            region: Region::M,
            dest: 0,
            size: 4,
            src_off: 0,
            crc: 0,
            load: Load::Plain,
        }];
        let sizes = RegionSizes {
            p: 0,
            s: 0,
            m: 16,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 0,
        };
        let def = make_test_def("test", parts, sizes, Some((8, 4)));

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("m1.m1"), [1u8, 2, 3, 4]).unwrap();

        let cart = load_cart(dir.path(), def, false).unwrap();
        assert_eq!(&cart.m[0..4], &[1, 2, 3, 4]);
        assert_eq!(&cart.m[8..12], &[1, 2, 3, 4]);
    }

    #[test]
    fn test_bad_crc() {
        // CRC faux avec check_crc=true -> Err(BadCrc)
        let parts = vec![RomPart {
            file: "test.p1",
            region: Region::P,
            dest: 0,
            size: 4,
            src_off: 0,
            crc: 0xDEADBEEF, // Wrong CRC
            load: Load::Plain,
        }];
        let sizes = RegionSizes {
            p: 8,
            s: 0,
            m: 0,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 0,
        };
        let def = make_test_def("test", parts, sizes, None);

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.p1"), [1u8, 2, 3, 4]).unwrap();

        let result = load_cart(dir.path(), def, true);
        match result {
            Err(CartError::BadCrc { file, .. }) => assert_eq!(file, "test.p1"),
            other => panic!("Expected BadCrc error, got {:?}", other),
        }
    }

    #[test]
    fn test_source_dir_and_zip_same_bytes() {
        // Source Dir et Source Zip donnent les mêmes octets
        let parts = vec![RomPart {
            file: "data.bin",
            region: Region::P,
            dest: 0,
            size: 4,
            src_off: 0,
            crc: 0,
            load: Load::Plain,
        }];
        let sizes = RegionSizes {
            p: 8,
            s: 0,
            m: 0,
            m_crypt: 0,
            v: 0,
            vb: 0,
            c: 0,
        };
        let def = make_test_def("test", parts, sizes, None);

        // Test with directory
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("data.bin"), [1u8, 2, 3, 4]).unwrap();
        let cart_dir = load_cart(dir.path(), def, false).unwrap();

        // Create zip in memory and write to file
        let mut buf = Vec::new();
        {
            let writer = std::io::Cursor::new(&mut buf);
            let mut zip_writer = zip::ZipWriter::new(writer);
            let opts = zip::write::FileOptions::<()>::default();
            zip_writer.start_file("data.bin", opts).unwrap();
            zip_writer.write_all(&[1u8, 2, 3, 4]).unwrap();
            zip_writer.finish().unwrap();
        }

        let zip_path = dir.path().join("test.zip");
        fs::write(&zip_path, &buf).unwrap();
        let cart_zip = load_cart(&zip_path, def, false).unwrap();

        assert_eq!(cart_dir.p[0..4], cart_zip.p[0..4]);
    }

    #[test]
    fn test_find_def() {
        // find_def("mslug3").unwrap().sizes.c == 0x4000000 ; ALL_CARTS.len()==151 (6 Metal Slug + 128 jeux + 7 CMC42 + 2 CMC50 + 3 SMA + 2 PCM2 + 3 K2K2)
        assert_eq!(crate::defs::ALL_CARTS.len(), 151);
        let mslug3 = find_def("mslug3").expect("mslug3 should exist");
        assert_eq!(mslug3.sizes.c, 0x4000000);
    }
    #[test]
    fn zip_entry_is_found_by_crc_when_name_differs() {
        // MAME identifie les ROM par CRC : `neo-sma` dans l'archive pour `green.neo-sma` attendu.
        let data = [9u8, 8, 7, 6];
        let crc = ng_common::crc::crc32_bytes(&data);
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            w.start_file("neo-sma", zip::write::FileOptions::<()>::default())
                .unwrap();
            w.write_all(&data).unwrap();
            w.finish().unwrap();
        }
        let dir = tempdir().unwrap();
        let zip_path = dir.path().join("t.zip");
        fs::write(&zip_path, &buf).unwrap();
        let mut src = Source::open(&zip_path).unwrap();
        assert_eq!(src.read_or_crc("green.neo-sma", crc).unwrap(), data);
        assert!(matches!(
            src.read_or_crc("green.neo-sma", crc ^ 1),
            Err(CartError::MissingFile(_))
        ));
    }
}
