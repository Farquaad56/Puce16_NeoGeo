//! CPU top-level struct and tick interface.

use crate::bus::{Bus, Cycle};
use crate::engine::addr::resolve;
use crate::engine::decode::{decode, plan_of, Decoded, Prefix, Table};
use crate::engine::mcycle::{fetch_pins, int_ack_pins};
use crate::engine::plan::{Addr, Kind, Reg, Tok, FETCH_PROBE};
use crate::engine::{Engine, IrqKind};
use crate::pins::Pins;
use crate::regs::Regs;
use crate::sem::sem;

/// Two chained M1 probes: the M1 in progress plus the next one (after a prefix byte).
static PROBES: [Tok; 2] = [FETCH_PROBE, FETCH_PROBE];

/// Common head of `DD CB d op` / `FD CB d op`: two M1 cycles, then `d` and `op` read as data.
static DDCB_HEAD: [Tok; 4] = [
    Tok::new(Kind::F, 4, Addr::PcInc, Reg::None, false),
    Tok::new(Kind::F, 4, Addr::PcInc, Reg::None, false),
    Tok::new(Kind::R, 3, Addr::PcInc, Reg::D, false),
    Tok::new(Kind::R, 5, Addr::PcInc, Reg::Op, false),
];

/// Which silicon is emulated.
///
/// The differences are the documented NMOS / CMOS ones:
///
/// | | `Nmos` | `Cmos` |
/// |---|---|---|
/// | `OUT (C),0` (ED 71) | writes 0x00 | writes 0xFF |
/// | `LD A,I` / `LD A,R` followed by INT | P/V is reset (bug) | no bug |
/// | `SCF` / `CCF` bits 3 and 5 | `A or (F xor Q)` | `A or F` (no Q) |
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Variant {
    /// NMOS Zilog Z80 (default): the behaviour of the original silicon.
    #[default]
    Nmos,
    /// CMOS Z80 (for example Zilog Z84C00).
    Cmos,
}

/// The Z80 CPU state: registers plus execution engine.
pub struct Cpu {
    /// Emulated variant (NMOS by default).
    pub(crate) variant: Variant,
    /// Register file (public for inspection).
    pub regs: Regs,
    /// Execution engine (internal).
    pub(crate) eng: Engine,
    /// Last ALU operation (feature `inspect`).
    #[cfg(feature = "inspect")]
    pub(crate) last_alu: Option<crate::probe::AluRecord>,
    /// PC history, write log and counters (feature `inspect`).
    #[cfg(feature = "inspect")]
    pub(crate) journal: crate::probe::Journal,
}

impl Cpu {
    /// Create a new CPU in its power-on state: the registers are zeroed, then `reset()` runs
    /// (AF = SP = 0xFFFF, PC = 0, engine idle).
    pub fn new() -> Cpu {
        let mut cpu = Cpu {
            variant: Variant::Nmos,
            regs: Regs::default(),
            eng: Engine::new(),
            #[cfg(feature = "inspect")]
            last_alu: None,
            #[cfg(feature = "inspect")]
            journal: crate::probe::Journal::default(),
        };
        cpu.reset();
        cpu
    }

