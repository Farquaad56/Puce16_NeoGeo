#![forbid(unsafe_code)]
//! ng-bus: 68000 system bus with Neo Geo memory mapping.

pub mod aes;
pub mod backup;
pub mod device;
pub mod io;
pub mod latch;
pub mod map68k;
pub mod memcard;
pub mod pbank;
pub mod prot_cartram;
pub mod prot_fatfury2;
pub mod prot_kof98;
pub mod prot_mslugx;
pub mod prot_pvc;
pub mod prot_sbp;
pub mod prot_sma;
pub mod rtc;
pub mod sound;
pub mod watchdog;
pub mod z80map;
pub mod z80port;

pub use device::{CartPort, Device, NullDevice};
pub use io::Io;
pub use latch::SystemLatch;
pub use map68k::Bus68k;
pub use sound::{SoundBoard, Ym};
pub use watchdog::Watchdog;
