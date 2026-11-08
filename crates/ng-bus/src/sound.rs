//! Neo Geo Z80 SoundBoard: memory map, banking, ports, latches, pin server.
//!
//! Implements the Z80 side of the Neo Geo audio hardware as specified in
//! docs/spec/03-audio-z80.md and ref/mame-neogeo/neogeo.cpp (lines 1105-1140,
//! 1300-1350, 1790-1810, 1948-1960).

use puce_z80::{Cpu, Pins};

use crate::z80map::banked_offset;
use crate::z80port::Z80Port;

/// YM2610 sound chip interface (to be implemented by ng-sound later).
pub trait Ym {
    /// Advance the YM2610 internal clock by `mclk` master-clock cycles.
    fn advance(&mut self, mclk: u64);
    /// Read from YM2610 port (port 0..3 = addresses A/B Z80 $04..$07).
    fn read(&mut self, port: u8) -> u8;
    /// Write to YM2610 port.
    fn write(&mut self, port: u8, v: u8);
    /// Return true if the YM2610 is requesting an IRQ (timers A/B).
    fn irq(&self) -> bool;
}

/// Z80 SoundBoard with memory map, banking, ports, and pin server.
pub struct SoundBoard<Y: Ym> {
    /// Z80 time in mclk cycles (+= 6 per tick).
    pub time: u64,
    /// BIOS audio ROM (sm1.sm1), up to 32 Kio.
    pub sm1: Vec<u8>,
    /// Cartridge audio ROM (M1), up to 512 Kio.
    pub m1: Vec<u8>,
    /// Z80 RAM, 2 Kio at $F800-$FFFF.
    pub ram: [u8; 0x800],
    /// Latch q5 (CRTFIX): true = use cart M1 for $0000-$7FFF, false = BIOS sm1.
    pub use_cart: bool,
    /// Bank numbers for windows $F000, $E000, $C000, $8000 (indices 0..3).
    pub bank: [u8; 4],
    /// Command byte from 68k.
    pub cmd: u8,
    /// True when a command is pending (not yet read by Z80).
    pub cmd_pending: bool,
    /// Reply byte written by Z80 to port $0C.
    pub reply: u8,
    /// True when NMI is enabled via `out ($08)`.
    pub nmi_enabled: bool,
    /// YM2610 sound chip instance.
    pub ym: Y,
    /// Pin-server state (last pins, read data, write done).
    port: Z80Port,
}

impl<Y: Ym> SoundBoard<Y> {
    /// Create a new SoundBoard with default reset values.
    pub fn new(ym: Y) -> Self {
        Self {
            time: 0,
            sm1: Vec::new(),
            m1: Vec::new(),
            ram: [0; 0x800],
            use_cart: false,
            bank: [0x1E, 0x0E, 0x06, 0x02], // MAME initial banks (hack for non-banked games)
            cmd: 0,
            cmd_pending: false,
            reply: 0,
            nmi_enabled: false,
            ym,
            port: Z80Port::default(),
        }
    }

    /// Reset the SoundBoard (banks to MAME defaults, clear latches).
    pub fn reset(&mut self) {
        self.bank = [0x1E, 0x0E, 0x06, 0x02];
        self.cmd_pending = false;
        self.nmi_enabled = false;
        self.port = Z80Port::default();
    }

