//! The CPU: architectural registers plus the micro-cycle engine.

use crate::bus::Bus;
use crate::decode::{decode, Decoded, InsnId};
use crate::engine::plan::{Plan, Tok};
use crate::engine::{Engine, Latch};
use crate::exception;
use crate::insn;
use crate::pins::Pins;
#[cfg(feature = "inspect")]
use crate::probe::BusEvent;
use crate::regs::Regs;
use crate::state::CpuState;

/// A Motorola 68000 core. It owns all its state and advances one clock per [`Cpu::tick`].
#[derive(Clone, Copy, Debug)]
pub struct Cpu {
    /// Architectural registers.
    pub regs: Regs,
    engine: Engine,
    /// Internal latches (effective address, operand, opcode) used by the hooks.
    pub(crate) lat: Latch,
    /// Instruction being executed (owner of the hooks of the running plan).
    cur: Decoded,
    /// Wait clocks still to insert in the current bus cycle (`tick_bus` only).
    waits: u8,
    /// Processor halted (double fault); only a reset restarts it.
    halted: bool,
    /// An exception sequence is running (a bus fault now is a double fault).
    in_exception: bool,
    /// STOP executed: the CPU idles (4 clocks per step) until an interrupt or a reset.
    pub(crate) stopped: bool,
}

