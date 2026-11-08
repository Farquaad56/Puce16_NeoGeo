//! Lowest-level use of the crate: the host owns the memory and serves it pin by pin.
//!
//! One call to `Cpu::tick` is one T-state. The CPU only shows pins; the host reads the outputs
//! of a tick and reacts in the NEXT tick:
//!
//! - `MREQ|RD`: put the byte of that address on D0..D7 of the next `pins_in`;
//! - `MREQ|WR`: store D0..D7 of the outputs at that address.
//!
//! Run it with `cargo run -q -p puce-z80 --example pins_host`.

use puce_z80::{Cpu, Pins};

/// LD A,1 ; ADD A,A ; HALT
const PROGRAM: [u8; 4] = [0x3E, 0x01, 0x87, 0x76];

fn main() {
    let mut ram = [0u8; 65536];
    ram[..PROGRAM.len()].copy_from_slice(&PROGRAM);

    let mut cpu = Cpu::new();
    let mut out = Pins(0); // outputs of the previous tick
    let mut ticks = 0u32;
    let mut instructions = 0;

    while instructions < 3 {
        // Inputs of this tick: the data requested by the previous outputs.
        let mut inp = Pins(0);
        if out.has(Pins::MREQ) && out.has(Pins::RD) {
            inp = inp.set_data(ram[usize::from(out.addr())]);
        }
        // A write requested by the previous outputs is stored now.
        if out.has(Pins::MREQ) && out.has(Pins::WR) {
            ram[usize::from(out.addr())] = out.data();
        }

        out = cpu.tick(inp);
        ticks += 1;
        println!(
            "T{:<3} A={:04X} D={:02X} {}{}{}{}{}",
            ticks,
            out.addr(),
            out.data(),
            if out.has(Pins::M1) { "M1 " } else { "" },
            if out.has(Pins::MREQ) { "MREQ " } else { "" },
            if out.has(Pins::RD) { "RD " } else { "" },
            if out.has(Pins::WR) { "WR " } else { "" },
            if out.has(Pins::RFSH) { "RFSH " } else { "" },
        );
        if cpu.at_instruction_boundary() {
            instructions += 1;
        }
    }

    println!(
        "A = {} after {} instructions and {} T-states",
        cpu.regs.af() >> 8,
        instructions,
        ticks
    );
}
