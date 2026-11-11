//! Entrées : clavier (egui) et manettes (gilrs) vers `PadState` / `SystemButtons` (actifs-bas).

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, Key};
use gilrs::Button;
use ng_common::{PadState, SystemButtons};

use crate::prefs::InputPrefs;

// Bits d'un PadState (voir ng-common/input.rs) : D C B A Droite Gauche Bas Haut.
const UP: u8 = 1;
const DOWN: u8 = 1 << 1;
const LEFT: u8 = 1 << 2;
const RIGHT: u8 = 1 << 3;
const BTN_A: u8 = 1 << 4;
const BTN_B: u8 = 1 << 5;
const BTN_C: u8 = 1 << 6;
const BTN_D: u8 = 1 << 7;
// Bits d'un SystemButtons.
const START1: u8 = 1;
const SELECT1: u8 = 1 << 1;
const START2: u8 = 1 << 2;
const SELECT2: u8 = 1 << 3;
const COIN1: u8 = 1 << 4;
const COIN2: u8 = 1 << 5;
const SERVICE: u8 = 1 << 6;
const TEST: u8 = 1 << 7;

/// Seuil du stick analogique.
pub const STICK_THRESHOLD: f32 = 0.5;

/// Une entrée de la machine que l'on peut assigner à une touche ou à un bouton de manette.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Action {
    P1Up,
    P1Down,
    P1Left,
    P1Right,
    P1A,
    P1B,
    P1C,
    P1D,
    P2Up,
    P2Down,
    P2Left,
    P2Right,
    P2A,
    P2B,
    P2C,
    P2D,
    Start1,
    Select1,
    Start2,
    Select2,
    Coin1,
    Coin2,
    Service,
    Test,
    // Panneau mahjong (jeux `Controller::Mahjong`) : rangée MJ.0 (A-G), MJ.1 (H-N), MJ.2.
    MjA,
    MjB,
    MjC,
    MjD,
    MjE,
    MjF,
    MjG,
    MjH,
    MjI,
    MjJ,
    MjK,
    MjL,
    MjM,
    MjN,
    MjPon,
    MjChi,
    MjKan,
    MjRon,
    MjReach,
}

/// Nombre total d'actions (joueurs, système, panneau mahjong).
pub const N_ACTIONS: usize = 43;

/// Où va une action : bits d'une manette ou bits système.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Pad(usize, u8),
    Sys(u8),
    /// Rangée (0-2) et bit du panneau mahjong.
    Mj(usize, u8),
}

/// (action, nom dans gui.toml, libellé, cible, touche par défaut, bouton par défaut).
type Row = (
    Action,
    &'static str,
    &'static str,
    Target,
    Option<Key>,
    Option<Button>,
);

