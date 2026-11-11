//! Préférences de l'interface, sauvegardées dans `gui.toml` (à côté de `neogeo.toml`).
//!
//! `neogeo.toml` garde les chemins, le modèle et le BIOS ; `gui.toml` garde l'affichage, les
//! touches et l'état du débogueur. Un fichier absent ou invalide donne les valeurs par défaut.

use crate::postfx::{Fx, Mask, ScanMode};
use std::path::{Path, PathBuf};

/// Mode d'échelle de l'image.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ScaleMode {
    /// Plus grand facteur entier tenant dans la fenêtre.
    #[default]
    Auto,
    Fixed(u32),
}

/// Forme des pixels à l'affichage.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Aspect {
    /// Pixels carrés (pixel perfect).
    #[default]
    Square,
    /// Image étirée en 4:3 comme sur un téléviseur (non pixel perfect).
    Tv4x3,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DisplayPrefs {
    pub scale: ScaleMode,
    pub fullscreen: bool,
    pub aspect: Aspect,
    /// Lissage bilinéaire (désactivé par défaut : pixels nets).
    pub smooth: bool,
}

/// Assignations action -> nom de touche / de bouton ; vide = valeurs par défaut.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct InputPrefs {
    pub keys: Vec<(String, String)>,
    pub pad: Vec<(String, String)>,
    /// Sensibilité de la souris pour trackball / molette, en % (`None` = 100).
    pub mouse_sens: Option<u32>,
    /// DIP switches MVS (`None` = 0xFF, tous « off »).
    pub dipsw: Option<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugPrefs {
    pub open: bool,
    /// Onglet actif du débogueur.
    pub tab: String,
    /// Région affichée par l'onglet Mémoire.
    pub region: String,
    pub show_status_bar: bool,
}

impl Default for DebugPrefs {
    fn default() -> Self {
        Self {
            open: false,
            tab: "cpu".into(),
            region: "work".into(),
            show_status_bar: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Prefs {
    pub display: DisplayPrefs,
    pub input: InputPrefs,
    pub debug: DebugPrefs,
    pub fx: Fx,
}

/// Chemin de `gui.toml` : même dossier que le fichier de configuration.
pub fn prefs_path(cfg_path: &Path) -> PathBuf {
    cfg_path.with_file_name("gui.toml")
}

/// Écrit `text` dans `path` sans jamais laisser de fichier tronqué (`.tmp` puis renommage).
pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text).map_err(|e| format!("{} : {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{} : {e}", path.display())
    })
}

fn scale_to_str(s: ScaleMode) -> String {
    match s {
        ScaleMode::Auto => "auto".into(),
        ScaleMode::Fixed(n) => format!("fixed:{n}"),
    }
}

fn parse_scale(v: &str) -> Option<ScaleMode> {
    if v == "auto" {
        return Some(ScaleMode::Auto);
    }
    let n: u32 = v.strip_prefix("fixed:")?.parse().ok()?;
    (1..=16).contains(&n).then_some(ScaleMode::Fixed(n))
}

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Un nom (action, touche, onglet) sûr à écrire tel quel entre guillemets.
fn is_plain(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-:. ".contains(c))
}

fn scan_mode_str(m: ScanMode) -> &'static str {
    match m {
        ScanMode::Off => "off",
        ScanMode::Standard => "standard",
        ScanMode::Adaptive => "adaptive",
    }
}

fn mask_str(m: Mask) -> &'static str {
    match m {
        Mask::None => "none",
        Mask::Trinitron => "trinitron",
        Mask::FadedYellow => "faded",
    }
}

impl Prefs {
    pub fn to_toml(&self) -> String {
        let d = &self.display;
        let mut s = format!(
            "[display]\nscale = \"{}\"\nfullscreen = {}\naspect = \"{}\"\nsmooth = {}\n",
            scale_to_str(d.scale),
            d.fullscreen,
            match d.aspect {
                Aspect::Square => "square",
                Aspect::Tv4x3 => "4:3",
            },
            d.smooth
        );
        if self.input.mouse_sens.is_some() || self.input.dipsw.is_some() {
            s.push_str("\n[input]\n");
            if let Some(n) = self.input.mouse_sens {
                s.push_str(&format!("mouse_sensitivity = {n}\n"));
            }
            if let Some(n) = self.input.dipsw {
                s.push_str(&format!("dipsw = {n}\n"));
            }
        }
        for (name, list) in [("keys", &self.input.keys), ("pad", &self.input.pad)] {
            s.push_str(&format!("\n[input.{name}]\n"));
            for (action, value) in list {
                if is_plain(action) && is_plain(value) && !action.contains(' ') {
                    s.push_str(&format!("{action} = \"{value}\"\n"));
                }
            }
        }
        let f = &self.fx;
        s.push_str(&format!(
            "\n[fx]\ninterp = {}\nscanlines = \"{}\"\nscan = {}\nscan_dark = {}\nscan_bright = {}\ngamma = {}\nmask = \"{}\"\nmask_pct = {}\n",
            f.interp,
            scan_mode_str(f.scan_mode),
            f.scan,
            f.scan_dk,
            f.scan_br,
            f.gamma,
            mask_str(f.mask),
            f.mask_pct
        ));
        let g = &self.debug;
        s.push_str(&format!(
            "\n[debug]\nopen = {}\ntab = \"{}\"\nregion = \"{}\"\nshow_status_bar = {}\n",
            g.open, g.tab, g.region, g.show_status_bar
        ));
        s
    }

