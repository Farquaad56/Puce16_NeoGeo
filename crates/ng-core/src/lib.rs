#![forbid(unsafe_code)]
//! ng-core : ordonnanceur, wait states, démarrage et intégration de la machine Neo Geo.

pub mod audio_mix;
pub mod boot;
pub mod debug;
pub mod host68k;
pub mod machine;
pub mod sched;
pub mod shared;
pub mod state;
pub mod waits;
pub mod ym_adapter;

pub use boot::{insert_memcard, load_machine, memcard_path, save_machine};
pub use debug::{DebugLevel, DebugSnapshot, LspcDebug};
pub use machine::{Machine, Stats};