#[rustfmt::skip]
const TABLE: [Row; N_ACTIONS] = {
    use Action::*;
    use Target::{Mj, Pad, Sys};
    [
        (P1Up, "P1Up", "J1 Haut", Pad(0, UP), Some(Key::ArrowUp), Some(Button::DPadUp)),
        (P1Down, "P1Down", "J1 Bas", Pad(0, DOWN), Some(Key::ArrowDown), Some(Button::DPadDown)),
        (P1Left, "P1Left", "J1 Gauche", Pad(0, LEFT), Some(Key::ArrowLeft), Some(Button::DPadLeft)),
        (P1Right, "P1Right", "J1 Droite", Pad(0, RIGHT), Some(Key::ArrowRight), Some(Button::DPadRight)),
        (P1A, "P1A", "J1 A", Pad(0, BTN_A), Some(Key::Z), Some(Button::South)),
        (P1B, "P1B", "J1 B", Pad(0, BTN_B), Some(Key::X), Some(Button::East)),
        (P1C, "P1C", "J1 C", Pad(0, BTN_C), Some(Key::A), Some(Button::West)),
        (P1D, "P1D", "J1 D", Pad(0, BTN_D), Some(Key::S), Some(Button::North)),
        (P2Up, "P2Up", "J2 Haut", Pad(1, UP), Some(Key::I), Some(Button::DPadUp)),
        (P2Down, "P2Down", "J2 Bas", Pad(1, DOWN), Some(Key::K), Some(Button::DPadDown)),
        (P2Left, "P2Left", "J2 Gauche", Pad(1, LEFT), Some(Key::J), Some(Button::DPadLeft)),
        (P2Right, "P2Right", "J2 Droite", Pad(1, RIGHT), Some(Key::L), Some(Button::DPadRight)),
        (P2A, "P2A", "J2 A", Pad(1, BTN_A), Some(Key::G), Some(Button::South)),
        (P2B, "P2B", "J2 B", Pad(1, BTN_B), Some(Key::H), Some(Button::East)),
        (P2C, "P2C", "J2 C", Pad(1, BTN_C), Some(Key::T), Some(Button::West)),
        (P2D, "P2D", "J2 D", Pad(1, BTN_D), Some(Key::Y), Some(Button::North)),
        (Start1, "Start1", "Start J1", Sys(START1), Some(Key::Enter), Some(Button::Start)),
        (Select1, "Select1", "Select J1", Sys(SELECT1), None, Some(Button::Select)),
        (Start2, "Start2", "Start J2", Sys(START2), Some(Key::P), Some(Button::Start)),
        (Select2, "Select2", "Select J2", Sys(SELECT2), Some(Key::Tab), Some(Button::Select)),
        (Coin1, "Coin1", "Pièce 1", Sys(COIN1), Some(Key::Num5), None),
        (Coin2, "Coin2", "Pièce 2", Sys(COIN2), Some(Key::Num6), None),
        (Service, "Service", "Service", Sys(SERVICE), Some(Key::Num9), None),
        (Test, "Test", "Test", Sys(TEST), Some(Key::F2), None),
        (MjA, "MjA", "Mahjong A", Mj(0, 1), Some(Key::A), None),
        (MjB, "MjB", "Mahjong B", Mj(0, 1 << 1), Some(Key::B), None),
        (MjC, "MjC", "Mahjong C", Mj(0, 1 << 2), Some(Key::C), None),
        (MjD, "MjD", "Mahjong D", Mj(0, 1 << 3), Some(Key::D), None),
        (MjE, "MjE", "Mahjong E", Mj(0, 1 << 4), Some(Key::E), None),
        (MjF, "MjF", "Mahjong F", Mj(0, 1 << 5), Some(Key::F), None),
        (MjG, "MjG", "Mahjong G", Mj(0, 1 << 6), Some(Key::G), None),
        (MjH, "MjH", "Mahjong H", Mj(1, 1), Some(Key::H), None),
        (MjI, "MjI", "Mahjong I", Mj(1, 1 << 1), Some(Key::I), None),
        (MjJ, "MjJ", "Mahjong J", Mj(1, 1 << 2), Some(Key::J), None),
        (MjK, "MjK", "Mahjong K", Mj(1, 1 << 3), Some(Key::K), None),
        (MjL, "MjL", "Mahjong L", Mj(1, 1 << 4), Some(Key::L), None),
        (MjM, "MjM", "Mahjong M", Mj(1, 1 << 5), Some(Key::M), None),
        (MjN, "MjN", "Mahjong N", Mj(1, 1 << 6), Some(Key::N), None),
        (MjPon, "MjPon", "Pon", Mj(2, 1), Some(Key::P), None),
        (MjChi, "MjChi", "Chi", Mj(2, 1 << 1), Some(Key::C), None),
        (MjKan, "MjKan", "Kan", Mj(2, 1 << 2), Some(Key::K), None),
        (MjRon, "MjRon", "Ron", Mj(2, 1 << 3), Some(Key::R), None),
        (MjReach, "MjReach", "Ri-chi", Mj(2, 1 << 4), Some(Key::I), None),
    ]
};

