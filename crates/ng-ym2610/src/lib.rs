#![forbid(unsafe_code)]
//! ng-ym2610: YM2610 FM/ADPCM sound chip emulation (Neo Geo).
//! Ported from ymfm by Aaron Giles (BSD 3-Clause License).

pub mod adpcm_a;
pub mod adpcm_b;
pub mod common;
pub mod fm_channel;
pub mod fm_engine;
pub mod fm_operator;
#[rustfmt::skip]
pub mod fm_tables;
pub mod opn_regs;
pub mod ssg;
pub mod ym2610;
