//! Entrées Neo Geo : pads joueurs et boutons système.
//!
//! Convention matérielle : bits actifs-BAS (1 = relâché, 0 = enfoncé).

/// État d'un pad joueur (8 boutons + D-pad).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PadState {
    /// Bits du pad : bit7=D, bit6=C, bit5=B, bit4=A, bit3=Droite, bit2=Gauche, bit1=Bas, bit0=Haut.
    pub bits: u8,
}

impl Default for PadState {
    fn default() -> Self {
        // Tous les boutons relâchés (bits à 1).
        PadState { bits: 0xFF }
    }
}

/// État des boutons système (start, select, pièces, service, test).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SystemButtons {
    /// Bits système : bit0=Start1, bit1=Select1, bit2=Start2, bit3=Select2,
    /// bit4=Coin1, bit5=Coin2, bit6=Service, bit7=Test. 1 = relâché.
    pub bits: u8,
}

impl Default for SystemButtons {
    fn default() -> Self {
        // Tous les boutons relâchés (bits à 1).
        SystemButtons { bits: 0xFF }
    }
}

/// Trait pour fournir l'état des entrées à l'émulateur.
pub trait InputProvider {
    /// Retourne l'état du pad du joueur donné (0 ou 1).
    fn pad(&self, player: usize) -> PadState;
    /// Retourne l'état des boutons système.
    fn system(&self) -> SystemButtons;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_default_all_released() {
        let pad = PadState::default();
        assert_eq!(pad.bits, 0xFF);
    }

    #[test]
    fn system_buttons_default_all_released() {
        let sys = SystemButtons::default();
        assert_eq!(sys.bits, 0xFF);
    }

    #[test]
    fn pad_button_pressed_clears_bit() {
        // Button A is bit 4. Pressing it clears the bit (active-low).
        let mut pad = PadState::default();
        pad.bits &= !(1 << 4);
        assert_eq!(pad.bits & (1 << 4), 0);
    }

    #[test]
    fn system_button_pressed_clears_bit() {
        // Start1 is bit 0. Pressing it clears the bit (active-low).
        let mut sys = SystemButtons::default();
        sys.bits &= !(1 << 0);
        assert_eq!(sys.bits & 1, 0);
    }

    #[test]
    fn input_provider_trait() {
        struct TestProvider;
        impl InputProvider for TestProvider {
            fn pad(&self, _player: usize) -> PadState {
                PadState::default()
            }
            fn system(&self) -> SystemButtons {
                SystemButtons::default()
            }
        }
        let provider = TestProvider;
        assert_eq!(provider.pad(0).bits, 0xFF);
        assert_eq!(provider.system().bits, 0xFF);
    }
}