/// Boutons de manette assignables et leur nom dans gui.toml.
const BUTTONS: [(Button, &str); 19] = [
    (Button::South, "South"),
    (Button::East, "East"),
    (Button::North, "North"),
    (Button::West, "West"),
    (Button::C, "C"),
    (Button::Z, "Z"),
    (Button::LeftTrigger, "LeftTrigger"),
    (Button::LeftTrigger2, "LeftTrigger2"),
    (Button::RightTrigger, "RightTrigger"),
    (Button::RightTrigger2, "RightTrigger2"),
    (Button::Select, "Select"),
    (Button::Start, "Start"),
    (Button::Mode, "Mode"),
    (Button::LeftThumb, "LeftThumb"),
    (Button::RightThumb, "RightThumb"),
    (Button::DPadUp, "DPadUp"),
    (Button::DPadDown, "DPadDown"),
    (Button::DPadLeft, "DPadLeft"),
    (Button::DPadRight, "DPadRight"),
];

/// Valeur de gui.toml pour « aucune assignation ».
const NONE: &str = "none";

impl Action {
    pub const ALL: [Action; N_ACTIONS] = {
        let mut all = [Action::P1Up; N_ACTIONS];
        let mut i = 0;
        while i < N_ACTIONS {
            all[i] = TABLE[i].0;
            i += 1;
        }
        all
    };

    fn row(self) -> &'static Row {
        &TABLE[self as usize]
    }

    /// Nom stable écrit dans gui.toml.
    pub fn name(self) -> &'static str {
        self.row().1
    }

    pub fn from_name(name: &str) -> Option<Action> {
        TABLE.iter().find(|r| r.1 == name).map(|r| r.0)
    }

    /// Libellé affiché.
    pub fn label(self) -> &'static str {
        self.row().2
    }

    pub fn default_key(self) -> Option<Key> {
        self.row().4
    }

    pub fn default_button(self) -> Option<Button> {
        self.row().5
    }

    /// Manette (0 ou 1) qui porte les boutons de cette action.
    pub fn gamepad(self) -> usize {
        match self.row().3 {
            Target::Pad(n, _) => n,
            Target::Mj(..) => 0,
            Target::Sys(_) => usize::from(matches!(
                self,
                Action::Start2 | Action::Select2 | Action::Coin2
            )),
        }
    }

    /// Action du panneau mahjong (modes de jeu distincts : les doublons de touches n'y sont pas
    /// échangés avec les actions joystick).
    pub fn is_mahjong(self) -> bool {
        matches!(self.row().3, Target::Mj(..))
    }

    /// 0 = joueur 1, 1 = joueur 2, 2 = système (pièces, service, test), 3 = panneau mahjong.
    pub fn group(self) -> usize {
        if self.is_mahjong() {
            return 3;
        }
        match self {
            Action::Coin1 | Action::Coin2 | Action::Service | Action::Test => 2,
            _ => self.gamepad(),
        }
    }
}

/// Nom d'une touche dans gui.toml (nom egui).
pub fn key_name(key: Key) -> String {
    key.name().to_string()
}

pub fn parse_key(name: &str) -> Option<Key> {
    Key::from_name(name)
}

pub fn button_name(b: Button) -> &'static str {
    BUTTONS
        .iter()
        .find(|(x, _)| *x == b)
        .map_or("?", |(_, n)| n)
}

pub fn parse_button(name: &str) -> Option<Button> {
    BUTTONS.iter().find(|(_, n)| *n == name).map(|(b, _)| *b)
}

/// Assignations action -> touche et action -> bouton de manette.
#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    keys: HashMap<Action, Key>,
    pad: HashMap<Action, Button>,
}

impl Default for Bindings {
    fn default() -> Self {
        let mut b = Bindings {
            keys: HashMap::new(),
            pad: HashMap::new(),
        };
        b.reset(|_| true);
        b
    }
}

