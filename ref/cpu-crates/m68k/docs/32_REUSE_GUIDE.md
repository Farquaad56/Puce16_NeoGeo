# Reuse guide: the 68000 core in a console or computer emulator

`puce-m68k` is a `no_std` crate without `unsafe` and without dependency. A host (Mega Drive, Atari ST,
Amiga, arcade board, ...) provides memory and peripherals through the `Bus` trait and calls the CPU
either one instruction at a time (`Cpu::step`) or one clock at a time (`Cpu::tick`, `Cpu::step_clock`).

```toml
[dependencies]
puce-m68k = { path = "crates/puce-m68k" }   # or the published crate
```

## 1. The `Bus` trait

```rust
use puce_m68k::bus::Bus;

struct Machine {
    ram: Vec<u8>,          // 64 KiB of RAM, mirrored, for the example
    irq_level: u8,
}

impl Bus for Machine {
    fn read(&mut self, addr: u32, _fc: u8, _uds: bool, _lds: bool) -> u16 {
        let a = (addr as usize) & 0xFFFE & (self.ram.len() - 1);
        u16::from_be_bytes([self.ram[a], self.ram[a + 1]])
    }
    fn write(&mut self, addr: u32, _fc: u8, uds: bool, lds: bool, data: u16) {
        let a = (addr as usize) & 0xFFFE & (self.ram.len() - 1);
        if uds {
            self.ram[a] = (data >> 8) as u8;
        }
        if lds {
            self.ram[a + 1] = data as u8;
        }
    }
    fn ipl(&mut self) -> u8 {
        self.irq_level
    }
}
```

- Every access is a 16-bit word at an even address. `uds` and `lds` select the byte lanes: a word access
  sets both, a byte access at an even address sets only `uds` (data on D8-D15), at an odd address only
  `lds` (data on D0-D7). A write must store only the selected lanes.
- `fc` is the function code (1 user data, 2 user program, 5 supervisor data, 6 supervisor program, 7 interrupt
  acknowledge). Use it to protect supervisor memory or to detect instruction fetches.
- `wait_clocks(addr, write)` returns extra wait clocks for the bus cycle that starts now (default 0: DTACK
  is immediate). A bus cycle always takes 4 clocks plus the wait states. This is where the DRAM refresh,
  the VDP/blitter contention or the 8-bit peripheral delays of a machine are modelled.
- `ipl()` is sampled at every instruction boundary and returns the interrupt priority level (0 none, 1..7).
  The CPU takes the interrupt when the level is above the interrupt mask in SR, or is 7 (non-maskable).
  Keep returning the level until the interrupt is acknowledged (see `int_ack`).
- `int_ack(level)` is the interrupt acknowledge cycle (FC = 7). Return `Some(vector)` for a device that
  supplies a vector, or `None` for autovectors (vector 24 + level, the usual case on consoles). The host
  should lower the level when the cycle happens.
- `on_tick(pins)` is called once per clock with the pins driven by the CPU, to clock peripherals in step
  with the processor (video counters, sound chips, timers).

## 2. Driving the CPU

```rust
use puce_m68k::cpu::Cpu;

let mut m = Machine { ram: vec![0; 0x10000], irq_level: 0 };
// ... load a ROM image into the memory map, with the initial SSP at 0 and PC at 4 ...

let mut cpu = Cpu::new();
cpu.reset();              // 40 clocks: SR = 0x2700, SSP and PC are read from vectors 0 and 1
cpu.step(&mut m);         // finishes the reset sequence

let budget: u64 = 7_670_454 / 60;   // clocks per frame (Mega Drive NTSC)
let mut spent = 0u64;
while spent < budget {
    spent += u64::from(cpu.step(&mut m));   // clocks of the whole instruction, wait states included
}
```

| Call | Granularity | Use |
|---|---|---|
| `Cpu::step(&mut bus)` | one instruction (or one exception sequence) | simple machines, debuggers |
| `Cpu::step_clock(&mut bus)` | one clock, starts the next instruction at a boundary | lock-step with other chips |
| `Cpu::tick_bus(&mut bus)` | one clock of the running instruction | same, without starting one |
| `Cpu::tick(pins)` | one clock on raw pins (DTACK in, address/data/strobes out) | hardware-level hosts, FPGA-style models |