    /// Lit un `gui.toml` ; toute ligne inconnue ou invalide est ignorée (valeur par défaut).
    pub fn from_toml(text: &str) -> Prefs {
        let mut p = Prefs::default();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_string();
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            match (section.as_str(), k) {
                ("display", "scale") => {
                    if let Some(s) = parse_scale(v) {
                        p.display.scale = s;
                    }
                }
                ("display", "fullscreen") => {
                    if let Some(b) = parse_bool(v) {
                        p.display.fullscreen = b;
                    }
                }
                ("display", "aspect") => match v {
                    "square" => p.display.aspect = Aspect::Square,
                    "4:3" => p.display.aspect = Aspect::Tv4x3,
                    _ => {}
                },
                ("display", "smooth") => {
                    if let Some(b) = parse_bool(v) {
                        p.display.smooth = b;
                    }
                }
                ("input", "mouse_sensitivity") => {
                    if let Ok(n) = v.parse::<u32>() {
                        p.input.mouse_sens = Some(n.clamp(10, 500));
                    }
                }
                ("input", "dipsw") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.input.dipsw = Some(n);
                    }
                }
                ("input.keys", _) if is_plain(k) && is_plain(v) => {
                    p.input.keys.push((k.to_string(), v.to_string()));
                }
                ("input.pad", _) if is_plain(k) && is_plain(v) => {
                    p.input.pad.push((k.to_string(), v.to_string()));
                }
                ("fx", "interp") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.fx.interp = n.min(100);
                    }
                }
                ("fx", "scanlines") => match v {
                    "off" => p.fx.scan_mode = ScanMode::Off,
                    "standard" => p.fx.scan_mode = ScanMode::Standard,
                    "adaptive" => p.fx.scan_mode = ScanMode::Adaptive,
                    _ => {}
                },
                ("fx", "scan") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.fx.scan = n.min(95);
                    }
                }
                ("fx", "scan_dark") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.fx.scan_dk = n.min(95);
                    }
                }
                ("fx", "scan_bright") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.fx.scan_br = n.min(95);
                    }
                }
                ("fx", "gamma") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.fx.gamma = n.clamp(100, 200);
                    }
                }
                ("fx", "mask") => match v {
                    "none" => p.fx.mask = Mask::None,
                    "trinitron" => p.fx.mask = Mask::Trinitron,
                    "faded" => p.fx.mask = Mask::FadedYellow,
                    _ => {}
                },
                ("fx", "mask_pct") => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.fx.mask_pct = n.min(100);
                    }
                }
                ("debug", "open") => {
                    if let Some(b) = parse_bool(v) {
                        p.debug.open = b;
                    }
                }
                ("debug", "tab") if is_plain(v) => p.debug.tab = v.to_string(),
                ("debug", "region") if is_plain(v) => p.debug.region = v.to_string(),
                ("debug", "show_status_bar") => {
                    if let Some(b) = parse_bool(v) {
                        p.debug.show_status_bar = b;
                    }
                }
                _ => {}
            }
        }
        p
    }

    /// Charge les préférences ; fichier absent ou illisible = valeurs par défaut.
    pub fn load(path: &Path) -> Prefs {
        std::fs::read_to_string(path)
            .map(|t| Prefs::from_toml(&t))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        write_atomic(path, &self.to_toml())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom() -> Prefs {
        Prefs {
            display: DisplayPrefs {
                scale: ScaleMode::Fixed(3),
                fullscreen: true,
                aspect: Aspect::Tv4x3,
                smooth: true,
            },
            input: InputPrefs {
                keys: vec![
                    ("P1A".into(), "Q".into()),
                    ("Start1".into(), "Space".into()),
                ],
                pad: vec![("P1B".into(), "North".into())],
                mouse_sens: Some(150),
                dipsw: Some(0xBF),
            },
            debug: DebugPrefs {
                open: true,
                tab: "sprites".into(),
                region: "vram".into(),
                show_status_bar: false,
            },
            fx: Fx::preset(crate::postfx::Preset::SonyTrinitron),
        }
    }

    #[test]
    fn round_trip() {
        let p = custom();
        assert_eq!(Prefs::from_toml(&p.to_toml()), p);
        let d = Prefs::default();
        assert_eq!(Prefs::from_toml(&d.to_toml()), d);
    }

    #[test]
    fn missing_or_garbage_gives_defaults() {
        assert_eq!(
            Prefs::load(Path::new("/inexistant/gui.toml")),
            Prefs::default()
        );
        assert_eq!(
            Prefs::from_toml("n'importe quoi\n[[[\n= = ="),
            Prefs::default()
        );
    }

    #[test]
    fn unknown_keys_are_ignored_and_scale_parses() {
        let p = Prefs::from_toml(
            "[display]\nfoo = 1\nscale = \"fixed:3\"\nfullscreen = peut-être\n[autre]\nscale = \"fixed:9\"\n",
        );
        assert_eq!(p.display.scale, ScaleMode::Fixed(3));
        assert!(!p.display.fullscreen);
        assert_eq!(
            Prefs::from_toml("[display]\nscale = \"auto\"")
                .display
                .scale,
            ScaleMode::Auto
        );
        assert_eq!(
            Prefs::from_toml("[display]\nscale = \"fixed:0\"")
                .display
                .scale,
            ScaleMode::Auto
        );
    }

    #[test]
    fn save_is_atomic_and_reloads() {
        let dir = std::env::temp_dir().join(format!("ng_gui_prefs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = prefs_path(&dir.join("neogeo.toml"));
        assert!(path.ends_with("gui.toml"));
        let p = custom();
        p.save(&path).expect("sauvegarde");
        assert!(!dir.join("gui.toml.tmp").exists());
        assert_eq!(Prefs::load(&path), p);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