impl Bindings {
    pub fn key(&self, a: Action) -> Option<Key> {
        self.keys.get(&a).copied()
    }

    pub fn button(&self, a: Action) -> Option<Button> {
        self.pad.get(&a).copied()
    }

    /// Assigne `key` à `a`. Si une autre action l'utilisait, elle reçoit l'ancienne touche de `a`.
    pub fn set_key(&mut self, a: Action, key: Key) {
        let old = self.keys.get(&a).copied();
        let other = Action::ALL.into_iter().find(|o| {
            *o != a && o.is_mahjong() == a.is_mahjong() && self.keys.get(o) == Some(&key)
        });
        if let Some(o) = other {
            match old {
                Some(k) => self.keys.insert(o, k),
                None => self.keys.remove(&o),
            };
        }
        self.keys.insert(a, key);
    }

    /// Comme `set_key`, l'échange ne concernant que les actions de la même manette.
    pub fn set_button(&mut self, a: Action, btn: Button) {
        let old = self.pad.get(&a).copied();
        let other = Action::ALL
            .into_iter()
            .find(|o| *o != a && o.gamepad() == a.gamepad() && self.pad.get(o) == Some(&btn));
        if let Some(o) = other {
            match old {
                Some(b) => self.pad.insert(o, b),
                None => self.pad.remove(&o),
            };
        }
        self.pad.insert(a, btn);
    }

    /// Remet les valeurs par défaut des actions retenues par `which`.
    pub fn reset(&mut self, which: impl Fn(Action) -> bool) {
        for a in Action::ALL.into_iter().filter(|a| which(*a)) {
            match a.default_key() {
                Some(k) => self.keys.insert(a, k),
                None => self.keys.remove(&a),
            };
            match a.default_button() {
                Some(b) => self.pad.insert(a, b),
                None => self.pad.remove(&a),
            };
        }
    }

    /// Assignations lues dans gui.toml ; une action ou une touche inconnue garde sa valeur par défaut.
    pub fn from_prefs(p: &InputPrefs) -> Bindings {
        let mut b = Bindings::default();
        for (action, value) in &p.keys {
            let Some(a) = Action::from_name(action) else {
                continue;
            };
            if value == NONE {
                b.keys.remove(&a);
            } else if let Some(k) = parse_key(value) {
                b.keys.insert(a, k);
            }
        }
        for (action, value) in &p.pad {
            let Some(a) = Action::from_name(action) else {
                continue;
            };
            if value == NONE {
                b.pad.remove(&a);
            } else if let Some(btn) = parse_button(value) {
                b.pad.insert(a, btn);
            }
        }
        b
    }

    pub fn to_prefs(&self) -> InputPrefs {
        let list = |f: &dyn Fn(Action) -> Option<String>| {
            Action::ALL
                .into_iter()
                .map(|a| (a.name().to_string(), f(a).unwrap_or_else(|| NONE.into())))
                .collect()
        };
        InputPrefs {
            keys: list(&|a| self.key(a).map(key_name)),
            pad: list(&|a| self.button(a).map(|b| button_name(b).to_string())),
            mouse_sens: None,
        }
    }
}

/// Conversion d'un déplacement de souris (pixels) en impulsions de compteur, selon la
/// sensibilité (1.0 = 1 impulsion par pixel).
pub fn pointer_counts(delta: f32, sens: f32) -> i32 {
    (delta * sens).round() as i32
}

/// Mouvement constant simulé par les flèches de J1 (impulsions par trame) : (dx, dy).
pub fn arrows_motion(pads: PadState) -> (i32, i32) {
    const STEP: i32 = 2;
    let b = pads.bits;
    let dx = if b & RIGHT == 0 { STEP } else { 0 } - if b & LEFT == 0 { STEP } else { 0 };
    let dy = if b & DOWN == 0 { STEP } else { 0 } - if b & UP == 0 { STEP } else { 0 };
    (dx, dy)
}

