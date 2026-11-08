# CPU-Motorola_68K_Crates_Rust

A reusable, **cycle-accurate Motorola 68000 emulator core** written in Rust, with the tools used to prove it correct:
a command-line tool, a local web debugger and a WebAssembly build.

## Workspace

| Crate | Role | Binary |
|---|---|---|
| [`puce-m68k`](crates/puce-m68k) | The CPU core (`no_std`, no `unsafe`, no dependency) | - |
| [`puce-m68k-machines`](crates/puce-m68k-machines) | Flat RAM machine, SingleStepTests runner, simulation session | - |
| [`puce-m68k-cli`](crates/puce-m68k-cli) | Command-line tool | `m68k` |
| [`puce-m68k-web`](crates/puce-m68k-web) | Local web server and Tailwind browser front end (HTTP + WebSocket) | `m68k-web` |
| [`puce-m68k-wasm`](crates/puce-m68k-wasm) | The core compiled to WebAssembly (serverless demo) | - |

## Technical specifications

- Motorola 68000 only, 24-bit address bus, 16-bit data bus, all instructions and addressing modes.
- Accuracy at the micro-cycle level: `Cpu::tick` is one clock, a bus cycle is 4 clocks plus DTACK wait states,
  with the prefetch queue (IR, IRC) modelled.
- Pins: DTACK, UDS/LDS, FC0-2, AS, R/W, IPL0-2, VPA; or the convenience `Bus` trait with `wait_clocks`,
  `ipl`, `int_ack` and `on_tick`.
- Exceptions: reset, illegal instruction, line A/F, TRAP, TRAPV, CHK, privilege violation, interrupts
  (autovector or supplied vector), STOP/RESET/RTE. Address errors (odd word or long access, odd PC) give the 14-byte frame of vector 3; there is no bus error input.
- `no_std`, `#![forbid(unsafe_code)]`, no dependency, no allocation in the core.
- Speed: about 90 MHz of emulated 68000 clock per second in release mode (`m68k bench`), against 7-8 MHz on
  real machines.

## Test results

The oracle is [SingleStepTests/m68000](https://github.com/SingleStepTests/m68000) (127 files, 317500 tests,
generated from MAME's microcoded core) plus `Yacht.txt` for the bus timing of each instruction.

| | Cases |
|---|---:|
| Passed | 317500 |
| Failed | 0 |

Per-file results: [`docs/SUITES_REPORT.md`](docs/SUITES_REPORT.md). `tests/baseline.json` is the reference
the regression check compares with. No known exception remains: the oracle labels the read of `TAS` as a plain
read, and the runner compares it that way (decision D-051).

## Installation

```
git clone https://github.com/Farquaad56/CPU-Motorola_68K_Crates_Rust
cd CPU-Motorola_68K_Crates_Rust
python docs/tools/fetch_refs.py            # manuals, Yacht.txt, SingleStepTests (network needed once)
cargo build --release --workspace
cargo test --workspace
```

## Usage

```
m68k run program.bin --org 1000            # run a program on the flat RAM machine
m68k disasm program.bin --org 1000         # disassemble
m68k ssc tests/singlestep --strict --compare tests/baseline.json   # oracle + regression check
m68k bench                                 # speed in MHz equivalent
m68k-web                                   # web debugger on http://127.0.0.1 (see its banner); q + Enter quits
```

As a library, implement `puce_m68k::bus::Bus` for your machine and call `Cpu::step`; the guide
[`docs/specs/32_REUSE_GUIDE.md`](docs/specs/32_REUSE_GUIDE.md) covers a Mega Drive, Atari ST or Amiga host.
The serverless browser demo is in [`crates/puce-m68k-wasm`](crates/puce-m68k-wasm).

## Documentation

Method and decisions: `docs/00_CADRAGE.md`, `docs/00_METHODE.md`, `docs/01_ARCHITECTURE.md`,
`docs/qa/DECISIONS.md`. Pins and tick: `docs/specs/11_PINS_AND_TICK.md`. Front-end protocol:
`docs/specs/31_FRONTEND_PROTOCOL.md`. The work was driven by 148 task cards (`docs/tasks/`) on a Hermes Kanban
board (`docs/03_KANBAN_HERMES.md`); `python docs/tools/task.py status` shows the progress.

## License

MIT OR Apache-2.0.
