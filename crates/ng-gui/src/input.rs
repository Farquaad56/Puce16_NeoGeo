//! Entrées : clavier (egui) et manettes (gilrs) vers `PadState` / `SystemButtons` (actifs-bas).

use std::collections::HashSet;

use eframe::egui::{self, Key};
use ng_common::{PadState, SystemButtons};

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

/// Table clavier -> bits pad (P1 puis P2).
const PAD_KEYS: [[(Key, u8); 8]; 2] = [
    [
        (Key::ArrowUp, UP),
        (Key::ArrowDown, DOWN),
        (Key::ArrowLeft, LEFT),
        (Key::ArrowRight, RIGHT),
        (Key::Z, BTN_A),
        (Key::X, BTN_B),
        (Key::A, BTN_C),
        (Key::S, BTN_D),
    ],
    [
        (Key::I, UP),
        (Key::K, DOWN),
        (Key::J, LEFT),
        (Key::L, RIGHT),
        (Key::G, BTN_A),
        (Key::H, BTN_B),
        (Key::T, BTN_C),
        (Key::Y, BTN_D),
    ],
];

/// Table clavier -> bits système.
const SYS_KEYS: [(Key, u8); 7] = [
    (Key::Enter, START1),
    (Key::P, START2),
    (Key::Num5, COIN1),
    (Key::Num6, COIN2),
    (Key::Num9, SERVICE),
    (Key::F2, TEST),
    (Key::Tab, SELECT2),
];

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
    keys: HashSet<Key>,
    shift: bool,
    /// Bits pressés venant des manettes (index 0 = P1).
    pad_pressed: [u8; 2],
    sys_pressed: u8,
    gilrs: Option<gilrs::Gilrs>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            gilrs: gilrs::Gilrs::new().ok(),
            ..Self::default()
        }
    }

    /// Simule l'état d'une touche (tests).
    pub fn set_key(&mut self, key: Key, down: bool) {
        if down {
            self.keys.insert(key);
        } else {
            self.keys.remove(&key);
        }
    }

    /// Relève les touches maintenues dans egui. Maj (droite ou gauche) = Select 1.
    pub fn update_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            self.keys = i.keys_down.clone();
            self.shift = i.modifiers.shift;
        });
    }

    /// Relève l'état des deux premières manettes.
    pub fn poll_gamepads(&mut self) {
        let Some(g) = self.gilrs.as_mut() else { return };
        while g.next_event().is_some() {}
        self.pad_pressed = [0; 2];
        self.sys_pressed = 0;
        for (n, (_, pad)) in g.gamepads().take(2).enumerate() {
            let mut bits = 0;
            for (btn, bit) in [
                (gilrs::Button::South, BTN_A),
                (gilrs::Button::East, BTN_B),
                (gilrs::Button::West, BTN_C),
                (gilrs::Button::North, BTN_D),
                (gilrs::Button::DPadUp, UP),
                (gilrs::Button::DPadDown, DOWN),
                (gilrs::Button::DPadLeft, LEFT),
                (gilrs::Button::DPadRight, RIGHT),
            ] {
                if pad.is_pressed(btn) {
                    bits |= bit;
                }
            }
            bits |= stick_bits(
                pad.value(gilrs::Axis::LeftStickX),
                pad.value(gilrs::Axis::LeftStickY),
            );
            self.pad_pressed[n] = bits;
            if pad.is_pressed(gilrs::Button::Start) {
                self.sys_pressed |= if n == 0 { START1 } else { START2 };
            }
            if pad.is_pressed(gilrs::Button::Select) {
                self.sys_pressed |= if n == 0 { SELECT1 } else { SELECT2 };
            }
        }
    }

    /// Injecte l'état « pressé » d'une manette (tests).
    pub fn set_pad_pressed(&mut self, n: usize, bits: u8) {
        self.pad_pressed[n] = bits;
    }

    pub fn pads(&self) -> (PadState, PadState) {
        let mk = |n: usize| {
            let mut pressed = self.pad_pressed[n];
            for (k, bit) in PAD_KEYS[n] {
                if self.keys.contains(&k) {
                    pressed |= bit;
                }
            }
            PadState { bits: !pressed }
        };
        (mk(0), mk(1))
    }

    pub fn system(&self) -> SystemButtons {
        let mut pressed = self.sys_pressed;
        for (k, bit) in SYS_KEYS {
            if self.keys.contains(&k) {
                pressed |= bit;
            }
        }
        if self.shift {
            pressed |= SELECT1;
        }
        SystemButtons { bits: !pressed }
    }

    /// Vrai si la touche Test est enfoncée.
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
    fn system_keys() {
        let mut i = InputState::default();
        i.set_key(Key::Num5, true);
        i.set_key(Key::F2, true);
        assert_eq!(i.system().bits, !(COIN1 | TEST));
        assert!(i.test());
    }
}
