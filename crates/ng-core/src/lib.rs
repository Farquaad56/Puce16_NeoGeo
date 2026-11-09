#![forbid(unsafe_code)]
//! ng-core : ordonnanceur, wait states, démarrage et intégration de la machine Neo Geo.

pub mod audio_mix;
pub mod host68k;
pub mod machine;
pub mod sched;
pub mod shared;
pub mod ym_adapter;

pub use machine::Machine;
