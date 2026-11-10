//! Barre d'état : jeu, BIOS, modèle, trame, fps, échelle, marche/pause.

/// Ce qu'affiche la barre d'état ; `None` = information indisponible.
#[derive(Default)]
pub struct StatusInfo<'a> {
    pub game: Option<&'a str>,
    pub bios: Option<&'a str>,
    pub model: Option<&'a str>,
    pub frame: Option<u64>,
    pub fps: Option<f32>,
    pub scale: Option<&'a str>,
    pub paused: bool,
}

/// Valeur affichée quand une information manque.
const MISSING: &str = "–";

pub fn format_status(s: &StatusInfo) -> String {
    let or = |v: Option<&str>| v.unwrap_or(MISSING).to_string();
    [
        or(s.game),
        or(s.bios),
        or(s.model),
        s.frame
            .map_or_else(|| MISSING.into(), |f| format!("trame {f}")),
        s.fps
            .map_or_else(|| MISSING.into(), |f| format!("{f:.1} fps")),
        or(s.scale),
        if s.paused { "PAUSE" } else { "MARCHE" }.to_string(),
    ]
    .join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_status() {
        let s = StatusInfo {
            game: Some("mslug"),
            bios: Some("euro"),
            model: Some("MVS"),
            frame: Some(120),
            fps: Some(59.19),
            scale: Some("x3 (960x672)"),
            paused: false,
        };
        assert_eq!(
            format_status(&s),
            "mslug · euro · MVS · trame 120 · 59.2 fps · x3 (960x672) · MARCHE"
        );
    }

    #[test]
    fn missing_values_show_a_dash() {
        let s = StatusInfo {
            paused: true,
            ..StatusInfo::default()
        };
        assert_eq!(format_status(&s), "– · – · – · – · – · – · PAUSE");
    }
}
