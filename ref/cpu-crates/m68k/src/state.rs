//! Snapshot of the architectural state, used to load and compare test states.

/// CPU registers plus the prefetch queue (IR, IRC). A7 is `usp` or `ssp` depending on S.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuState {
    /// D0-D7.
    pub d: [u32; 8],
    /// A0-A6.
    pub a: [u32; 7],
    /// User stack pointer.
    pub usp: u32,
    /// Supervisor stack pointer.
    pub ssp: u32,
    /// Status register.
    pub sr: u16,
    /// Address of the next word to prefetch (instruction address + 4 at a boundary).
    pub pc: u32,
    /// Prefetch queue: IR then IRC.
    pub prefetch: [u16; 2],
}
