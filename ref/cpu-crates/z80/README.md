# CPU-Zilog-Z80_Crates_Rust

A reusable, **cycle-accurate Zilog Z80 emulator** written in Rust, with the tools used to prove it correct.

The heart of the project is `puce-z80`, a CPU core you can drop into a console, computer or arcade
emulator: `no_std`, no allocation, no `unsafe`, no dependency, deterministic, accurate to the T-state.
Around it, the workspace provides a CP/M machine, a ZX Spectrum-like test machine, a command-line tool
and a browser front end, all used to run the reference Z80 test suites.

## Contents

- [Workspace](#workspace)
- [Technical specifications](#technical-specifications)
- [Installation](#installation)
- [Usage](#usage)
- [Test suites and results](#test-suites-and-results)
- [Repository layout](#repository-layout)
- [Development](#development)
- [License](#license)

## Workspace

| Crate | Role | Binary |
|---|---|---|
| [`puce-z80`](crates/puce-z80) | The CPU core (`no_std`, no `unsafe`, no dependency) | - |
| [`puce-z80-machines`](crates/puce-z80-machines) | CP/M machine, ZX Spectrum-like machine, SingleStepTests / FUSE runners, simulation session | - |
| [`puce-z80-cli`](crates/puce-z80-cli) | Command-line tool | `z80` |
| [`puce-z80-web`](crates/puce-z80-web) | Local web server and browser front end (HTTP + WebSocket) | `z80-web` |
| [`puce-z80-wasm`](crates/puce-z80-wasm) | The core compiled to WebAssembly (serverless browser demo) | - |

Only `puce-z80` is meant to be reused by other projects; the other crates are not published.

## Technical specifications

### The CPU core (`puce-z80`)

| | |
|---|---|
| Language | Rust 2021, `#![no_std]`, `#![forbid(unsafe_code)]`, no allocation, **zero dependencies** |
| Accuracy | One call to `Cpu::tick` is one clock period (T-state), wait states and bus cycles included |
| Behaviour | NMOS Zilog Z80 silicon: undocumented flags (X/Y), `WZ` (MEMPTR), `Q` (SCF/CCF), block instructions, interrupt corner cases |
| Instruction set | Complete, including the undocumented opcodes (`SLL`, `IXH/IXL/IYH/IYL`, `DD CB d op` forms, `ED` duplicates) |
| Interrupts | NMI (edge latched), INT in modes 0, 1 and 2, `EI` delay, `LD A,I/R` bug, `RETI` observation |
| Bus | `BUSRQ`/`BUSAK`, `WAIT`, `RESET`, `HALT`, `RFSH` |
| Determinism | A `Cpu` owns all of its state: no global, no `static mut`, no internal clock. Several CPUs can run in one program |
| Speed | About 12 million instructions per second (release mode, CP/M exercisers) |

**Two ways to drive it**

- `Cpu::step(&mut bus)` runs one instruction against a `Bus` (memory, ports, interrupts, wait states) and
  returns the T-states used. This is the easy way.
- `Cpu::tick(pins) -> pins` runs exactly one T-state. The host wires address, data and control pins itself,
  which is what a console or arcade board with a shared bus needs. `step` is built on `tick`.

**Variants**: `Variant::Nmos` (default) or `Variant::Cmos` (`Cpu::with_variant`). The CMOS Z80 writes 0xFF
for `OUT (C),0`, has no `LD A,I/R` + INT bug and has no `Q` effect on SCF/CCF.

**Cargo features of `puce-z80`**

| Feature | Effect |
|---|---|
| `std` | `SimpleRam::boxed()` |
| `inspect` | Last ALU operation and bus journal, for debuggers |
| `disasm` | Instruction disassembler (`puce_z80::disasm`) |
| `serde` | `Serialize`/`Deserialize` for `Regs`, `Variant` and `CpuState` (save states: `Cpu::save_state`, `Cpu::from_state`) |

**Conventions**: the NMOS silicon is the reference (z80test, SingleStepTests). Where FUSE disagrees, the
difference is listed as a known exception. After `HALT`, `PC` points just past the HALT opcode.

### Machines, tool and front end

- **CP/M machine**: loads `.com` / `.cim` programs at 0x0100 and implements BDOS functions 2 and 9. Runs PRELIM, ZEXDOC and ZEXALL.
- **ZX Spectrum-like machine**: 48K memory map, 50 Hz frames and memory contention, enough for the z80test and timing ROMs.
- **Web front end**: a local server (`std::net` HTTP + WebSocket, localhost only) and a page in French / English that
  only *displays* the state computed by the crate (registers, disassembly, memory, breakpoints, console).
- **WebAssembly**: `puce-z80-wasm` exposes the core to JavaScript through `wasm-bindgen`.

## Installation

### Requirements

- A recent stable **Rust** toolchain (edition 2021; developed with 1.98) from <https://rustup.rs>.
- Optional: `rustup target add thumbv7em-none-eabihf wasm32-unknown-unknown` to check the `no_std` and WebAssembly builds.
- Optional, browser tests only: Node.js and Playwright (see `tests/e2e`).

### Use the core in your own emulator

```toml
[dependencies]
puce-z80 = { git = "https://github.com/Farquaad56/CPU-Zilog-Z80_Crates_Rust", package = "puce-z80" }
# optional features: features = ["serde", "disasm", "inspect", "std"]
```

### Build the whole workspace

```text
git clone https://github.com/Farquaad56/CPU-Zilog-Z80_Crates_Rust
cd CPU-Zilog-Z80_Crates_Rust
cargo build --workspace --release
```

The binaries are produced in `target/release/`: `z80` and `z80-web` (`.exe` on Windows).

### Download the test ROMs (needed for the test suites only)

The ROMs are not stored in the repository. Fetch them once (checked against `tests/manifest.toml`):

```text
cargo run --release -p puce-z80-cli -- fetch
```

`tests/roms/zx48.rom` (a 16 KiB ZX Spectrum 48K ROM) is not redistributable: place it there yourself if you want it.

## Usage

### As a library

```rust
use puce_z80::{Bus, Cpu};

struct Machine {
    ram: Box<[u8; 65536]>,
}

impl Bus for Machine {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[usize::from(addr)]
    }
    fn write(&mut self, addr: u16, data: u8) {
        self.ram[usize::from(addr)] = data;
    }
    // Optional: io_read, io_write, int_ack, irq, nmi, busrq, extra_wait, on_tick.
}

fn main() {
    let mut m = Machine { ram: Box::new([0; 65536]) };
    m.ram[..4].copy_from_slice(&[0x3E, 0x01, 0x87, 0x76]); // LD A,1 ; ADD A,A ; HALT

    let mut cpu = Cpu::new();
    let mut tstates = 0;
    for _ in 0..3 {
        tstates += cpu.step(&mut m);
    }
    assert_eq!(cpu.regs.a, 2);
    assert_eq!(tstates, 15);
}
```

`Bus` hooks for real machines:

| Hook | Use |
|---|---|
| `extra_wait(kind, addr)` | Memory contention (ZX Spectrum), arcade WAIT states |
| `on_tick(pins)` | Clock a video chip, a sound chip or a CTC once per T-state |
| `irq()`, `nmi()`, `int_ack()` | INT / NMI lines and the vector byte (IM 0/1/2) |
| `busrq()` | Shared buses (Mega Drive style) |

Complete hosts are in `crates/puce-z80/examples`:

```text
cargo run -q -p puce-z80 --example pins_host      # lowest level: the host serves memory pin by pin
cargo run -q -p puce-z80 --example contended_bus  # ZX-like contention and on_tick
cargo run -q -p puce-z80 --example shared_bus     # BUSRQ/BUSAK and RESET
cargo run -q -p puce-z80 --example im2_ctc        # IM 2, vector from a peripheral, RETI snooping
```

More in [`crates/puce-z80/README.md`](crates/puce-z80/README.md) (pin API, save states).

### Command line (`z80`)

```text
z80 run <ROM>                    run a binary
z80 cpm [--max-instr N] <ROM>    run a CP/M program (.com / .cim) with BDOS calls
z80 suite <prelim|zexdoc|zexall|all> [--max-instr N] [--skip a,b] [--json out.json]
z80 ssc <DIR> [--filter 00,dd/21] [--max-fail N] [--cycles]   SingleStepTests
z80 fuse <DIR>                   FUSE test vectors
z80 z80test <file.tap> [--rom zx48.rom] [--max-frames N]
z80 bench [--seconds N]          throughput in T-states per second
z80 disasm <FILE> <ORG>          disassemble a binary
z80 fetch                        download the test ROMs
```

Examples:

```text
cargo run --release -p puce-z80-cli -- suite zexdoc
cargo run --release -p puce-z80-cli -- suite all --json suites.json
cargo run --release -p puce-z80-cli -- bench --seconds 5
```

### Web front end (`z80-web`)

```text
cargo run --release -p puce-z80-web -- --port 8080 --roms tests/roms
```

Open <http://127.0.0.1:8080>. Load a ROM from the list or upload a file, then run, pause, step, set
breakpoints and watch registers, memory and the console. `--port N` (default 8080) and `--roms DIR`
(default `tests/roms`) are the only options. The server listens on localhost only.

### WebAssembly demo

```text
rustup target add wasm32-unknown-unknown
cargo build -p puce-z80-wasm --target wasm32-unknown-unknown --release
cargo install wasm-bindgen-cli            # once
wasm-bindgen --target web --out-dir crates/puce-z80-wasm/demo/pkg target/wasm32-unknown-unknown/release/puce_z80_wasm.wasm
```

Serve `crates/puce-z80-wasm/demo` over HTTP and open `index.html`. Details in
[`crates/puce-z80-wasm/README.md`](crates/puce-z80-wasm/README.md).

## Test suites and results

| Suite | Result |
|---|---|
| PRELIM | passes |
| ZEXDOC | 67 / 67 OK |
| ZEXALL | 67 / 67 OK (undocumented flags included) |
| z80test (Patrik Rak) | 6 / 6 programs pass |
| SingleStepTests/z80 | 1,604,000 / 1,604,000 cases (state and bus cycles) |
| FUSE | 1356 / 1356 tests (6 documented exceptions where FUSE differs from the silicon) |
| Browser end-to-end | PRELIM and ZEXDOC (67 OK) run from the page |

The latest run is in [`docs/SUITES_REPORT.md`](docs/SUITES_REPORT.md).

## Repository layout

```text
crates/            the five crates above
tests/             manifest.toml (ROM checksums), baseline.json, e2e (browser tests); ROMs are downloaded here
tools/             ci.ps1 / ci.sh, hygiene, no_std check, release check, validate.ps1
docs/              method, architecture, QA protocol, decisions (docs/qa/DECISIONS.md), task specs
CHANGELOG.md       release notes
```

## Development

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Or the whole gate at once: `powershell -File tools\ci.ps1` (Windows) / `bash tools/ci.sh`.
`bash tools/release_check.sh` additionally runs the validated suites, the `no_std` / WebAssembly builds and the hygiene checks.
Design decisions are in `docs/qa/DECISIONS.md`; the architecture is described in `docs/01_ARCHITECTURE.md`.

## License

Licensed under either of MIT ([`LICENSE-MIT`](LICENSE-MIT)) or Apache-2.0 ([`LICENSE-APACHE`](LICENSE-APACHE)), at your option.