`step` returns the number of clocks used (4 clocks = one bus cycle with no wait state). It returns 0 only
when the CPU is halted (double fault: reset needed). `Cpu::is_stopped()` is true after `STOP`: `step` then
returns 4 per call until an interrupt is taken, so a host loop keeps its timing while the CPU sleeps.

### Pins and `tick`

`Cpu::tick(pins)` takes and returns a `Pins` value, a 64-bit word: address on bits 0-23, data on bits 24-39,
then AS, UDS, LDS, R/W, DTACK, FC0-2, IPL0-2, VPA, RESET and HALT (constants in `puce_m68k::pins`). One call
is one clock: the host reads the outputs, decodes the cycle (address strobe, R/W, lanes), drives DATA and
DTACK for the next clock and calls `tick` again. Specification: `docs/specs/11_PINS_AND_TICK.md`.

## 3. Exceptions and interrupts

- Illegal opcodes raise vector 4, line A (`0xAxxx`) vector 10, line F (`0xFxxx`) vector 11. `TRAP #n` uses
  vectors 32-47, `CHK` vector 6, `TRAPV` vector 7, a privilege violation (user mode) vector 8.
- Every exception pushes PC and SR on the supervisor stack, reads the vector at `4 * n` and refills the
  prefetch queue: 34 clocks (44 for an interrupt, 40 for reset).
- Call `Cpu::exception(vector)` to raise an exception from the host, `Cpu::reset()` for a reset, `Cpu::halt()`
  to stop the CPU. `Cpu::load_state(&CpuState)` and `Cpu::state()` save and restore the registers.
- A word or long access at an odd address (or an odd PC) raises an address error: the aborted cycle takes 4 clocks, then the 14-byte group 0 frame (vector 3) is pushed. Bus errors have no input.

## 4. Mapping a machine

The 24-bit address bus is yours to decode; mask addresses with `0x00FF_FFFF` before decoding.

| Machine | CPU clock | Notes for the `Bus` |
|---|---|---|
| Sega Mega Drive / Genesis | 7.67 MHz NTSC (master 53.693175 MHz / 7) | ROM at 0, work RAM at `0xFF0000`, VDP ports at `0xC00000`, Z80 bus and I/O at `0xA00000`; VDP DMA and the Z80 holding the bus are waits (`wait_clocks`); VDP interrupts are autovector levels 4 (HBlank) and 6 (VBlank) |
| Atari ST | 8 MHz | ROM at `0xE00000`, RAM from 0 (shared with the video shifter: the 4-clock-aligned access rule adds wait states), MFP 68901 at `0xFFFA00` supplying a vector through `int_ack`, ACIA keyboard at `0xFFFC00` |
| Amiga | 7.09 MHz PAL / 7.16 MHz NTSC | Chip RAM is shared with the custom chips: the CPU waits when DMA slots are taken, which is `wait_clocks` on chip-RAM addresses; the custom chips raise levels 1-6 (autovector), CIA at `0xBFE001` and `0xBFD000` |

For a per-frame loop, convert the CPU clock into your master clock, run `step` until the budget of the
scanline or frame is spent, and let `on_tick` or the loop body advance the video and sound chips by the
clocks returned.

## 5. Status and known limits

- All 68000 instructions run and are verified against the SingleStepTests/m68000 oracle (317500 tests):
  `docs/SUITES_REPORT.md` has the per-file results. No file fails (`TAS` included: the core tags its read
  as an indivisible cycle, the oracle as a plain read, and the runner treats both alike).
- The oracle cases that raise an address error pass too (frame, pushed PC and IR, flags, registers); the pushed
  values follow tables derived from the oracle (decision D-052). A second fault during the frame halts the
  CPU. There is no bus error input.
- The core is `no_std`, single-threaded and allocation free. The `inspect` feature records a bus journal
  for debuggers and tests.
- Speed: `m68k bench` measures about 90 MHz of emulated 68000 clock per second in release mode on the
  development machine, far above the 7-8 MHz of the real machines.
