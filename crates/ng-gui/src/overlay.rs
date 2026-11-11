//! Overlay de performance : barre semi-transparente en surimpression (FPS, temps d'émulation par
//! trame avec courbe, vitesse, coût des filtres, tampon audio). Désactivable, deux niveaux de détail.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, RichText};

/// Nombre de trames gardées pour la courbe.
pub const HISTORY: usize = 120;
/// Durée d'une trame à vitesse réelle, en ms (405 504 mclk à 24 MHz).
pub const BUDGET_MS: f32 = 16.9;
/// Cadence réelle de la machine, en trames par seconde.
pub const REAL_FPS: f32 = 59.19;

const PINK: Color32 = Color32::from_rgb(255, 90, 120);
const YELLOW: Color32 = Color32::from_rgb(240, 220, 60);
const GREEN: Color32 = Color32::from_rgb(40, 200, 60);
const PURPLE: Color32 = Color32::from_rgb(170, 110, 240);
const GREY: Color32 = Color32::from_rgb(190, 190, 190);

/// Coin de la fenêtre où se place l'overlay.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Corner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Corner; 4] = [
        Corner::TopLeft,
        Corner::TopRight,
        Corner::BottomLeft,
        Corner::BottomRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Corner::TopLeft => "Haut gauche",
            Corner::TopRight => "Haut droite",
            Corner::BottomLeft => "Bas gauche",
            Corner::BottomRight => "Bas droite",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Corner::TopLeft => "top-left",
            Corner::TopRight => "top-right",
            Corner::BottomLeft => "bottom-left",
            Corner::BottomRight => "bottom-right",
        }
    }

    pub fn from_name(s: &str) -> Option<Corner> {
        Corner::ALL.into_iter().find(|c| c.name() == s)
    }

    fn align(self) -> Align2 {
        match self {
            Corner::TopLeft => Align2::LEFT_TOP,
            Corner::TopRight => Align2::RIGHT_TOP,
            Corner::BottomLeft => Align2::LEFT_BOTTOM,
            Corner::BottomRight => Align2::RIGHT_BOTTOM,
        }
    }
}

/// Réglages de l'overlay (sauvegardés dans `gui.toml`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OverlayPrefs {
    pub enabled: bool,
    /// `true` = détaillé (courbe, vitesse, filtres, audio), `false` = FPS et ms seulement.
    pub full: bool,
    pub corner: Corner,
}

impl Default for OverlayPrefs {
    fn default() -> Self {
        OverlayPrefs {
            enabled: false,
            full: true,
            corner: Corner::TopLeft,
        }
    }
}

/// État de santé d'une mesure, affiché par une couleur.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Health {
    Good,
    Warn,
    Bad,
}

impl Health {
    fn color(self) -> Color32 {
        match self {
            Health::Good => GREEN,
            Health::Warn => Color32::from_rgb(255, 170, 40),
            Health::Bad => Color32::from_rgb(255, 70, 70),
        }
    }
}

/// Temps d'émulation d'une trame : vert sous 60 % du budget, orange jusqu'au budget, rouge au-delà.
pub fn ms_health(ms: f32) -> Health {
    if ms < BUDGET_MS * 0.6 {
        Health::Good
    } else if ms < BUDGET_MS {
        Health::Warn
    } else {
        Health::Bad
    }
}

/// Vitesse par rapport au temps réel : vert à 98 % et plus, orange à 90 % et plus, rouge en dessous.
pub fn speed_health(pct: f32) -> Health {
    if pct >= 98.0 {
        Health::Good
    } else if pct >= 90.0 {
        Health::Warn
    } else {
        Health::Bad
    }
}

/// Un craquement audio de moins de ce délai garde l'indicateur audio en rouge.
const UNDERRUN_HOLD: Duration = Duration::from_secs(3);

