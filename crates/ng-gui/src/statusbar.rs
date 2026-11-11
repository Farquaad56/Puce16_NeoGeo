//! Barre d'état : jeu, BIOS, modèle, temps par trame, fps, échelle, marche/pause/arrêt.

/// Ce qu'affiche la barre d'état ; `None` = information indisponible.
#[derive(Default)]
pub struct StatusInfo<'a> {
    pub game: Option<&'a str>,
    pub bios: Option<&'a str>,
    pub model: Option<&'a str>,
    /// Temps d'émulation moyen par trame, en ms.
    pub ms: Option<f32>,
    pub fps: Option<f32>,
    pub scale: Option<&'a str>,
    pub paused: bool,
    pub stopped: bool,
}

/// Valeur affichée quand une information manque.
const MISSING: &str = "–";

pub fn format_status(s: &StatusInfo) -> String {
    let or = |v: Option<&str>| v.unwrap_or(MISSING).to_string();
    [
        or(s.game),
        or(s.bios),
        or(s.model),
        s.ms.map_or_else(|| MISSING.into(), |m| format!("{m:.2} ms/trame")),
        s.fps
            .map_or_else(|| MISSING.into(), |f| format!("{f:.1} fps")),
        or(s.scale),
        match (s.stopped, s.paused) {
            (true, _) => "ARRÊT",
            (false, true) => "PAUSE",
            (false, false) => "MARCHE",
        }
        .to_string(),
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
            ms: Some(4.103),
            fps: Some(59.19),
            scale: Some("x3 (960x672)"),
            paused: false,
            stopped: false,
        };
        assert_eq!(
            format_status(&s),
            "mslug · euro · MVS · 4.10 ms/trame · 59.2 fps · x3 (960x672) · MARCHE"
        );
    }

    #[test]
    fn stopped_state() {
        let s = StatusInfo {
            stopped: true,
            paused: true,
            ..StatusInfo::default()
        };
        assert_eq!(format_status(&s), "– · – · – · – · – · – · ARRÊT");
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