/// Bits (pressés = 1) d'un stick analogique.
pub fn stick_bits(x: f32, y: f32) -> u8 {
    let mut b = 0;
    if x > STICK_THRESHOLD {
        b |= RIGHT;
    }
    if x < -STICK_THRESHOLD {
        b |= LEFT;
    }
    if y > STICK_THRESHOLD {
        b |= UP;
    }
    if y < -STICK_THRESHOLD {
        b |= DOWN;
    }
    b
}

#[derive(Default)]
pub struct InputState {
    pub bindings: Bindings,
    keys: HashSet<Key>,
    shift: bool,
    /// Bits pressés venant des manettes (index 0 = P1).
    pad_pressed: [u8; 2],
    sys_pressed: u8,
    /// Touches du panneau mahjong pressées par les manettes (3 rangées).
    mj_pressed: [u8; 3],
    /// Dernier bouton de manette pressé (capture de la fenêtre Contrôles).
    last_button: Option<(usize, Button)>,
    gilrs: Option<gilrs::Gilrs>,
}

impl InputState {
    pub fn with_bindings(bindings: Bindings) -> Self {
        Self {
            bindings,
            gilrs: gilrs::Gilrs::new().ok(),
            ..Self::default()
        }
    }

    /// Simule l'état d'une touche (tests).
    #[cfg(test)]
    pub fn set_key(&mut self, key: Key, down: bool) {
        if down {
            self.keys.insert(key);
        } else {
            self.keys.remove(&key);
        }
    }

