#![forbid(unsafe_code)]
//! ng-cart : cartouches (chargeur, déchiffrement). Voir tasks/T300.
pub mod cmc;
pub mod install;
pub mod kof2k2;
pub mod kof98;
#[rustfmt::skip]
pub mod cmc_tables;
pub mod pcm2;
pub mod pvc;
pub mod sma;
#[rustfmt::skip]
pub mod defs;
#[rustfmt::skip]
pub mod types;

mod bios;
#[cfg(test)]
mod catalogue_tests;
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