/// Mesures affichées.
#[derive(Default)]
pub struct Perf {
    /// Temps d'émulation de chaque trame (ms), les plus récentes à la fin.
    pub emu_ms: VecDeque<f32>,
    /// Trames par seconde produites par le thread d'émulation.
    pub fps: f32,
    /// Durée lissée d'une image de l'interface (ms).
    pub ui_ms: f32,
    /// Coût lissé des filtres vidéo (ms), `None` si aucun filtre n'est actif.
    pub fx_ms: Option<f32>,
    /// Remplissage du tampon audio, de 0 à 1.
    pub audio_fill: f32,
    /// Nombre de sous-alimentations audio depuis le démarrage.
    pub underruns: u64,
    /// Instant de la dernière sous-alimentation constatée.
    pub underrun_at: Option<Instant>,
}

impl Perf {
    pub fn push_emu(&mut self, ms: f32) {
        if self.emu_ms.len() >= HISTORY {
            self.emu_ms.pop_front();
        }
        self.emu_ms.push_back(ms);
    }

    /// Moyenne des `n` dernières trames.
    pub fn avg(&self, n: usize) -> Option<f32> {
        let k = self.emu_ms.len().min(n);
        (k > 0).then(|| self.emu_ms.iter().rev().take(k).sum::<f32>() / k as f32)
    }

    /// (minimum, maximum) sur tout l'historique.
    pub fn min_max(&self) -> Option<(f32, f32)> {
        let min = self.emu_ms.iter().copied().reduce(f32::min)?;
        let max = self.emu_ms.iter().copied().reduce(f32::max)?;
        Some((min, max))
    }

    /// Vitesse par rapport au temps réel, en %.
    pub fn speed_pct(&self) -> f32 {
        self.fps / REAL_FPS * 100.0
    }

    /// Enregistre l'état audio ; une hausse du compteur date une sous-alimentation.
    pub fn set_audio(&mut self, fill: f32, underruns: u64) {
        if underruns > self.underruns {
            self.underrun_at = Some(Instant::now());
        }
        self.audio_fill = fill;
        self.underruns = underruns;
    }

    /// Rouge pendant 3 s après un craquement, orange si le tampon est presque vide, sinon vert.
    pub fn audio_health(&self) -> Health {
        if self
            .underrun_at
            .is_some_and(|t| t.elapsed() < UNDERRUN_HOLD)
        {
            Health::Bad
        } else if self.audio_fill < 0.15 {
            Health::Warn
        } else {
            Health::Good
        }
    }

    /// Efface les mesures (changement de jeu, arrêt).
    pub fn reset(&mut self) {
        self.emu_ms.clear();
        self.fps = 0.0;
        self.audio_fill = 0.0;
        self.underrun_at = None;
    }

    /// Lisse une durée d'image de l'interface.
    pub fn smooth_ui(&mut self, dt_ms: f32) {
        self.ui_ms = if self.ui_ms == 0.0 {
            dt_ms
        } else {
            self.ui_ms * 0.9 + dt_ms * 0.1
        };
    }

    /// Lisse le coût des filtres.
    pub fn smooth_fx(&mut self, ms: f32) {
        self.fx_ms = Some(self.fx_ms.map_or(ms, |old| old * 0.8 + ms * 0.2));
    }
}

/// Situation affichée à la place des mesures quand l'émulation ne tourne pas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunState {
    Running,
    Paused,
    Stopped,
}

/// Valeur à largeur fixe (alignée à droite) : la barre ne change jamais de taille quand les
/// chiffres passent de 9,9 à 10,2 ms ou de 5 à 100 %.
fn num(v: f32, width: usize, decimals: usize) -> String {
    let max = 10f32.powi(width as i32 - decimals as i32 - 1) - 1.0;
    format!("{:>width$.decimals$}", v.clamp(0.0, max))
}

fn seg(ui: &mut egui::Ui, label: &str, color: Color32, value: String) {
    ui.label(RichText::new(label).monospace().strong().color(color));
    ui.label(RichText::new(value).monospace().color(Color32::WHITE));
}

/// Comme `seg`, avec une valeur colorée selon la santé de la mesure.
fn seg_h(ui: &mut egui::Ui, label: &str, color: Color32, value: String, h: Health) {
    ui.label(RichText::new(label).monospace().strong().color(color));
    ui.label(RichText::new(value).monospace().strong().color(h.color()));
}

