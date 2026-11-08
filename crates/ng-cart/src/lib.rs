#![forbid(unsafe_code)]
//! ng-cart : cartouches (chargeur, déchiffrement). Voir tasks/T300.
#[rustfmt::skip]
pub mod cmc_tables;
#[rustfmt::skip]
pub mod defs;
#[rustfmt::skip]
pub mod types;

mod bios;
mod config;
mod romset;
pub use bios::{load_bios, Bios, BiosError};
pub use config::{load_config, parse_config, Config, Model};
pub use romset::{find_def, load_cart, Cart, CartError, Source};

#[cfg(test)]
mod smoke {
    #[test]
    fn it_builds() {}
}