    /// Read a byte from the Z80 memory map.
    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => {
                // Fixed region: cart M1 or BIOS sm1 depending on use_cart latch
                let rom = if self.use_cart { &self.m1 } else { &self.sm1 };
                if (addr as usize) < rom.len() {
                    rom[addr as usize]
                } else {
                    0xFF
                }
            }
            0x8000..=0xBFFF => self.banked_read(3, addr), // 16 Kio window
            0xC000..=0xDFFF => self.banked_read(2, addr), // 8 Kio window
            0xE000..=0xEFFF => self.banked_read(1, addr), // 4 Kio window
            0xF000..=0xF7FF => self.banked_read(0, addr), // 2 Kio window
            0xF800..=0xFFFF => self.ram[(addr - 0xF800) as usize],
        }
    }

    /// Write a byte to the Z80 memory map (only RAM is writable).
    pub fn write(&mut self, addr: u16, v: u8) {
        if (0xF800..=0xFFFF).contains(&addr) {
            self.ram[(addr - 0xF800) as usize] = v;
        }
        // All other regions are ROM — writes ignored.
    }

    /// Read from a Z80 I/O port.
    pub fn input(&mut self, addr: u16) -> u8 {
        let port = addr as u8;
        match port & 0x0F {
            0x00 => {
                // Command latch read — clears pending flag
                self.cmd_pending = false;
                self.cmd
            }
            0x04..=0x07 => {
                // YM2610 ports (advance time first)
                self.ym.advance(self.time);
                self.ym.read(port & 3)
            }
            0x08..=0x0B => {
                // Banking: IN A,(C) with C=$08..$0B sets bank from B (high byte of addr)
                let i = (port & 3) as usize;
                self.bank[i] = (addr >> 8) as u8;
                0
            }
            _ => 0xFF,
        }
    }

    /// Write to a Z80 I/O port.
    pub fn output(&mut self, addr: u16, v: u8) {
        let port = addr as u8;
        match port & 0x0F {
            0x00 => {
                // Command latch write — also clears pending flag
                self.cmd_pending = false;
            }
            0x04..=0x07 => {
                // YM2610 ports (advance time first)
                self.ym.advance(self.time);
                self.ym.write(port & 3, v);
            }
            0x08 => {
                // NMI enable/disable: bit 4 of port selects
                self.nmi_enabled = (port & 0x10) == 0;
            }
            0x0C => {
                // Reply latch write (Z80 -> 68k)
                self.reply = v;
            }
            _ => {}
        }
    }

    /// Called by the 68k side to send a command byte.
    pub fn host_write_cmd(&mut self, v: u8) {
        self.cmd = v;
        self.cmd_pending = true;
    }

    /// NMI level: high when command is pending AND NMI is enabled.
    pub fn nmi_level(&self) -> bool {
        self.cmd_pending && self.nmi_enabled
    }

    /// IRQ level: from YM2610 timers.
    pub fn irq_level(&self) -> bool {
        self.ym.irq()
    }

    /// Read from a banked window.
    fn banked_read(&mut self, i: usize, addr: u16) -> u8 {
        let off = banked_offset(self.m1.len(), self.bank[i], i, addr);
        if off < self.m1.len() {
            self.m1[off]
        } else {
            0xFF
        }
    }

    /// Serve one T-state of the Z80 CPU via pin-level interaction.
    ///
    /// This is the "pin server" that replaces `Cpu::step()` — it drives the CPU
    /// one tick at a time, handling memory and I/O requests as they appear on
    /// the pins. Each call advances `self.time` by 6 mclk cycles (one T-state).
    pub fn tick(&mut self, cpu: &mut Cpu) {
        // Build input pins for this tick based on previous outputs
        let mut pins_in = Pins(0);

        // Handle read requests from previous tick's outputs
        let last = self.port.last;
        let mem_rd = last.has(Pins::MREQ) && last.has(Pins::RD);
        let io_rd = last.has(Pins::IORQ) && last.has(Pins::RD) && !last.has(Pins::M1);
        let ack = last.is_int_ack();

        if mem_rd || io_rd || ack {
            // Provide data requested by previous outputs
            let data = match self.port.read_data.take() {
                Some(d) => d,
                None => {
                    let d = if mem_rd {
                        self.read(last.addr())
                    } else if io_rd {
                        self.input(last.addr())
                    } else {
                        0xFF // INT ACK returns opcode (not used in Neo Geo)
                    };
                    self.port.read_data = Some(d);
                    d
                }
            };
            pins_in = pins_in.set_data(data);
        } else {
            self.port.read_data = None;
        }

        // Handle write requests from previous tick's outputs
        let mem_wr = last.has(Pins::MREQ) && last.has(Pins::WR);
        let io_wr = last.has(Pins::IORQ) && last.has(Pins::WR);
        if mem_wr || io_wr {
            if !self.port.write_done {
                if mem_wr {
                    self.write(last.addr(), last.data());
                } else {
                    self.output(last.addr(), last.data());
                }
                self.port.write_done = true;
            }
        } else {
            self.port.write_done = false;
        }

        // Forward interrupt lines (no WAIT, no BUSRQ on Neo Geo)
        if self.irq_level() {
            pins_in = pins_in.with(Pins::INT);
        }
        if self.nmi_level() {
            pins_in = pins_in.with(Pins::NMI);
        }

        // Execute one T-state
        let out = cpu.tick(pins_in);
        self.port.last = out;
        self.time += 6;
    }
}
