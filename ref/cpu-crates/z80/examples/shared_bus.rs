//! A shared bus, Mega Drive style: another processor takes the Z80 bus (BUSRQ/BUSAK), writes the
//! Z80 RAM while the Z80 is frozen, gives the bus back, and finally resets the Z80.
//!
//! - the host raises `Pins::BUSRQ` in `pins_in`; the Z80 finishes the current bus cycle and then
//!   answers with `Pins::BUSAK` in `pins_out`, floating its other outputs;
//! - while `BUSAK` is active nothing runs: PC and the registers stay frozen;
//! - dropping `BUSRQ` lets the Z80 carry on where it stopped;
//! - `Pins::RESET` restarts it at address 0.
//!
//! Run it with `cargo run -q -p puce-z80 --example shared_bus`.

use puce_z80::{Cpu, Pins};

/// LD A,0 followed by INC A forever (the RAM is filled with 0x3C after the first two bytes).
fn new_ram() -> Box<[u8; 65536]> {
    let mut ram = Box::new([0x3Cu8; 65536]);
    ram[0] = 0x3E;
    ram[1] = 0x00;
    ram
}

struct Host {
    cpu: Cpu,
    ram: Box<[u8; 65536]>,
    out: Pins,
}

impl Host {
    fn new() -> Host {
        Host {
            cpu: Cpu::new(),
            ram: new_ram(),
            out: Pins(0),
        }
    }

    /// One T-state; `extra` are the other input pins (BUSRQ, RESET).
    fn tick(&mut self, extra: u64) -> Pins {
        let mut inp = Pins(extra);
        if self.out.has(Pins::MREQ) && self.out.has(Pins::RD) {
            inp = inp.set_data(self.ram[usize::from(self.out.addr())]);
        }
        if self.out.has(Pins::MREQ) && self.out.has(Pins::WR) {
            self.ram[usize::from(self.out.addr())] = self.out.data();
        }
        self.out = self.cpu.tick(inp);
        self.out
    }
}

fn main() {
    let mut z80 = Host::new();

    // The Z80 runs for a while.
    for _ in 0..30 {
        z80.tick(0);
    }
    println!(
        "running: PC={:04X} A={}",
        z80.cpu.regs.pc,
        z80.cpu.regs.af() >> 8
    );

    // The other processor asks for the bus and waits for BUSAK.
    let mut waited = 0;
    while !z80.tick(Pins::BUSRQ).has(Pins::BUSAK) {
        waited += 1;
    }
    println!(
        "BUSAK after {} more T-states, PC={:04X}",
        waited, z80.cpu.regs.pc
    );

    // It owns the bus: it writes the Z80 RAM; the Z80 does not move.
    let frozen_pc = z80.cpu.regs.pc;
    for i in 0..20u16 {
        assert!(z80.tick(Pins::BUSRQ).has(Pins::BUSAK));
        z80.ram[0x1000 + usize::from(i)] = i as u8; // the "master" writes here
    }
    assert_eq!(z80.cpu.regs.pc, frozen_pc);
    println!(
        "bus held 20 T-states: PC still {:04X}, RAM[0x1005]={}",
        frozen_pc, z80.ram[0x1005]
    );

    // The bus is given back: the Z80 resumes.
    for _ in 0..30 {
        z80.tick(0);
    }
    println!(
        "resumed: PC={:04X} A={}",
        z80.cpu.regs.pc,
        z80.cpu.regs.af() >> 8
    );

    // RESET: back to address 0.
    z80.tick(Pins::RESET);
    println!("after RESET: PC={:04X}", z80.cpu.regs.pc);
}