/// One idle bus-less cycle of a stopped CPU.
static STOP_IDLE: [Tok; 1] = [Tok::N(4)];

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    /// Creates a CPU with zeroed registers and no plan to run.
    pub const fn new() -> Self {
        Cpu {
            regs: Regs {
                d: [0; 8],
                a: [0; 7],
                usp: 0,
                ssp: 0,
                sr: 0,
                pc: 0,
                ir: 0,
                irc: 0,
            },
            engine: Engine::new(),
            lat: Latch {
                ea: 0,
                val: 0,
                ird: 0,
                aux: 0,
                aux2: 0,
                idle: 0,
                vec: 0,
                level: 0,
                ipc: 0,
                ae_step: 0,
                ae: [0; 6],
                sr0: 0,
            },
            cur: Decoded {
                id: InsnId::ILLEGAL,
                variant: 0,
            },
            waits: 0,
            halted: false,
            in_exception: false,
            stopped: false,
        }
    }

    /// Loads a plan and starts executing it at the next tick.
    #[allow(dead_code)] // used by the tests, then by the decoder (E05)
    pub(crate) fn start_plan(&mut self, plan: Plan) {
        self.engine.start(plan);
        self.run_hooks();
    }

    /// Runs the hooks the engine has passed over, in order (they take no clock).
    fn run_hooks(&mut self) {
        while let Some(k) = self.engine.pop_hook() {
            if k == exception::HK_AE_PUSH && self.in_exception {
                exception::ae_push(self);
            } else if exception::is_hook(k) {
                exception::hook(k, self);
            } else {
                insn::run_hook(self.cur.id, k, self);
            }
        }
    }

    /// Decodes IR and loads the plan of the instruction.
    ///
    /// An opcode that decodes to ILLEGAL starts nothing: the CPU stays at the instruction
    /// boundary and step returns 0. Illegal-instruction exceptions come with E11; until then the
    /// event is not journaled either (the journal has no kind for it).
    fn begin_instruction(&mut self) {
        self.cur = decode(self.regs.ir);
        self.lat.ird = self.regs.ir;
        self.lat.ipc = self.regs.pc.wrapping_sub(4);
        self.lat.sr0 = self.regs.sr;
        if self.cur.id == InsnId::ILLEGAL {
            // Illegal opcode, line A (vector 10), line F (vector 11): exception with the PC of
            // the instruction itself.
            let vector = match self.regs.ir >> 12 {
                0xA => 10,
                0xF => 11,
                _ => 4,
            };
            self.exception(vector);
            return;
        }
        let plan = insn::plan_for(self.cur.id, self.cur.variant, self);
        self.start_plan(plan);
    }

    /// At an instruction boundary: takes a pending interrupt or starts the next instruction. A
    /// stopped CPU (STOP) idles for 4 clocks until an interrupt is taken; the interrupt then
    /// returns to the instruction after STOP.
    fn start_next<B: Bus>(&mut self, bus: &mut B) {
        let ipl = bus.ipl();
        if self.stopped {
            self.regs.pc = self.regs.pc.wrapping_add(4);
            if self.take_interrupt(ipl) {
                self.stopped = false;
            } else {
                self.regs.pc = self.regs.pc.wrapping_sub(4);
                self.start_plan(&STOP_IDLE);
            }
        } else if !self.take_interrupt(ipl) {
            self.begin_instruction();
        }
    }

    /// Executes up to the next instruction boundary and returns the clocks consumed, wait states
    /// included (0 for an illegal opcode). If a plan is already running it is finished first.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let start = self.engine.clocks();
        if self.halted {
            return 0;
        }
        if self.at_instruction_boundary() {
            self.start_next(bus);
        }
        while !self.at_instruction_boundary() {
            self.tick_bus(bus);
        }
        (self.engine.clocks() - start) as u32
    }

    /// Advances one clock. `pins` carries the inputs (DTACK, DATA), the result the outputs.
    pub fn tick(&mut self, pins: Pins) -> Pins {
        let out = self.engine.step(pins, &mut self.regs, &mut self.lat);
        if let Some(f) = self.engine.take_fault() {
            self.address_error(f);
        }
        self.run_hooks();
        if self.engine.boundary() {
            self.in_exception = false;
        }
        out
    }

    /// Starts the exception sequence of `vector` (group 1 and 2 frame, 34 clocks) at the
    /// instruction boundary: the PC of the instruction at IR is pushed.
    pub fn exception(&mut self, vector: u8) {
        self.lat.vec = vector;
        self.begin_system_plan(exception::exception_plan());
    }

    /// Interrupt request of `level` (IPL0-2), sampled at the instruction boundary: taken when
    /// `level` is above the interrupt mask or is 7. Returns true when the 44-clock interrupt
    /// sequence has been started.
    pub fn take_interrupt(&mut self, level: u8) -> bool {
        let mask = ((self.regs.sr >> 8) & 7) as u8;
        if self.halted
            || !self.at_instruction_boundary()
            || level == 0
            || (level < 7 && level <= mask)
        {
            return false;
        }
        self.lat.level = level;
        self.begin_system_plan(exception::interrupt_plan());
        true
    }

    /// Starts the reset sequence (40 clocks): SR = 0x2700, SSP and PC read from vectors 0 and 1.
    pub fn reset(&mut self) {
        self.halted = false;
        self.stopped = false;
        self.begin_system_plan(exception::reset_plan());
    }

    /// True after a double fault (or [`Cpu::halt`]): the CPU runs nothing until [`Cpu::reset`].
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// True after STOP, until an interrupt is taken or the CPU is reset.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Halts the processor.
    pub fn halt(&mut self) {
        self.halted = true;
        self.engine.start(&[]);
        self.in_exception = false;
    }

    /// Reports a bus or address error. During an exception sequence it is a double fault and the
    /// CPU halts (returns true); otherwise nothing happens here yet (address error frames are
    /// E11-03, deferred).
    pub fn bus_fault(&mut self) -> bool {
        if self.in_exception {
            self.halt();
            true
        } else {
            false
        }
    }

    /// Address error: a word or long access at an odd address (or an odd PC) ends the instruction.
    /// The 14-byte group 0 frame is pushed and vector 3 is taken. During an exception sequence
    /// it is a double fault and the CPU halts.
    fn address_error(&mut self, f: crate::engine::Fault) {
        if self.in_exception {
            self.halt();
            return;
        }
        let ird = self.lat.ird;
        self.ae_side_effects(f, ird);
        let pc = exception::ae_pc(ird, f.write, self.lat.ipc, f.addr);
        let rw = if f.write { 0 } else { 0x10 };
        // The opcode pushed is that of the instruction, or the next one once the last prefetch
        // of the instruction has been done.
        // (MOVE.l to -(An) keeps its own opcode: its prefetch comes after the first write.)
        let moved = f.last_pf_done && !(ird >> 12 == 2 && (ird >> 6) & 7 == 4);
        let ir = u32::from(if moved { self.regs.ir } else { ird });
        let status = (ir & 0xFFE0) | rw | u32::from(f.fc);
        let sr = u32::from(self.regs.sr);
        self.lat.ae = [pc, sr, ir, f.addr, status, 0];
        self.lat.ae_step = 0;
        self.regs.sr = (self.regs.sr | crate::regs::S) & !crate::regs::T;
        self.lat.vec = 3;
        self.begin_system_plan(exception::address_error_plan());
    }

    /// What the aborted instruction has already done to the registers when the oracle takes the
    /// address error (table derived from the SingleStepTests, D-052).
    fn ae_side_effects(&mut self, f: crate::engine::Fault, ird: u16) {
        let hi = ird >> 12;
        let sz = (ird >> 6) & 3;
        if let Some(k) = f.post {
            // A word access through (An)+ has incremented the register; a long one has not.
            if !f.long {
                insn::run_hook(self.cur.id, k, self);
            }
        }
        if hi == 5 && ird & 0xF8 == 0xC8 {
            // DBcc: the counter decrement is not kept.
            let r = (ird & 7) as usize;
            let d = self.regs.d[r];
            self.regs.d[r] = (d & 0xFFFF_0000) | u32::from((d as u16).wrapping_add(1));
        }
        let (s, d) = ((ird & 7) as u8, ((ird >> 9) & 7) as u8);
        if matches!(hi, 0x9 | 0xD) && ird & 0x138 == 0x108 && sz == 2 {
            // ADDX.l, SUBX.l -(Ay),-(Ax): low word first, the faulting register keeps its value.
            let r = if s != d && self.regs.an(s).wrapping_add(2) == f.addr {
                s
            } else if self.regs.an(d).wrapping_add(2) == f.addr {
                d
            } else {
                s
            };
            self.regs.set_an(r, self.regs.an(r).wrapping_add(4));
        }
        if hi == 0xB && ird & 0x138 == 0x108 && sz == 2 {
            // CMPM.l: a fault on the source leaves it a step of 2; on the destination, none.
            if self.regs.an(s) == f.addr {
                self.regs.set_an(s, self.regs.an(s).wrapping_add(2));
            }
        }
        if hi == 2 && f.write {
            // MOVE.l: the flags are set at different moments of the sequence.
            let (sm, sr_) = ((ird >> 3) & 7, ird & 7);
            let dm = (ird >> 6) & 7;
            if dm == 4 {
                self.regs.set_an(d, self.regs.an(d).wrapping_add(4));
            }
            let mem = sm >= 2 && !(sm == 7 && sr_ == 4);
            let old = self.lat.sr0 & 0x1f;
            let v = self.lat.val;
            let nz = |v: u32, bits: u32| -> u16 {
                let m = if bits == 32 {
                    u32::MAX
                } else {
                    (1 << bits) - 1
                };
                let v = v & m;
                (if v >> (bits - 1) != 0 { 8 } else { 0 }) | (if v == 0 { 4 } else { 0 })
            };
            let ccr = if !mem {
                match dm {
                    2 | 3 => Some(old),
                    5 | 6 => Some((old & 0x13) | nz(v, 32)),
                    _ => None,
                }
            } else if matches!(dm, 2 | 3) || (dm == 7 && d == 1) {
                Some((old & 0x10) | nz(v, 16))
            } else {
                None
            };
            if let Some(c) = ccr {
                self.regs.sr = (self.regs.sr & !0x1f) | c;
            }
        }
    }

    fn begin_system_plan(&mut self, plan: Plan) {
        self.cur = Decoded {
            id: InsnId::ILLEGAL,
            variant: 0,
        };
        self.lat.ird = self.regs.ir;
        self.in_exception = true;
        self.start_plan(plan);
    }

    /// Extension word of the current instruction: IRC, before the `Pf` that consumes it.
    pub fn ext_word(&self) -> u16 {
        self.regs.irc
    }

    /// True when no plan is running (the next instruction can start).
    pub fn at_instruction_boundary(&self) -> bool {
        self.engine.boundary()
    }

    /// Total clocks elapsed, wait states included.
    #[allow(dead_code)] // used by the tests, then by the runners
    pub(crate) fn clocks(&self) -> u64 {
        self.engine.clocks()
    }

    /// Loads a state (IR = `prefetch[0]`, IRC = `prefetch[1]`) and returns to an instruction boundary.
    pub fn load_state(&mut self, s: &CpuState) {
        self.stopped = false;
        self.regs = Regs {
            d: s.d,
            a: s.a,
            usp: s.usp,
            ssp: s.ssp,
            sr: s.sr,
            pc: s.pc,
            ir: s.prefetch[0],
            irc: s.prefetch[1],
        };
        self.engine.start(&[]);
        self.cur = Decoded {
            id: InsnId::ILLEGAL,
            variant: 0,
        };
        self.waits = 0;
    }

    /// Current architectural state.
    pub fn state(&self) -> CpuState {
        let r = &self.regs;
        CpuState {
            d: r.d,
            a: r.a,
            usp: r.usp,
            ssp: r.ssp,
            sr: r.sr,
            pc: r.pc,
            prefetch: [r.ir, r.irc],
        }
    }

    /// One clock connected to a [`Bus`].
    ///
    /// DTACK is high except during the wait clocks the bus asked for at the start of the cycle
    /// (`Bus::wait_clocks`). `Bus::read` is called at the clock where the CPU samples data (clock 3
    /// of the cycle), `Bus::write` at the clock where the data is valid (clock 2, once DTACK is
    /// acknowledged). `Bus::on_tick` receives the output pins of every clock.
    pub fn tick_bus<B: Bus>(&mut self, bus: &mut B) {
        let cur = self
            .engine
            .current(&self.regs, &self.lat)
            .filter(|c| c.kind.is_some());
        let mut pins = Pins(0);
        let mut dtack = true;
        if let Some(c) = cur {
            if c.clock == 0 && !c.fault {
                self.waits = bus.wait_clocks(c.addr, c.is_write());
            }
            if c.clock == 2 && self.waits > 0 && !c.fault {
                self.waits -= 1;
                dtack = false;
            }
            if c.clock == 3 && !c.is_write() && !c.fault {
                let d = if c.fc == 7 {
                    let l = ((c.addr >> 1) & 7) as u8;
                    u16::from(bus.int_ack(l).unwrap_or(24 + l))
                } else {
                    bus.read(c.addr, c.fc, c.uds, c.lds)
                };
                pins = pins.with_data(d);
            }
        }
        let out = self.tick(pins.with_dtack(dtack));
        if let Some(c) = cur {
            if c.is_write() && c.clock == 2 && dtack && !c.fault {
                bus.write(c.addr, c.fc, c.uds, c.lds, out.data());
            }
        }
        bus.on_tick(out);
    }

    /// Bus events executed since the last `clear_journal` (feature `inspect`).
    #[cfg(feature = "inspect")]
    pub fn journal(&self) -> &[BusEvent] {
        self.engine.journal.as_slice()
    }

    /// Number of events that did not fit in the journal (feature `inspect`).
    #[cfg(feature = "inspect")]
    pub fn journal_overflow(&self) -> u32 {
        self.engine.journal.overflow()
    }

    /// Empties the journal and its overflow counter (feature `inspect`).
    #[cfg(feature = "inspect")]
    pub fn clear_journal(&mut self) {
        self.engine.journal.clear();
    }

    /// One clock, starting an instruction (or an interrupt sequence) first when the CPU is at a
    /// boundary. `None` when nothing runs (halted CPU, illegal opcode); `Some(true)` when the
    /// clock completed the instruction.
    pub fn step_clock<B: Bus>(&mut self, bus: &mut B) -> Option<bool> {
        if self.halted {
            return None;
        }
        if self.at_instruction_boundary() {
            self.start_next(bus);
            if self.at_instruction_boundary() {
                return None;
            }
        }
        self.tick_bus(bus);
        Some(self.at_instruction_boundary())
    }

    /// `n` calls of [`Cpu::tick_bus`].
    pub fn step_clocks<B: Bus>(&mut self, bus: &mut B, n: u32) {
        for _ in 0..n {
            self.tick_bus(bus);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plan::{AddrSrc, Tok};

    const ACK: Pins = Pins(1 << 44);

    /// Runs the plan with DTACK high except on the 1-based ticks in `low`; returns the tick count.
    fn run(cpu: &mut Cpu, plan: Plan, low: &[u32]) -> u32 {
        cpu.start_plan(plan);
        let mut n = 0;
        while !cpu.at_instruction_boundary() {
            n += 1;
            let pins = if low.contains(&n) { Pins(0) } else { ACK };
            cpu.tick(pins);
            assert!(n < 100, "plan does not terminate");
        }
        n
    }

    #[test]
    fn e02_06_read_4_clocks() {
        let mut cpu = Cpu::new();
        assert_eq!(run(&mut cpu, &[Tok::Rw(AddrSrc::Ea)], &[]), 4);
    }

    #[test]
    fn e02_06_read_wait() {
        let mut cpu = Cpu::new();
        assert_eq!(run(&mut cpu, &[Tok::Rw(AddrSrc::Ea)], &[3, 4, 5]), 7);
    }

    #[test]
    fn e02_06_long_8_clocks() {
        let mut cpu = Cpu::new();
        assert_eq!(run(&mut cpu, &[Tok::Rl(AddrSrc::Ea)], &[]), 8);
    }

    #[test]
    fn e02_06_internal_n4() {
        let mut cpu = Cpu::new();
        cpu.start_plan(&[Tok::N(4)]);
        for _ in 0..4 {
            let out = cpu.tick(Pins(0));
            assert!(!out.as_() && !out.uds() && !out.lds());
        }
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e02_06_two_tokens() {
        let mut cpu = Cpu::new();
        assert_eq!(run(&mut cpu, &[Tok::N(2), Tok::Rw(AddrSrc::Ea)], &[]), 6);
    }

    #[test]
    fn e02_06_clock_counter() {
        let mut cpu = Cpu::new();
        let n = run(&mut cpu, &[Tok::N(2), Tok::Rw(AddrSrc::Ea)], &[5, 6]);
        assert_eq!(n, 8);
        assert_eq!(cpu.clocks(), 8);
    }

    fn sample_state() -> CpuState {
        CpuState {
            d: [1, 2, 3, 4, 5, 6, 7, 8],
            a: [10, 11, 12, 13, 14, 15, 16],
            usp: 0x1000,
            ssp: 0x2000,
            sr: 0x2704,
            pc: 0x404,
            prefetch: [0x4E71, 0x1234],
        }
    }

    #[derive(Default)]
    struct Probe {
        reads: std::vec::Vec<(u32, u8, bool, bool)>,
        writes: std::vec::Vec<(u32, u8, bool, bool, u16)>,
        ticks: u32,
        waits: u8,
    }

    impl Bus for Probe {
        fn read(&mut self, addr: u32, fc: u8, uds: bool, lds: bool) -> u16 {
            self.reads.push((addr, fc, uds, lds));
            0xCAFE
        }
        fn write(&mut self, addr: u32, fc: u8, uds: bool, lds: bool, data: u16) {
            self.writes.push((addr, fc, uds, lds, data));
        }
        fn wait_clocks(&mut self, _addr: u32, _write: bool) -> u8 {
            self.waits
        }
        fn on_tick(&mut self, _pins: Pins) {
            self.ticks += 1;
        }
    }

    #[test]
    fn e03_03_load_roundtrip() {
        let mut cpu = Cpu::new();
        let s = sample_state();
        cpu.load_state(&s);
        assert_eq!(cpu.state(), s);
    }

    #[test]
    fn e03_03_ir_irc() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        assert_eq!(cpu.regs.ir, 0x4E71);
        assert_eq!(cpu.regs.irc, 0x1234);
    }

    #[test]
    fn e03_03_boundary_after_load() {
        let mut cpu = Cpu::new();
        cpu.start_plan(&[Tok::Pf]);
        assert!(!cpu.at_instruction_boundary());
        cpu.load_state(&sample_state());
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_03_tick_bus_read() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        cpu.regs.sr = 0; // user mode: data FC = 1
        cpu.lat.ea = 0x404;
        cpu.start_plan(&[Tok::Rw(AddrSrc::Ea)]);
        let mut bus = Probe::default();
        cpu.step_clocks(&mut bus, 4);
        assert_eq!(bus.reads, std::vec![(0x404, 1, true, true)]);
        assert!(bus.writes.is_empty());
        assert!(cpu.at_instruction_boundary());
        assert_eq!(bus.ticks, 4);
    }

    #[test]
    fn e03_03_tick_bus_wait() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        cpu.start_plan(&[Tok::Rw(AddrSrc::Ea)]);
        let mut bus = Probe {
            waits: 2,
            ..Probe::default()
        };
        let mut n = 0;
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut bus);
            n += 1;
            assert!(n < 50);
        }
        assert_eq!(n, 6);
        assert_eq!(bus.reads.len(), 1);
    }

    #[test]
    fn e03_03_step_clocks() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        cpu.start_plan(&[Tok::N(8)]);
        let mut bus = Probe::default();
        cpu.step_clocks(&mut bus, 4);
        assert_eq!(bus.ticks, 4);
        assert_eq!(cpu.clocks(), 4);
        assert!(!cpu.at_instruction_boundary());
    }

    fn pf(cpu: &mut Cpu, waits: u8) -> (Probe, u32) {
        cpu.start_plan(&[Tok::Pf]);
        let mut bus = Probe {
            waits,
            ..Probe::default()
        };
        let mut n = 0;
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut bus);
            n += 1;
            assert!(n < 50);
        }
        (bus, n)
    }

    #[test]
    fn e05_03_pf_shifts_queue() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        pf(&mut cpu, 0);
        assert_eq!(cpu.regs.ir, 0x1234);
        assert_eq!(cpu.regs.irc, 0xCAFE);
    }

    #[test]
    fn e05_03_pf_advances_pc() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        pf(&mut cpu, 0);
        assert_eq!(cpu.regs.pc, 0x406);
    }

    #[test]
    fn e05_03_pf_fc_super() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        let (bus, _) = pf(&mut cpu, 0);
        assert_eq!(bus.reads[0].1, 6);
    }

    #[test]
    fn e05_03_pf_fc_user() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        cpu.regs.sr = 0;
        let (bus, _) = pf(&mut cpu, 0);
        assert_eq!(bus.reads[0].1, 2);
    }

    #[test]
    fn e05_03_pf_address() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        let (bus, _) = pf(&mut cpu, 0);
        assert_eq!(bus.reads, std::vec![(0x404, 6, true, true)]);
    }

    #[test]
    fn e05_03_pf_4_clocks() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        assert_eq!(pf(&mut cpu, 0).1, 4);
        assert_eq!(pf(&mut cpu, 2).1, 6);
        // the queue shifted once per Pf, not once per wait clock
        assert_eq!(cpu.regs.pc, 0x408);
    }

    #[test]
    fn e05_03_ext_word() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        assert_eq!(cpu.ext_word(), 0x1234);
        pf(&mut cpu, 0);
        assert_eq!(cpu.ext_word(), 0xCAFE);
    }

    #[cfg(feature = "inspect")]
    #[test]
    fn e05_03_pf_journal() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        pf(&mut cpu, 0);
        let j = cpu.journal();
        assert_eq!(j.len(), 1);
        assert_eq!(
            (j[0].kind, j[0].fc, j[0].addr, j[0].data),
            (2, 6, 0x404, 0xCAFE)
        );
    }

    fn with_ir(ir: u16) -> Cpu {
        let mut cpu = Cpu::new();
        let mut st = sample_state();
        st.prefetch[0] = ir;
        cpu.load_state(&st);
        cpu
    }

    #[test]
    fn e05_05_illegal_no_advance() {
        let mut cpu = with_ir(0x0AFC); // EORI with size 11: illegal on the 68000
        let mut bus = Probe::default();
        // Illegal instruction: vector 4, 34-clock exception sequence.
        assert_eq!(cpu.step(&mut bus), 34);
        assert!(cpu.at_instruction_boundary());
        assert_eq!(cpu.lat.vec, 4);
    }

    #[test]
    fn e05_05_test_insn_runs() {
        let mut cpu = with_ir(crate::insn::testing::OP_HOOKS);
        let mut bus = Probe::default();
        assert_eq!(cpu.step(&mut bus), 4);
        assert_eq!(bus.reads.len(), 1);
        assert_eq!(cpu.regs.ir, 0x1234);
    }

    #[test]
    fn e05_05_boundary() {
        let mut cpu = with_ir(crate::insn::testing::OP_HOOKS);
        let mut bus = Probe::default();
        cpu.step(&mut bus);
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e05_05_hook_dispatch() {
        let mut cpu = with_ir(crate::insn::testing::OP_HOOKS);
        let mut bus = Probe::default();
        cpu.step(&mut bus);
        assert_eq!(cpu.regs.d[0], 2, "hook 1 ran once (D0 was 1)");
        assert_eq!(cpu.regs.d[2], 0x404, "hook 1 ran before the Pf");
        assert_eq!(cpu.regs.d[1], 0x406, "hook 2 ran after the Pf");
    }

    #[test]
    fn e05_05_step_counts_waits() {
        let mut cpu = with_ir(crate::insn::testing::OP_CLOCKS);
        let mut bus = Probe {
            waits: 2,
            ..Probe::default()
        };
        // N(2) + 2 Pf of 4 clocks, each lengthened by 2 wait clocks
        assert_eq!(cpu.step(&mut bus), 2 + 6 + 6);
        let mut cpu = with_ir(crate::insn::testing::OP_CLOCKS);
        assert_eq!(cpu.step(&mut Probe::default()), 10);
    }

    /// Bus that records writes and answers reads with a fixed word.
    #[test]
    fn d035_read_latches_operand() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        cpu.lat.ea = 0x1000;
        // word, long (two words), byte on an even and on an odd address
        cpu.start_plan(&[Tok::Rw(AddrSrc::Ea)]);
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut Probe::default());
        }
        assert_eq!(cpu.lat.val, 0xCAFE);
        cpu.start_plan(&[Tok::Rl(AddrSrc::Ea)]);
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut Probe::default());
        }
        assert_eq!(cpu.lat.val, 0xCAFE_CAFE);
        cpu.start_plan(&[Tok::Rb(AddrSrc::Ea)]);
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut Probe::default());
        }
        assert_eq!(cpu.lat.val, 0xCA);
        cpu.lat.ea = 0x1001;
        cpu.start_plan(&[Tok::Rb(AddrSrc::Ea)]);
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut Probe::default());
        }
        assert_eq!(cpu.lat.val, 0xFE);
    }

    #[test]
    fn d035_write_data_and_addresses() {
        let mut cpu = Cpu::new();
        cpu.load_state(&sample_state());
        cpu.lat.ea = 0x2000;
        cpu.lat.val = 0x1122_3344;
        let mut bus = Probe::default();
        cpu.start_plan(&[
            Tok::Wl(AddrSrc::Ea),
            Tok::Ww(AddrSrc::EaPlus2),
            Tok::Wb(AddrSrc::Ea),
        ]);
        while !cpu.at_instruction_boundary() {
            cpu.tick_bus(&mut bus);
        }
        assert_eq!(
            bus.writes,
            std::vec![
                (0x2000, 5, true, true, 0x1122),
                (0x2002, 5, true, true, 0x3344),
                (0x2002, 5, true, true, 0x3344),
                (0x2000, 5, true, false, 0x4400),
            ]
        );
    }
}