    /// Relève les touches maintenues dans egui.
    pub fn update_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            self.keys = i.keys_down.clone();
            self.shift = i.modifiers.shift;
        });
    }

    /// Relève l'état des deux premières manettes selon les assignations ; le stick gauche et la
    /// croix donnent toujours les directions.
    pub fn poll_gamepads(&mut self) {
        let Some(g) = self.gilrs.as_mut() else { return };
        while let Some(ev) = g.next_event() {
            if let gilrs::EventType::ButtonPressed(b, _) = ev.event {
                let n = g.gamepads().position(|(id, _)| id == ev.id).unwrap_or(0);
                self.last_button = Some((n, b));
            }
        }
        self.pad_pressed = [0; 2];
        self.sys_pressed = 0;
        self.mj_pressed = [0; 3];
        for (n, (_, pad)) in g.gamepads().take(2).enumerate() {
            self.pad_pressed[n] |= stick_bits(
                pad.value(gilrs::Axis::LeftStickX),
                pad.value(gilrs::Axis::LeftStickY),
            );
            for a in Action::ALL {
                let Some(btn) = self.bindings.button(a) else {
                    continue;
                };
                if a.gamepad() == n && pad.is_pressed(btn) {
                    match a.row().3 {
                        Target::Pad(p, bit) => self.pad_pressed[p] |= bit,
                        Target::Sys(bit) => self.sys_pressed |= bit,
                        Target::Mj(r, bit) => self.mj_pressed[r] |= bit,
                    }
                }
            }
        }
    }

    /// Bouton de manette pressé depuis le dernier appel : (numéro de manette, bouton).
    pub fn take_pressed_button(&mut self) -> Option<(usize, Button)> {
        self.last_button.take()
    }

    fn key_down(&self, a: Action) -> bool {
        self.bindings.key(a).is_some_and(|k| self.keys.contains(&k))
    }

    pub fn pads(&self) -> (PadState, PadState) {
        let mut pressed = self.pad_pressed;
        for a in Action::ALL {
            if let (Target::Pad(n, bit), true) = (a.row().3, self.key_down(a)) {
                pressed[n] |= bit;
            }
        }
        (
            PadState { bits: !pressed[0] },
            PadState { bits: !pressed[1] },
        )
    }

    /// Panneau mahjong (3 rangées, bit à 0 = pressée). Les touches du panneau ne comptent que si
    /// `active` (jeu mahjong) : sinon elles se confondent avec celles du joystick.
    pub fn mahjong(&self, active: bool) -> [u8; 3] {
        let mut pressed = if active { self.mj_pressed } else { [0; 3] };
        if active {
            for a in Action::ALL {
                if let (Target::Mj(r, bit), true) = (a.row().3, self.key_down(a)) {
                    pressed[r] |= bit;
                }
            }
        }
        [!pressed[0], !pressed[1], !pressed[2]]
    }

    pub fn system(&self) -> SystemButtons {
        let mut pressed = self.sys_pressed;
        for a in Action::ALL {
            if let (Target::Sys(bit), true) = (a.row().3, self.key_down(a)) {
                pressed |= bit;
            }
        }
        // Maj = Select J1 tant qu'aucune touche ne lui est assignée.
        if self.shift && self.bindings.key(Action::Select1).is_none() {
            pressed |= SELECT1;
        }
        SystemButtons { bits: !pressed }
    }

    /// Vrai si la touche Test est enfoncée.
    #[cfg(test)]
    pub fn test(&self) -> bool {
        self.system().bits & TEST == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_means_default_pads() {
        let i = InputState::default();
        assert_eq!(i.pads(), (PadState::default(), PadState::default()));
        assert_eq!(i.system(), SystemButtons::default());
    }

    #[test]
    fn z_presses_a_only() {
        let mut i = InputState::default();
        i.set_key(Key::Z, true);
        assert_eq!(i.pads().0.bits, !BTN_A);
        assert_eq!(i.pads().1, PadState::default());
    }

    #[test]
    fn up_and_down_both_pressed() {
        let mut i = InputState::default();
        i.set_key(Key::ArrowUp, true);
        i.set_key(Key::ArrowDown, true);
        assert_eq!(i.pads().0.bits & (UP | DOWN), 0);
    }

    #[test]
    fn stick_threshold() {
        assert_eq!(stick_bits(0.6, 0.0), RIGHT);
        assert_eq!(stick_bits(0.4, 0.0), 0);
        assert_eq!(stick_bits(-0.6, 0.6), LEFT | UP);
    }

    #[test]
    fn defaults_match_the_original_table() {
        let mut i = InputState::default();
        i.set_key(Key::Y, true);
        assert_eq!(i.pads().1.bits, !BTN_D);
        assert_eq!(i.pads().0, PadState::default());
        assert_eq!(Action::ALL.len(), N_ACTIONS);
        for (n, a) in Action::ALL.into_iter().enumerate() {
            assert_eq!(a as usize, n, "ordre de TABLE");
            assert_eq!(Action::from_name(a.name()), Some(a));
        }
    }

    #[test]
    fn rebinding_moves_the_action() {
        let mut i = InputState::default();
        i.bindings.set_key(Action::P1A, Key::Q);
        i.set_key(Key::Z, true);
        assert_eq!(i.pads().0, PadState::default());
        i.set_key(Key::Q, true);
        assert_eq!(i.pads().0.bits, !BTN_A);
    }

    #[test]
    fn taking_a_used_key_swaps() {
        let mut b = Bindings::default();
        b.set_key(Action::P1B, Key::Z); // Z était à P1A, X à P1B
        assert_eq!(b.key(Action::P1B), Some(Key::Z));
        assert_eq!(b.key(Action::P1A), Some(Key::X));
        // une action sans touche prend celle d'une autre : l'autre n'en a plus
        b.set_key(Action::Select1, Key::Enter);
        assert_eq!(b.key(Action::Start1), None);
        // manettes : échange seulement sur la même manette
        b.set_button(Action::P1A, Button::East);
        assert_eq!(b.button(Action::P1B), Some(Button::South));
        assert_eq!(b.button(Action::P2B), Some(Button::East));
    }

    #[test]
    fn prefs_round_trip_and_invalid_entries() {
        let mut b = Bindings::default();
        b.set_key(Action::P1A, Key::Q);
        b.set_key(Action::Select1, Key::Enter);
        b.set_button(Action::Coin1, Button::Mode);
        assert_eq!(Bindings::from_prefs(&b.to_prefs()), b);
        assert_eq!(
            Bindings::from_prefs(&InputPrefs::default()),
            Bindings::default()
        );
        let p = InputPrefs {
            keys: vec![
                ("P1A".into(), "PasUneTouche".into()),
                ("Inconnue".into(), "Q".into()),
                ("P1B".into(), "W".into()),
            ],
            pad: vec![("P1A".into(), "Bidule".into())],
            mouse_sens: None,
        };
        let b = Bindings::from_prefs(&p);
        assert_eq!(b.key(Action::P1A), Some(Key::Z));
        assert_eq!(b.key(Action::P1B), Some(Key::W));
        assert_eq!(b.button(Action::P1A), Some(Button::South));
    }

    #[test]
    fn key_names_round_trip() {
        for a in Action::ALL {
            if let Some(k) = a.default_key() {
                assert_eq!(parse_key(&key_name(k)), Some(k), "{k:?}");
            }
            if let Some(b) = a.default_button() {
                assert_eq!(parse_button(button_name(b)), Some(b));
            }
        }
    }

    #[test]
    fn shift_is_select1_until_rebound() {
        let mut i = InputState {
            shift: true,
            ..InputState::default()
        };
        assert_eq!(i.system().bits, !SELECT1);
        i.bindings.set_key(Action::Select1, Key::Q);
        assert_eq!(i.system(), SystemButtons::default());
    }

    #[test]
    fn system_keys() {
        let mut i = InputState::default();
        i.set_key(Key::Num5, true);
        i.set_key(Key::F2, true);
        assert_eq!(i.system().bits, !(COIN1 | TEST));
        assert!(i.test());
    }

    #[test]
    fn pointer_conversion() {
        assert_eq!(pointer_counts(10.0, 1.0), 10);
        assert_eq!(pointer_counts(10.0, 0.5), 5);
        assert_eq!(pointer_counts(-10.0, 0.5), -5);
        assert_eq!(arrows_motion(PadState::default()), (0, 0));
        assert_eq!(arrows_motion(PadState { bits: !RIGHT }), (2, 0));
        assert_eq!(arrows_motion(PadState { bits: !(LEFT | UP) }), (-2, -2));
    }

    #[test]
    fn mahjong_defaults_and_rows() {
        let mut i = InputState::default();
        // hors jeu mahjong : aucune touche du panneau
        i.set_key(Key::A, true);
        assert_eq!(i.mahjong(false), [0xFF; 3]);
        // jeu mahjong : A = rangée 0 bit 0
        assert_eq!(i.mahjong(true), [0xFE, 0xFF, 0xFF]);
        i.set_key(Key::J, true); // rangée 1 bit 2
        i.set_key(Key::R, true); // Ron : rangée 2 bit 3
        assert_eq!(i.mahjong(true), [0xFE, 0xFB, 0xF7]);
    }

    #[test]
    fn mahjong_prefs_roundtrip_and_swap_stays_in_mode() {
        let mut b = Bindings::default();
        // « A » est aussi la touche par défaut de J1 C : pas d'échange entre les deux modes.
        assert_eq!(b.key(Action::P1C), Some(Key::A));
        assert_eq!(b.key(Action::MjA), Some(Key::A));
        b.set_key(Action::MjA, Key::B); // B appartient à MjB : échange dans le mode mahjong
        assert_eq!(b.key(Action::MjA), Some(Key::B));
        assert_eq!(b.key(Action::MjB), Some(Key::A));
        assert_eq!(b.key(Action::P1C), Some(Key::A));
        assert_eq!(Bindings::from_prefs(&b.to_prefs()), b);
        assert_eq!(Action::MjPon.group(), 3);
        assert_eq!(Action::MjPon.label(), "Pon");
    }
}