    /// Create a CPU of the given variant, in its power-on state (see [`Cpu::new`]).
    pub fn with_variant(variant: Variant) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.variant = variant;
        cpu
    }

    /// The emulated variant.
    pub fn variant(&self) -> Variant {
        self.variant
    }

    /// Returns true when the CPU is at an instruction boundary (not mid-instruction).
    pub fn at_instruction_boundary(&self) -> bool {
        !self.eng.active
    }

    /// True when the instruction just executed was `EI`: interrupt acceptance is delayed by
    /// one instruction (the `ei` field of the SingleStepTests format).
    pub fn ei_pending(&self) -> bool {
        self.eng.ei_delay
    }

    /// Set the EI delay state (test harnesses: load an initial state).
    pub fn set_ei_pending(&mut self, v: bool) {
        self.eng.ei_delay = v;
    }

    /// True when the instruction just executed was `LD A,I` or `LD A,R` (the `p` field of the
    /// SingleStepTests format).
    pub fn ld_air_pending(&self) -> bool {
        self.eng.ld_air_now
    }

    /// Set the `LD A,I / LD A,R` state (test harnesses: load an initial state).
    pub fn set_ld_air_pending(&mut self, v: bool) {
        self.eng.ld_air_now = v;
    }

    /// Execute one T-state.
    ///
    /// `pins_in` contains inputs seen by the CPU during this T-state.
    /// Returns outputs after this T-state.
    pub fn tick(&mut self, pins: Pins) -> Pins {
        // RESET: the reset logic runs once when the pin goes active (even if it stays active for
        // fewer than the 3 T of the real chip: the crate resets anyway). While it is held, the
        // outputs are inactive and no instruction runs; the next tick after release starts one.
        if pins.has(Pins::RESET) {
            if !self.eng.in_reset {
                self.reset();
                self.eng.in_reset = true;
            }
            return Pins(0);
        }
        self.eng.in_reset = false;

        // Interrupt pins: INT is a level, NMI is edge-triggered and latched (E09-03).
        self.eng.int_level = pins.has(Pins::INT);
        let nmi = pins.has(Pins::NMI);
        if nmi && !self.eng.nmi_prev {
            self.eng.nmi_latch = true;
        }
        self.eng.nmi_prev = nmi;

        // Bus granted: the CPU is frozen and drives only BUSAK until BUSRQ is released.
        if self.eng.bus_granted {
            if pins.has(Pins::BUSRQ) {
                return Pins(0).with(Pins::BUSAK);
            }
            self.eng.bus_granted = false;
        }

        // Step 2: If not active, start new instruction with FETCH_PROBE
        if !self.eng.active {
            self.eng.table = Table::Main;
            self.eng.prefix = crate::engine::decode::Prefix::None;
            self.eng.fetches = 0;
            self.eng.plan = &PROBES[..1];
            self.eng.idx = 0;
            self.eng.t = 0;
            self.eng.active = true;
            // Reset cond at instruction boundary (spec: eng.cond reset to false)
            self.eng.cond = false;
            self.eng.ddcb = false;
            // Reset f_written at instruction start
            self.eng.f_written = false;
            self.eng.wz_post = None;
            self.eng.wz_start = self.regs.wz;
            // HALT: the next 'instruction' is an M1 NOP cycle at PC (E09-02).
            self.eng.halt_cycle = self.regs.halted;
            // Interrupt acceptance at the instruction boundary, before the normal fetch.
            self.eng.irq = crate::engine::IrqKind::None;
            self.eng.int_seq = false;
            self.eng.last_ld_air = self.eng.ld_air_now;
            self.eng.ld_air_now = false;
            if self.eng.nmi_latch {
                self.eng.nmi_latch = false;
                self.start_nmi();
            } else if self.eng.int_level && self.regs.iff1 && !self.eng.ei_delay {
                self.start_int();
            }
            // The instruction after EI has now been given its one-instruction grace.
            self.eng.ei_delay = false;
            #[cfg(feature = "inspect")]
            if !self.eng.halt_cycle && self.eng.irq == IrqKind::None {
                self.journal.history.push(self.regs.pc);
                self.journal.insns += 1;
            }
        }

        // Step 3: Get current token
        let mut cur = self.eng.plan[self.eng.idx];

        // At t0 of bus tokens, resolve address and store in cur_addr
        if self.eng.t == 0 && cur.is_bus() {
            let port = matches!(cur.kind, Kind::N | Kind::O);
            // An M1 cycle always fetches from PC (table F tokens may carry Addr::None).
            if cur.kind == Kind::F
                && (self.eng.halt_cycle || self.eng.irq == IrqKind::Nmi || cur.len == 7)
            {
                // HALT NOP / NMI acknowledge cycle: the address is PC and PC does not change.
                self.eng.cur_addr = self.regs.pc;
            } else {
                let a = if cur.kind == Kind::F {
                    Addr::PcInc
                } else {
                    cur.addr
                };
                self.eng.cur_addr = resolve(self, a, port);
            }
        }

        // Step 4: Calculate pins_out for (cur, t)
        let mut out = Pins(0).set_addr(self.eng.cur_addr);

        match cur.kind {
            Kind::F => {
                // R is incremented (7 bits, bit 7 kept) at the first T of every M1.
                if self.eng.t == 0 {
                    let r = self.regs.r;
                    self.regs.r = (r & 0x80) | (r.wrapping_add(1) & 0x7F);
                }
                // The refresh address carries R as it was before this M1 incremented it
                // (SingleStepTests, docs/specs/33_SINGLESTEP_FORMAT.md).
                let r_refresh = (self.regs.r & 0x80) | (self.regs.r.wrapping_sub(1) & 0x7F);
                out = fetch_pins(
                    self.eng.cur_addr,
                    self.regs.i,
                    r_refresh,
                    self.eng.t,
                    cur.len,
                    self.eng.halt_cycle,
                );
                // Opcode latched at T3: decode and possibly replace the current token.
                // The byte read during a HALT cycle is ignored (no decode).
                if self.eng.t == 2
                    && !self.eng.halt_cycle
                    && self.eng.irq != IrqKind::Nmi
                    && cur.len != 7
                {
                    let byte = pins.data();
                    self.eng.latched = byte;
                    self.decode_m1(byte);
                    cur = self.eng.plan[self.eng.idx];
                }
                // INT acknowledge (IA7, len 7): M1 + IORQ, the bus byte is latched at the last
                // wait T-state. UNKNOWN - to confirm: exact latch T (calibrated in E13-10).
                if cur.len == 7 {
                    out = int_ack_pins(self.eng.cur_addr, self.eng.t);
                    if self.eng.t == 5 {
                        self.eng.latched = pins.data();
                    }
                }
            }
            Kind::R => {
                // Memory read: t0=A, t1=A+MREQ+RD, t2=A (data latched)
                if self.eng.t == 1 {
                    out = out.with(Pins::MREQ | Pins::RD);
                }
                // At T3 (t==2), latch data and store to destination register
                if self.eng.t == 2 {
                    let data = pins.data();
                    self.eng.latched = data;
                    self.eng.store(&mut self.regs, cur.reg, data);
                }
            }
            Kind::W => {
                // Memory write: t0=A+D, t1=A+D+MREQ+WR, t2=A+D
                let data = self.eng.load(&self.regs, cur.reg);
                out = out.set_data(data);
                #[cfg(feature = "inspect")]
                if self.eng.t == 0 {
                    self.journal.writes.push(self.eng.cur_addr);
                }
                if self.eng.t == 1 {
                    out = out.with(Pins::MREQ | Pins::WR);
                }
            }
            Kind::N => {
                // Port read: t0=A, t1=A, t2=A+IORQ+RD (+TW), t3=A (data latched)
                if self.eng.t == 2 {
                    out = out.with(Pins::IORQ | Pins::RD);
                }
                // At T4 (t==3), latch data and store to destination register
                if self.eng.t == 3 {
                    let data = pins.data();
                    self.eng.latched = data;
                    self.eng.store(&mut self.regs, cur.reg, data);
                }
            }
            Kind::O => {
                // Port write: t0=A+D, t1=A+D, t2=A+D+IORQ+WR (+TW), t3=A+D
                let data = self.eng.load(&self.regs, cur.reg);
                out = out.set_data(data);
                if self.eng.t == 2 {
                    out = out.with(Pins::IORQ | Pins::WR);
                }
            }
            Kind::I => {
                // Internal cycle: last bus address, no control signals
            }
            Kind::X => {
                // Hook - should not reach here (handled in step 5)
            }
        }

        // Last complete bus cycle (feature inspect).
        #[cfg(feature = "inspect")]
        {
            let t = self.eng.t;
            let done = match cur.kind {
                Kind::F => t == if cur.len == 7 { 5 } else { 2 },
                Kind::R | Kind::W => t == 2,
                Kind::N | Kind::O => t == 3,
                _ => false,
            };
            if done {
                let (kind, data) = match cur.kind {
                    Kind::F => (0, pins.data()),
                    Kind::R => (1, pins.data()),
                    Kind::W => (2, out.data()),
                    Kind::N => (3, pins.data()),
                    _ => (4, out.data()),
                };
                self.journal.bus = crate::probe::BusView {
                    addr: self.eng.cur_addr,
                    data,
                    kind,
                    pins: out,
                };
            }
        }

        // WAIT state insertion: at sampling points, if WAIT pin is active,
        // insert a wait T-state (don't advance t, increment ts_wait).
        let sampling = match cur.kind {
            Kind::F | Kind::R | Kind::W => self.eng.t == 1,
            Kind::N | Kind::O => self.eng.t == 2,
            _ => false, // I and X don't sample WAIT
        };

        if sampling && pins.has(Pins::WAIT) {
            self.eng.ts_wait += 1;
            #[cfg(feature = "inspect")]
            {
                self.journal.tstates += 1;
            }
            return out; // Repeat same outputs on next tick at same t
        }

        // Increment t and ts counter
        self.eng.t += 1;
        self.eng.ts += 1;
        #[cfg(feature = "inspect")]
        {
            self.journal.tstates += 1;
        }

        // Step 5: If token complete, advance to next
        if self.eng.t == cur.len {
            // After an M1 the address bus keeps the refresh address (I:R) during the internal
            // T-states that follow (SingleStepTests, docs/specs/33_SINGLESTEP_FORMAT.md).
            if cur.kind == Kind::F {
                let r_before = (self.regs.r & 0x80) | (self.regs.r.wrapping_sub(1) & 0x7F);
                self.eng.cur_addr = (u16::from(self.regs.i) << 8) | u16::from(r_before);
            }
            if self.eng.ddcb && cur.kind == Kind::R && cur.reg == Reg::Op {
                // End of `DD CB d op`: the operation byte selects the real plan.
                self.eng.table = Table::DdCb;
                self.eng.op = self.eng.opb;
                self.eng.plan = plan_of(Table::DdCb, self.eng.opb);
                self.eng.ddcb = false;
                self.eng.idx = 3;
            }
            self.eng.idx += 1;
            self.eng.t = 0;

            // Run any hooks that follow this token, skipping conditional tokens when cond is false
            while self.eng.idx < self.eng.plan.len() {
                let next = &self.eng.plan[self.eng.idx];
                if next.is_hook() {
                    let hook = next.hook_no();
                    if self.eng.int_seq {
                        self.int_hook(hook);
                    } else {
                        sem(self, self.eng.table, self.eng.op, hook);
                    }
                    self.eng.idx += 1;
                } else if next.cond && !self.eng.cond {
                    // Skip conditional token (0 T-states)
                    self.eng.idx += 1;
                } else {
                    break;
                }
            }

            // If plan complete, mark as inactive (instruction boundary)
            if self.eng.idx >= self.eng.plan.len() {
                // Final WZ that depends on the last bus cycle (LD (nn),A, OUT (n),A, ...).
                if let Some(v) = self.eng.wz_post.take() {
                    self.regs.wz = v;
                }
                // Q factor: at instruction boundary, copy F to Q if F was written
                if self.eng.f_written {
                    self.regs.q = self.regs.f;
                } else {
                    self.regs.q = 0;
                }
                self.eng.active = false;
            }

            // BUSRQ is sampled at the last T of every bus cycle (not for I cycles or hooks).
            if cur.is_bus() && pins.has(Pins::BUSRQ) {
                self.eng.bus_granted = true;
            }
        }

        out
    }

    /// Apply the decode reaction for the opcode byte latched at T3 of an M1 cycle.
    fn decode_m1(&mut self, byte: u8) {
        match decode(self.eng.table, byte) {
            Decoded::Plan(tbl, plan) => {
                // The M1 in progress becomes plan[fetches] (F4, F5 or F6).
                self.eng.table = tbl;
                self.eng.plan = plan;
                self.eng.op = byte;
                self.eng.idx = self.eng.fetches as usize;
                self.eng.fetches += 1;
            }
            Decoded::Prefix(tbl) => {
                self.eng.table = tbl;
                if byte == 0xDD {
                    self.eng.prefix = Prefix::Dd;
                } else if byte == 0xFD {
                    self.eng.prefix = Prefix::Fd;
                }
                self.eng.fetches += 1;
                self.eng.plan = &PROBES;
                self.eng.idx = 0;
            }
            Decoded::IndexCb => {
                self.eng.ddcb = true;
                self.eng.fetches += 1;
                self.eng.plan = &DDCB_HEAD;
                self.eng.idx = 1;
            }
            Decoded::Restart(b) => {
                // The previous prefix is consumed without effect (its M1 already cost
                // 4 T). The byte just fetched is the new first prefix M1 of the plan.
                match b {
                    0xDD => {
                        self.eng.prefix = Prefix::Dd;
                        self.eng.table = Table::DdFd;
                    }
                    0xFD => {
                        self.eng.prefix = Prefix::Fd;
                        self.eng.table = Table::DdFd;
                    }
                    _ => {
                        self.eng.prefix = Prefix::None;
                        self.eng.table = Table::Ed;
                    }
                }
                self.eng.restart_cost += 4;
                self.eng.fetches = 1;
                self.eng.plan = &PROBES;
                self.eng.idx = 0;
            }
        }
    }

    /// Total WAIT T-states inserted during the current (or last completed) instruction.
    pub fn wait_states(&self) -> u32 {
        self.eng.ts_wait
    }

    /// T-states of the current (or last completed) instruction, excluding WAIT states.
    pub fn instruction_tstates(&self) -> u32 {
        self.eng.ts
    }

    /// Execute one instruction and return the T-states consumed (WAIT included).
    ///
    /// Convenience API on top of `tick`: it serves memory and ports through the
    /// `Bus` trait, inserts `extra_wait` states and stops at the next instruction
    /// boundary. There is a single timing implementation (D-002).
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let mut tstates: u32 = 0;
        let mut last = Pins(0);
        let mut read_data: Option<u8> = None;
        let mut write_done = false;
        let mut wait_left: u8 = 0;

        loop {
            let mut pins_in = Pins(0);

            // A read requested by the previous outputs: the data goes into this tick.
            // The bus is called once per cycle even if the CPU repeats its outputs (WAIT).
            let mem_rd = last.has(Pins::MREQ) && last.has(Pins::RD);
            let io_rd = last.has(Pins::IORQ) && last.has(Pins::RD) && !last.has(Pins::M1);
            let ack = last.is_int_ack();
            if mem_rd || io_rd || ack {
                let data = *read_data.get_or_insert_with(|| {
                    if mem_rd {
                        bus.read(last.addr())
                    } else if io_rd {
                        bus.io_read(last.addr())
                    } else {
                        bus.int_ack()
                    }
                });
                pins_in = pins_in.set_data(data);
            } else {
                read_data = None;
            }

            // A write requested by the previous outputs: performed once per cycle.
            let mem_wr = last.has(Pins::MREQ) && last.has(Pins::WR);
            let io_wr = last.has(Pins::IORQ) && last.has(Pins::WR);
            if mem_wr || io_wr {
                if !write_done {
                    if mem_wr {
                        bus.write(last.addr(), last.data());
                    } else {
                        bus.io_write(last.addr(), last.data());
                    }
                    write_done = true;
                }
            } else {
                write_done = false;
            }

            // Hold WAIT while the next tick is a sampling point of the current cycle.
            if wait_left > 0 && self.samples_wait_next() {
                pins_in = pins_in.with(Pins::WAIT);
                wait_left -= 1;
            }

            // INT, NMI and BUSRQ are only forwarded here (handled in E09).
            if bus.irq() {
                pins_in = pins_in.with(Pins::INT);
            }
            if bus.nmi() {
                pins_in = pins_in.with(Pins::NMI);
            }
            if bus.busrq() {
                pins_in = pins_in.with(Pins::BUSRQ);
            }

            // At the first T of a bus cycle the address is valid: ask for wait states.
            let starting = self.cycle_starting();
            last = self.tick(pins_in);
            bus.on_tick(last);
            tstates += 1;
            if let Some(kind) = starting {
                wait_left = bus.extra_wait(kind, last.addr());
            }

            debug_assert!(
                tstates < 100,
                "step() ran 100 ticks without an instruction boundary"
            );
            if self.at_instruction_boundary() {
                break;
            }
        }

        tstates
    }

    /// Kind of bus cycle that the next tick starts (its t0), if any.
    fn cycle_starting(&self) -> Option<Cycle> {
        if !self.eng.active {
            return Some(Cycle::Fetch);
        }
        if self.eng.t != 0 {
            return None;
        }
        match self.eng.plan[self.eng.idx].kind {
            Kind::F if self.eng.plan[self.eng.idx].len == 7 => Some(Cycle::IntAck),
            Kind::F => Some(Cycle::Fetch),
            Kind::R => Some(Cycle::MemRead),
            Kind::W => Some(Cycle::MemWrite),
            Kind::N => Some(Cycle::IoRead),
            Kind::O => Some(Cycle::IoWrite),
            Kind::I | Kind::X => None,
        }
    }

    /// True if the next tick samples the WAIT pin (T2 of F/R/W, T3 of N/O).
    fn samples_wait_next(&self) -> bool {
        if !self.eng.active {
            return false;
        }
        match self.eng.plan[self.eng.idx].kind {
            Kind::F | Kind::R | Kind::W => self.eng.t == 1,
            Kind::N | Kind::O => self.eng.t == 2,
            Kind::I | Kind::X => false,
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Cpu {
    /// Start executing a given plan (for testing).
    pub(crate) fn start_plan(&mut self, plan: &'static [Tok]) {
        self.eng.plan = plan;
        self.eng.idx = 0;
        self.eng.t = 0;
        self.eng.active = true;
        self.eng.ts = 0;
        self.eng.ts_wait = 0;
        // Reset cond at instruction boundary (spec: eng.cond reset to false)
        self.eng.cond = false;

        // Run any initial hooks, skipping conditional tokens when cond is false
        while self.eng.idx < self.eng.plan.len() {
            let next = &self.eng.plan[self.eng.idx];
            if next.is_hook() {
                sem(self, self.eng.table, self.eng.op, next.hook_no());
                self.eng.idx += 1;
            } else if next.cond && !self.eng.cond {
                // Skip conditional token (0 T-states)
                self.eng.idx += 1;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::decode::{Prefix, Table};
    use std::boxed::Box;

    #[test]
    fn e03_01_new_idle() {
        let cpu = Cpu::new();
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_01_default_eq_new() {
        let default_cpu = Cpu::default();
        let new_cpu = Cpu::new();
        assert_eq!(default_cpu.regs, new_cpu.regs);
    }

    #[test]
    fn e03_01_engine_defaults() {
        let eng = Engine::new();
        assert_eq!(eng.table, Table::Main);
        assert_eq!(eng.prefix, Prefix::None);
        assert_eq!(eng.idx, 0);
        assert_eq!(eng.t, 0);
        assert!(!eng.cond);
        assert_eq!(eng.tmp_l, 0);
        assert_eq!(eng.tmp_h, 0);
    }

    #[test]
    fn e03_01_fetch_probe() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let probe = FETCH_PROBE;
        assert_eq!(probe.kind, Kind::F);
        assert_eq!(probe.len, 4);
        assert_eq!(probe.addr, Addr::PcInc);
        assert_eq!(probe.reg, Reg::None);
    }

    // E03-07: Tick loop execution tests

    #[test]
    fn e03_07_read_plan_3t() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1234);
        cpu.start_plan(plan);

        // Tick 1 (t=0): address on bus, no control signals
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x1234);
        assert!(!out1.has(Pins::MREQ | Pins::RD));

        // Tick 2 (t=1): MREQ|RD active
        let out2 = cpu.tick(Pins(0));
        assert!(out2.has(Pins::MREQ | Pins::RD));

        // Tick 3 (t=2): data latched into tmp_l, instruction complete
        let in_data = Pins(0).set_data(0xAB);
        let _out3 = cpu.tick(in_data);
        assert_eq!(cpu.eng.tmp_l, 0xAB);
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_write_plan() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::W, 3, Addr::Hl, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x5678);
        cpu.eng.tmp_l = 0x55;
        cpu.start_plan(plan);

        // Tick 1 (t=0): address and data on bus, no control signals
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x5678);
        assert_eq!(out1.data(), 0x55);
        assert!(!out1.has(Pins::MREQ | Pins::WR));

        // Tick 2 (t=1): MREQ|WR active with data
        let out2 = cpu.tick(Pins(0));
        assert!(out2.has(Pins::MREQ | Pins::WR));
        assert_eq!(out2.data(), 0x55);

        // Tick 3 (t=2): instruction complete
        let _out3 = cpu.tick(Pins(0));
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_internal_plan() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(
            Kind::I,
            5,
            Addr::None,
            Reg::None,
            false,
        )]));
        let mut cpu = Cpu::new();
        cpu.start_plan(plan);

        // Internal cycle: no control signals for all 5 ticks
        for _ in 0..4 {
            let out = cpu.tick(Pins(0));
            assert!(!out.has(Pins::MREQ | Pins::IORQ | Pins::RD | Pins::WR));
            assert!(!cpu.at_instruction_boundary());
        }
        // Last tick completes the instruction
        let _out5 = cpu.tick(Pins(0));
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_two_cycles() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([
            Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false),
            Tok::new(Kind::R, 3, Addr::Bc, Reg::H, false),
        ]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1000);
        cpu.regs.set_bc(0x2000);
        cpu.start_plan(plan);

        // First cycle: 3 ticks
        for _ in 0..3 {
            let _out = cpu.tick(Pins(0));
            assert!(!cpu.at_instruction_boundary());
        }

        // Second cycle: 3 more ticks, different address
        let out4 = cpu.tick(Pins(0).set_data(0x11));
        assert_eq!(out4.addr(), 0x2000);
        for _ in 0..1 {
            let _out = cpu.tick(Pins(0));
            assert!(!cpu.at_instruction_boundary());
        }
        // After 6 ticks total, instruction complete
        let _out7 = cpu.tick(Pins(0).set_data(0x22));
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_hook_runs_after() {
        use crate::engine::plan::{Addr, Kind, Reg};
        // Plan: R3 followed by hook X0
        let plan = Box::leak(Box::new([
            Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false),
            Tok::new(Kind::X, 0, Addr::None, Reg::None, false),
        ]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1000);
        cpu.start_plan(plan);

        // Tick through the R3 cycle (3 ticks)
        for _ in 0..2 {
            cpu.tick(Pins(0));
        }
        // On tick 3, the hook should run after pins_out is computed
        let _out = cpu.tick(Pins(0).set_data(0x42));
        assert_eq!(cpu.eng.tmp_l, 0x42);
        // Hook ran, so idx advanced past it; instruction complete
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_io_plan() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::N, 4, Addr::An, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.a = 0x12;
        cpu.regs.wz = 0x0034; // port number n in Z
        cpu.start_plan(plan);

        // Tick 1 (t=0): port address on bus
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x1234);
        assert!(!out1.has(Pins::IORQ | Pins::RD));

        // Tick 2 (t=1): still just address
        let out2 = cpu.tick(Pins(0));
        assert!(!out2.has(Pins::IORQ | Pins::RD));

        // Tick 3 (t=2): IORQ|RD active (+TW auto)
        let out3 = cpu.tick(Pins(0));
        assert!(out3.has(Pins::IORQ | Pins::RD));

        // Tick 4 (t=3): data latched, instruction complete
        let in_data = Pins(0).set_data(0x99);
        let _out4 = cpu.tick(in_data);
        assert_eq!(cpu.eng.tmp_l, 0x99);
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_boundary_after_last() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(
            Kind::I,
            2,
            Addr::None,
            Reg::None,
            false,
        )]));
        let mut cpu = Cpu::new();
        cpu.start_plan(plan);

        assert!(!cpu.at_instruction_boundary());
        cpu.tick(Pins(0));
        assert!(!cpu.at_instruction_boundary());
        cpu.tick(Pins(0));
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e03_07_xy_plan() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::R, 3, Addr::Xy, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.ix = 0x1000;
        cpu.eng.prefix = Prefix::Dd;
        cpu.eng.disp = 0xFE; // -2 signed
        cpu.start_plan(plan);

        // Tick 1: address should be IX + disp = 0x0FFE, and wz updated
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x0FFE);
        assert_eq!(cpu.regs.wz, 0x0FFE);

        // Complete the cycle
        cpu.tick(Pins(0));
        let in_data = Pins(0).set_data(0xCC);
        cpu.tick(in_data);
        assert_eq!(cpu.eng.tmp_l, 0xCC);
    }

    // E03-08: Conditional tokens (?) tests

    #[test]
    fn e03_08_cond_false_skips() {
        use crate::engine::plan::{Addr, Kind, Reg};
        // Plan: R3, ?I5 (conditional), R3 - cond=false so I5 is skipped
        let plan = Box::leak(Box::new([
            Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false),
            Tok::new(Kind::I, 5, Addr::None, Reg::None, true), // conditional
            Tok::new(Kind::R, 3, Addr::Bc, Reg::H, false),
        ]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1000);
        cpu.regs.set_bc(0x2000);
        cpu.start_plan(plan);

        // First R3: 3 ticks
        for _ in 0..3 {
            let _out = cpu.tick(Pins(0));
        }
        // After first token, hooks run (none), then conditional I5 is skipped (cond=false)
        assert!(!cpu.at_instruction_boundary());

        // Second R3: 3 more ticks
        for _ in 0..3 {
            let _out = cpu.tick(Pins(0));
        }
        assert!(cpu.at_instruction_boundary());
        // Total: 6 T-states (I5 skipped)
        assert_eq!(cpu.eng.ts, 6);
    }

    #[test]
    fn e03_08_cond_true_runs() {
        use crate::engine::plan::{Addr, Kind, Reg};
        // Same plan but cond set to true before the conditional token is reached
        let plan = Box::leak(Box::new([
            Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false),
            Tok::new(Kind::I, 5, Addr::None, Reg::None, true), // conditional
            Tok::new(Kind::R, 3, Addr::Bc, Reg::H, false),
        ]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1000);
        cpu.regs.set_bc(0x2000);
        cpu.start_plan(plan);

        // First R3: tick 1, then set cond=true before token completes
        let _out = cpu.tick(Pins(0));
        cpu.eng.cond = true;
        for _ in 0..2 {
            let _out = cpu.tick(Pins(0));
        }
        // Now I5 should run (cond is true): 5 ticks
        for _ in 0..5 {
            let _out = cpu.tick(Pins(0));
        }

        // Second R3: 3 more ticks
        for _ in 0..3 {
            let _out = cpu.tick(Pins(0));
        }
        assert!(cpu.at_instruction_boundary());
        // Total: 11 T-states (I5 executed)
        assert_eq!(cpu.eng.ts, 11);
    }

    #[test]
    fn e03_08_cond_reset() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(
            Kind::I,
            2,
            Addr::None,
            Reg::None,
            false,
        )]));
        let mut cpu = Cpu::new();

        // First instruction: set cond=true during execution
        cpu.start_plan(plan);
        cpu.eng.cond = true;
        for _ in 0..2 {
            cpu.tick(Pins(0));
        }
        assert!(cpu.at_instruction_boundary());
        assert!(cpu.eng.cond);

        // Start second instruction - cond should be reset to false by start_plan
        cpu.start_plan(plan);
        assert!(!cpu.eng.cond);
    }

    #[test]
    fn e03_08_djnz_shape() {
        use crate::engine::plan::{Addr, Kind, Reg};
        // DJNZ shape: F5 R3 X0 ?I5 X1 (simplified for test)
        // Without cond: 5+3 = 8 ticks (skip I5 and hook contributes 0)
        let plan = Box::leak(Box::new([
            Tok::new(Kind::I, 5, Addr::None, Reg::None, false),
            Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false),
            Tok::new(Kind::X, 0, Addr::None, Reg::None, false), // hook X0 (sets cond)
            Tok::new(Kind::I, 5, Addr::None, Reg::None, true),  // conditional I5
            Tok::new(Kind::X, 1, Addr::None, Reg::None, false), // hook X1
        ]));

        // Test with cond=false (hook doesn't set it) - should skip I5: 8 ticks
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1000);
        cpu.start_plan(plan);
        for _ in 0..8 {
            let _out = cpu.tick(Pins(0));
        }
        assert!(cpu.at_instruction_boundary());
        assert_eq!(cpu.eng.ts, 8);

        // Test with cond=true (hook sets it) - should run I5: 13 ticks
        let mut cpu2 = Cpu::new();
        cpu2.regs.set_hl(0x1000);
        cpu2.start_plan(plan);
        // Run through F5 (5 ticks), then during R3 set cond=true before R3 completes
        for _ in 0..6 {
            let _out = cpu2.tick(Pins(0));
        }
        // Set cond=true to simulate X0 hook setting it (before R3 finishes)
        cpu2.eng.cond = true;
        // Complete R3 (1 more tick), then I5 runs (5 ticks): 6 more ticks total
        for _ in 0..7 {
            let _out = cpu2.tick(Pins(0));
        }
        assert!(cpu2.at_instruction_boundary());
        assert_eq!(cpu2.eng.ts, 13);
    }

    // E03-09: WAIT state insertion tests

    #[test]
    fn e03_09_wait_extends_read() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1234);
        cpu.start_plan(plan);

        // Tick 1 (t=0): address on bus, no WAIT sampling
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x1234);

        // Tick 2 (t=1): MREQ|RD active, WAIT sampled here - apply WAIT for 2 ticks
        let out2 = cpu.tick(Pins(Pins::WAIT));
        assert!(out2.has(Pins::MREQ | Pins::RD));
        assert_eq!(cpu.wait_states(), 1);

        // Tick 3 (t=1 still): WAIT still active, another wait state inserted
        let out3 = cpu.tick(Pins(Pins::WAIT));
        assert!(out3.has(Pins::MREQ | Pins::RD));
        assert_eq!(cpu.wait_states(), 2);

        // Tick 4 (t=1 now advances): WAIT released, normal progression resumes
        let out4 = cpu.tick(Pins(0));
        assert!(out4.has(Pins::MREQ | Pins::RD));

        // Tick 5 (t=2): data latched, instruction complete
        let in_data = Pins(0).set_data(0xAB);
        let _out5 = cpu.tick(in_data);
        assert_eq!(cpu.eng.tmp_l, 0xAB);
        assert!(cpu.at_instruction_boundary());

        // Total: 3 normal T-states + 2 wait states = 5 ticks
        assert_eq!(cpu.wait_states(), 2);
    }

    #[test]
    fn e03_09_wait_not_in_internal() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(
            Kind::I,
            4,
            Addr::None,
            Reg::None,
            false,
        )]));
        let mut cpu = Cpu::new();
        cpu.start_plan(plan);

        // Internal cycle: WAIT pin is ignored for all 4 ticks
        for _ in 0..3 {
            let out = cpu.tick(Pins(Pins::WAIT));
            assert!(!out.has(Pins::MREQ | Pins::IORQ | Pins::RD | Pins::WR));
        }
        let _out4 = cpu.tick(Pins(0));
        assert!(cpu.at_instruction_boundary());

        // No wait states inserted for internal cycle
        assert_eq!(cpu.wait_states(), 0);
    }

    #[test]
    fn e03_09_wait_io() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::N, 4, Addr::An, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.a = 0x12;
        cpu.regs.wz = 0x0034; // port number n in Z
        cpu.start_plan(plan);

        // Tick 1 (t=0): port address on bus
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x1234);

        // Tick 2 (t=1): still just address, no WAIT sampling for N at t=1
        let out2 = cpu.tick(Pins(Pins::WAIT));
        assert!(!out2.has(Pins::IORQ | Pins::RD));

        // Tick 3 (t=2): IORQ|RD active (+TW auto), WAIT sampled here -> wait state inserted
        let out3 = cpu.tick(Pins(Pins::WAIT));
        assert!(out3.has(Pins::IORQ | Pins::RD));
        assert_eq!(cpu.wait_states(), 1);

        // Tick 4 (t=2 still): WAIT released, advance to t=3
        let _out4 = cpu.tick(Pins(0));

        // Tick 5 (t=3): data latched here, then advance to t=4 -> instruction complete
        let in_data = Pins(0).set_data(0x99);
        let _out5 = cpu.tick(in_data);
        assert_eq!(cpu.eng.tmp_l, 0x99);
        assert!(cpu.at_instruction_boundary());

        assert_eq!(cpu.wait_states(), 1);
    }

    #[test]
    fn e03_09_outputs_repeat() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1234);
        cpu.start_plan(plan);

        // Tick 1 (t=0): address on bus
        let out1 = cpu.tick(Pins(0));
        assert_eq!(out1.addr(), 0x1234);

        // Tick 2 (t=1): MREQ|RD active - capture this output
        let out_normal = cpu.tick(Pins(Pins::WAIT));
        assert!(out_normal.has(Pins::MREQ | Pins::RD));
        assert_eq!(out_normal.addr(), 0x1234);

        // Tick 3 (t=1 still): WAIT active, outputs must be identical to tick 2
        let out_wait = cpu.tick(Pins(Pins::WAIT));
        assert_eq!(out_wait.0, out_normal.0); // Exact same pin values
        assert!(out_wait.has(Pins::MREQ | Pins::RD));
        assert_eq!(out_wait.addr(), 0x1234);

        // Release WAIT and complete instruction
        cpu.tick(Pins(0));
        let in_data = Pins(0).set_data(0xAB);
        cpu.tick(in_data);
    }

    #[test]
    fn e03_09_no_wait_total() {
        use crate::engine::plan::{Addr, Kind, Reg};
        let plan = Box::leak(Box::new([Tok::new(Kind::R, 3, Addr::Hl, Reg::L, false)]));
        let mut cpu = Cpu::new();
        cpu.regs.set_hl(0x1234);
        cpu.start_plan(plan);

        // Tick through with WAIT inserted at t=1 for 2 ticks
        cpu.tick(Pins(0)); // t=0
        cpu.tick(Pins(Pins::WAIT)); // t=1, wait state 1
        cpu.tick(Pins(Pins::WAIT)); // t=1, wait state 2
        cpu.tick(Pins(0)); // t=1 advances to t=2
        let in_data = Pins(0).set_data(0xAB);
        cpu.tick(in_data); // t=2 completes instruction

        assert!(cpu.at_instruction_boundary());
        // instruction_tstates excludes wait states: should be 3 (the R3 token length)
        assert_eq!(cpu.instruction_tstates(), 3);
        // wait_states counts only the inserted waits
        assert_eq!(cpu.wait_states(), 2);
    }

    /// Run one instruction from `mem` (PC=0) with a trivial memory model.
    /// Returns the CPU and the number of T-states executed.
    fn e03_10_run(mem: &[u8]) -> (Cpu, u32) {
        let mut cpu = Cpu::new();
        e03_10_run_cpu(&mut cpu, mem)
    }

    fn e03_10_run_cpu(cpu: &mut Cpu, mem: &[u8]) -> (Cpu, u32) {
        let mut last = Pins(0);
        let mut n = 0u32;
        loop {
            let inp = if last.has(Pins::MREQ) && last.has(Pins::RD) {
                let b = mem.get(last.addr() as usize).copied().unwrap_or(0);
                Pins(0).set_data(b)
            } else {
                Pins(0)
            };
            last = cpu.tick(inp);
            n += 1;
            if cpu.at_instruction_boundary() || n > 64 {
                break;
            }
        }
        let done = core::mem::take(cpu);
        (done, n)
    }

    #[test]
    fn e03_10_nop_4t() {
        let (cpu, n) = e03_10_run(&[0x00]);
        assert_eq!(n, 4);
        assert_eq!(cpu.regs.pc, 1);
        assert_eq!(cpu.regs.r, 1);
    }

    #[test]
    fn e03_10_ld_bc_nn_10t() {
        let (cpu, n) = e03_10_run(&[0x01, 0x34, 0x12]);
        assert_eq!(n, 10);
        assert_eq!(cpu.regs.pc, 3);
    }

    #[test]
    fn e03_10_cb_prefix_8t() {
        let (cpu, n) = e03_10_run(&[0xCB, 0x00]);
        assert_eq!(n, 8);
        assert_eq!(cpu.regs.r, 2);
        assert_eq!(cpu.regs.pc, 2);
    }

    #[test]
    fn e03_10_ed_nop_8t() {
        let (_cpu, n) = e03_10_run(&[0xED, 0x00]);
        assert_eq!(n, 8);
    }

    #[test]
    fn e03_10_dd_ld_ix_nn_14t() {
        let (cpu, n) = e03_10_run(&[0xDD, 0x21, 0x34, 0x12]);
        assert_eq!(n, 14);
        assert_eq!(cpu.regs.pc, 4);
    }

    #[test]
    fn e03_10_dd_dd_chain() {
        let (cpu, n) = e03_10_run(&[0xDD, 0xDD, 0x21, 0x34, 0x12]);
        assert_eq!(n, 18);
        assert_eq!(cpu.regs.r, 3);
        assert_eq!(cpu.regs.pc, 5);
    }

    #[test]
    fn e03_10_dd_ed() {
        let (_cpu, n) = e03_10_run(&[0xDD, 0xED, 0x44]);
        assert_eq!(n, 12);
    }

    #[test]
    fn e03_10_ddcb_bit_20t() {
        let (cpu, n) = e03_10_run(&[0xDD, 0xCB, 0x05, 0x46]);
        assert_eq!(n, 20);
        assert_eq!(cpu.regs.pc, 4);
        assert_eq!(cpu.eng.opb, 0x46);
    }

    #[test]
    fn e03_10_fd_cb_rlc_23t() {
        let (_cpu, n) = e03_10_run(&[0xFD, 0xCB, 0x02, 0x06]);
        assert_eq!(n, 23);
    }

    #[test]
    fn e03_10_m1_extended() {
        let (_cpu, n) = e03_10_run(&[0x03]);
        assert_eq!(n, 6);
    }

    #[test]
    fn e03_10_refresh_bus() {
        let mut cpu = Cpu::new();
        cpu.regs.i = 0x12;
        cpu.regs.r = 0x34;
        let _t1 = cpu.tick(Pins(0));
        let _t2 = cpu.tick(Pins(0));
        let t3 = cpu.tick(Pins(0));
        // The refresh address carries R before this M1 increments it (E13-10).
        assert_eq!(t3.addr(), 0x1234);
        assert!(t3.has(Pins::RFSH));
        assert!(t3.has(Pins::MREQ));
    }

    #[test]
    fn e03_10_r_bit7_kept() {
        let mut cpu = Cpu::new();
        cpu.regs.r = 0xFF;
        let (cpu, _n) = e03_10_run_cpu(&mut cpu, &[0x00]);
        assert_eq!(cpu.regs.r, 0x80);
    }

    /// Spy bus for the E03-12 tests: 64 KB RAM plus call logs.
    struct E0312Spy {
        ram: crate::bus::SimpleRam,
        reads: std::vec::Vec<u16>,
        writes: std::vec::Vec<(u16, u8)>,
        io_reads: u32,
        ticks: u32,
        fetch_wait: u8,
    }

    impl Bus for E0312Spy {
        fn read(&mut self, addr: u16) -> u8 {
            self.reads.push(addr);
            self.ram.read(addr)
        }
        fn write(&mut self, addr: u16, data: u8) {
            self.writes.push((addr, data));
            self.ram.write(addr, data);
        }
        fn io_read(&mut self, _port: u16) -> u8 {
            self.io_reads += 1;
            0xFF
        }
        fn extra_wait(&mut self, kind: Cycle, _addr: u16) -> u8 {
            if kind == Cycle::Fetch {
                self.fetch_wait
            } else {
                0
            }
        }
        fn on_tick(&mut self, _pins: Pins) {
            self.ticks += 1;
        }
    }

    fn e03_12_spy(prog: &[u8], fetch_wait: u8) -> E0312Spy {
        let mut spy = E0312Spy {
            ram: crate::bus::SimpleRam::new(),
            reads: std::vec::Vec::new(),
            writes: std::vec::Vec::new(),
            io_reads: 0,
            ticks: 0,
            fetch_wait,
        };
        spy.ram.mem[..prog.len()].copy_from_slice(prog);
        spy
    }

    #[test]
    fn e03_12_step_nop() {
        let mut bus = e03_12_spy(&[0x00], 0);
        let mut cpu = Cpu::new();
        assert_eq!(cpu.step(&mut bus), 4);
    }

    #[test]
    fn e03_12_step_ld_bc_nn() {
        let mut bus = e03_12_spy(&[0x01, 0x34, 0x12], 0);
        let mut cpu = Cpu::new();
        assert_eq!(cpu.step(&mut bus), 10);
        assert_eq!(bus.reads.as_slice(), &[0u16, 1, 2]);
    }

    #[test]
    fn e03_12_step_write_logged() {
        let mut bus = e03_12_spy(&[0xDD, 0x36, 0x05, 0x77], 0);
        let mut cpu = Cpu::new();
        cpu.regs.ix = 0x1000;
        let _t = cpu.step(&mut bus);
        assert_eq!(bus.writes.as_slice(), &[(0x1005u16, 0x77u8)]);
    }

    #[test]
    fn e03_12_extra_wait_adds() {
        let mut bus = e03_12_spy(&[0x00], 2);
        let mut cpu = Cpu::new();
        assert_eq!(cpu.step(&mut bus), 6);
    }

    #[test]
    fn e03_12_on_tick_count() {
        let mut bus = e03_12_spy(&[0x00], 0);
        let mut cpu = Cpu::new();
        let n = cpu.step(&mut bus);
        assert_eq!(bus.ticks, 4);
        assert_eq!(bus.ticks, n);
    }

    #[test]
    fn e03_12_io_read_called() {
        let mut bus = e03_12_spy(&[0xDB, 0x10], 0);
        let mut cpu = Cpu::new();
        let _t = cpu.step(&mut bus);
        assert_eq!(bus.io_reads, 1);
    }

    #[test]
    fn e03_12_returns_tstates() {
        let mut bus = e03_12_spy(&[0x01, 0x34, 0x12], 1);
        let mut cpu = Cpu::new();
        assert_eq!(cpu.step(&mut bus), 11);
        assert_eq!(cpu.wait_states(), 1);
    }
}