fn sep(ui: &mut egui::Ui) {
    ui.label(RichText::new("|").monospace().color(Color32::DARK_GRAY));
}

/// Courbe des temps d'émulation : vert sous le budget, rouge au-dessus ; la ligne grise = budget.
fn graph(ui: &mut egui::Ui, perf: &Perf) {
    let size = egui::vec2(HISTORY as f32 * 1.4, 24.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::from_black_alpha(120));
    let scale = |ms: f32| rect.bottom() - (ms / (2.0 * BUDGET_MS)).clamp(0.0, 1.0) * rect.height();
    let by = scale(BUDGET_MS);
    p.line_segment(
        [egui::pos2(rect.left(), by), egui::pos2(rect.right(), by)],
        egui::Stroke::new(1.0, Color32::from_gray(90)),
    );
    let step = rect.width() / HISTORY as f32;
    let mut prev: Option<egui::Pos2> = None;
    for (i, &ms) in perf.emu_ms.iter().enumerate() {
        let pt = egui::pos2(rect.left() + i as f32 * step, scale(ms));
        if let Some(a) = prev {
            let c = if ms > BUDGET_MS { Color32::RED } else { GREEN };
            p.line_segment([a, pt], egui::Stroke::new(1.2, c));
        }
        prev = Some(pt);
    }
}

/// Dessine l'overlay. `top_pad` / `bottom_pad` : marges (points) pour ne pas recouvrir la barre de
/// menus ni la barre d'état ; `scale` : texte d'échelle, ex. « x4 1280x896 ».
pub fn show(
    ctx: &egui::Context,
    prefs: &OverlayPrefs,
    perf: &Perf,
    state: RunState,
    scale: Option<&str>,
    (top_pad, bottom_pad): (f32, f32),
) {
    if !prefs.enabled {
        return;
    }
    let offset = match prefs.corner {
        Corner::TopLeft => egui::vec2(8.0, top_pad),
        Corner::TopRight => egui::vec2(-8.0, top_pad),
        Corner::BottomLeft => egui::vec2(8.0, -bottom_pad),
        Corner::BottomRight => egui::vec2(-8.0, -bottom_pad),
    };
    egui::Area::new(egui::Id::NULL.with("perf_overlay"))
        .order(egui::Order::Foreground)
        .anchor(prefs.corner.align(), offset)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(215))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        match state {
                            RunState::Stopped => {
                                seg(ui, "ARRÊT", PINK, String::new());
                                return;
                            }
                            RunState::Paused => seg(ui, "PAUSE", PINK, " ".repeat(5)),
                            RunState::Running => seg_h(
                                ui,
                                "FPS  ",
                                Color32::WHITE,
                                num(perf.fps, 5, 1),
                                speed_health(perf.speed_pct()),
                            ),
                        }
                        let avg = perf.avg(30);
                        let emu =
                            avg.map_or_else(|| " ".repeat(8), |v| format!("{} ms", num(v, 5, 2)));
                        sep(ui);
                        seg_h(ui, "EMU", YELLOW, emu, avg.map_or(Health::Good, ms_health));
                        if !prefs.full {
                            return;
                        }
                        let (lo, hi) = perf.min_max().unwrap_or((0.0, 0.0));
                        ui.label(
                            RichText::new(format!("min {} max {}", num(lo, 5, 1), num(hi, 5, 1)))
                                .monospace()
                                .color(GREY),
                        );
                        graph(ui, perf);
                        sep(ui);
                        seg_h(
                            ui,
                            "VIT",
                            PINK,
                            format!("{}%", num(perf.speed_pct(), 4, 0)),
                            speed_health(perf.speed_pct()),
                        );
                        sep(ui);
                        seg(ui, "UI", GREEN, format!("{} ms", num(perf.ui_ms, 5, 1)));
                        // toujours présent (tiret sans filtre) : la largeur ne change pas
                        let fx = perf.fx_ms.map_or_else(
                            || format!("{:>5} ms", "-"),
                            |v| format!("{} ms", num(v, 5, 1)),
                        );
                        seg(ui, "FILTRE", GREEN, fx);
                        sep(ui);
                        seg_h(
                            ui,
                            "AUDIO",
                            PURPLE,
                            format!("{}%", num((perf.audio_fill * 100.0).min(100.0), 4, 0)),
                            perf.audio_health(),
                        );
                        if let Some(s) = scale {
                            sep(ui);
                            ui.label(RichText::new(s).monospace().color(GREY));
                        }
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_capped_and_averaged() {
        let mut p = Perf::default();
        assert_eq!(p.avg(10), None);
        for i in 0..(HISTORY + 20) {
            p.push_emu(i as f32);
        }
        assert_eq!(p.emu_ms.len(), HISTORY);
        assert_eq!(p.emu_ms.front().copied(), Some(20.0));
        // moyenne des 3 dernières : 137, 138, 139
        assert_eq!(p.avg(3), Some(138.0));
        assert_eq!(p.min_max(), Some((20.0, 139.0)));
    }

    #[test]
    fn speed_is_relative_to_real_time() {
        let p = Perf {
            fps: REAL_FPS,
            ..Perf::default()
        };
        assert!((p.speed_pct() - 100.0).abs() < 1e-3);
        let p = Perf {
            fps: REAL_FPS / 2.0,
            ..Perf::default()
        };
        assert!((p.speed_pct() - 50.0).abs() < 1e-3);
    }

    #[test]
    fn smoothing_starts_from_the_first_value() {
        let mut p = Perf::default();
        p.smooth_ui(10.0);
        assert_eq!(p.ui_ms, 10.0);
        p.smooth_ui(20.0);
        assert!((p.ui_ms - 11.0).abs() < 1e-4);
        p.smooth_fx(5.0);
        assert_eq!(p.fx_ms, Some(5.0));
    }

    #[test]
    fn health_thresholds() {
        assert_eq!(ms_health(4.0), Health::Good);
        assert_eq!(ms_health(12.0), Health::Warn);
        assert_eq!(ms_health(20.0), Health::Bad);
        assert_eq!(speed_health(100.0), Health::Good);
        assert_eq!(speed_health(93.0), Health::Warn);
        assert_eq!(speed_health(50.0), Health::Bad);
    }

    #[test]
    fn audio_turns_red_after_an_underrun() {
        let mut p = Perf::default();
        p.set_audio(0.8, 0);
        assert_eq!(p.audio_health(), Health::Good);
        p.set_audio(0.05, 0);
        assert_eq!(p.audio_health(), Health::Warn);
        p.set_audio(0.5, 1);
        assert_eq!(p.audio_health(), Health::Bad);
        // le compteur ne bouge plus : toujours rouge pendant le délai, pas de nouveau déclencheur
        p.set_audio(0.5, 1);
        assert_eq!(p.audio_health(), Health::Bad);
        p.underrun_at = Some(Instant::now() - Duration::from_secs(10));
        assert_eq!(p.audio_health(), Health::Good);
        p.reset();
        assert_eq!(p.underrun_at, None);
    }

    #[test]
    fn numbers_keep_a_fixed_width() {
        for v in [0.0, 0.4, 9.9, 10.2, 99.94, 150.0, 1e9, -3.0] {
            assert_eq!(num(v, 5, 1).len(), 5, "{v}");
            assert_eq!(num(v, 5, 2).len(), 5, "{v}");
        }
        for v in [0.0, 5.0, 100.0, 12345.0] {
            assert_eq!(num(v, 4, 0).len(), 4, "{v}");
        }
        assert_eq!(num(9.9, 5, 1), "  9.9");
        assert_eq!(num(10.2, 5, 1), " 10.2");
    }

    #[test]
    fn corner_names_round_trip() {
        for c in Corner::ALL {
            assert_eq!(Corner::from_name(c.name()), Some(c));
        }
        assert_eq!(Corner::from_name("milieu"), None);
        assert!(!OverlayPrefs::default().enabled);
    }
}
