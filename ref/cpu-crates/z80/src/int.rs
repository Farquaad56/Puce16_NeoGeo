//! Interrupt, HALT and RESET handling.
//!
//! E09-01: RESET (pin handling lives in `Cpu::tick`, the register state in `Cpu::reset`).

use crate::cpu::Cpu;
use crate::engine::plan::{Addr, Kind, Reg, Tok};
use crate::engine::IrqKind;
use crate::regs::flag;

/// NMI acknowledge: M1 of 5 T (byte ignored, PC not incremented), then push PC high and low.
/// The hook after the last write sets IFF, PC and WZ.
/// INT acknowledge cycle: 7 T, M1 + IORQ, PC not incremented, byte on the bus latched.
const IA7: Tok = Tok::new(Kind::F, 7, Addr::None, Reg::None, false);

/// INT mode 1: acknowledge, push PC, jump to 0x0038 (13 T).
static IM1_PLAN: [Tok; 4] = [
    IA7,
    Tok::new(Kind::W, 3, Addr::DecSp, Reg::H, false),
    Tok::new(Kind::W, 3, Addr::DecSp, Reg::L, false),
    Tok::new(Kind::X, 0, Addr::None, Reg::None, false),
];

/// INT mode 2: acknowledge, push PC, X0 builds the vector, read the word, X1 jumps (19 T).
static IM2_PLAN: [Tok; 7] = [
    IA7,
    Tok::new(Kind::W, 3, Addr::DecSp, Reg::H, false),
    Tok::new(Kind::W, 3, Addr::DecSp, Reg::L, false),
    Tok::new(Kind::X, 0, Addr::None, Reg::None, false),
    Tok::new(Kind::R, 3, Addr::Nn, Reg::L, false),
    Tok::new(Kind::R, 3, Addr::NnPlus1, Reg::H, false),
    Tok::new(Kind::X, 1, Addr::None, Reg::None, false),
];

static NMI_PLAN: [Tok; 4] = [
    Tok::new(Kind::F, 5, Addr::None, Reg::None, false),
    Tok::new(Kind::W, 3, Addr::DecSp, Reg::H, false),
    Tok::new(Kind::W, 3, Addr::DecSp, Reg::L, false),
    Tok::new(Kind::X, 0, Addr::None, Reg::None, false),
];

impl Cpu {
    /// Apply the RESET state: PC = 0, I = R = 0, IFF1 = IFF2 = false, IM = 0, AF = SP = 0xFFFF,
    /// WZ = 0, not halted, and the engine back at an instruction boundary.
    ///
    /// The other registers (BC DE HL IX IY and the shadow set) are left unchanged, as on the NMOS
    /// chip (decision D-028 for Q-001: the real values of the undefined registers after a
    /// power-on RESET are not specified by Zilog, so nothing is invented).
    pub fn reset(&mut self) {
        self.regs.pc = 0;
        self.regs.i = 0;
        self.regs.r = 0;
        self.regs.iff1 = false;
        self.regs.iff2 = false;
        self.regs.im = 0;
        self.regs.set_af(0xFFFF);
        self.regs.sp = 0xFFFF;
        self.regs.wz = 0;
        self.regs.q = 0;
        self.regs.halted = false;

        self.eng.active = false;
        self.eng.cond = false;
        self.eng.ei_delay = false;
        self.eng.ddcb = false;
        self.eng.f_written = false;
        self.eng.irq = IrqKind::None;
        self.eng.int_seq = false;
        self.eng.bus_granted = false;
        self.eng.wz_post = None;
        self.eng.ld_air_now = false;
        self.eng.last_ld_air = false;
        self.eng.nmi_latch = false;
        self.eng.nmi_prev = false;
    }

    /// Begin the NMI sequence (11 T): the return address goes to TMP (PC, already past HALT).
    pub(crate) fn start_nmi(&mut self) {
        self.eng.irq = IrqKind::Nmi;
        let ret = self.regs.pc;
        if self.regs.halted {
            self.regs.halted = false;
            self.eng.halt_cycle = false;
        }
        self.eng.tmp_h = (ret >> 8) as u8;
        self.eng.tmp_l = ret as u8;
        self.eng.plan = &NMI_PLAN;
        self.eng.idx = 0;
        self.eng.int_seq = true;
    }

    /// Hook of an interrupt sequence (called instead of `sem` while one is running).
    pub(crate) fn int_hook(&mut self, hook_no: u8) {
        match (self.eng.irq, self.regs.im, hook_no) {
            (IrqKind::Nmi, _, _) => {
                self.regs.iff2 = self.regs.iff1;
                self.regs.iff1 = false;
                self.regs.pc = 0x0066;
                self.regs.wz = 0x0066;
            }
            (IrqKind::Int, 0, 0) => {
                // IM 0: the byte on the bus is executed. v1 handles RST n only; any other byte
                // is treated as RST 38H (0xFF, the pulled-up bus value).
                // UNKNOWN - to confirm : CALL nn / other instructions on the bus (3 ack cycles).
                let b = self.eng.latched;
                let target = if (b & 0xC7) == 0xC7 {
                    (b & 0x38) as u16
                } else {
                    0x0038
                };
                self.regs.iff1 = false;
                self.regs.iff2 = false;
                self.regs.pc = target;
                self.regs.wz = target;
            }
            (IrqKind::Int, 1, 0) => {
                self.regs.iff1 = false;
                self.regs.iff2 = false;
                self.regs.pc = 0x0038;
                self.regs.wz = 0x0038;
            }
            (IrqKind::Int, 2, 0) => {
                self.regs.iff1 = false;
                self.regs.iff2 = false;
                self.regs.wz = ((self.regs.i as u16) << 8) | (self.eng.latched & 0xFE) as u16;
            }
            (IrqKind::Int, 2, 1) => {
                self.regs.pc = ((self.eng.tmp_h as u16) << 8) | self.eng.tmp_l as u16;
                self.regs.wz = self.regs.pc;
            }
            _ => {}
        }
    }

    /// Begin the INT sequence (filled in by E09-05/06): for now only records the request.
    pub(crate) fn start_int(&mut self) {
        self.eng.irq = IrqKind::Int;
        // NMOS bug: an INT accepted right after LD A,I / LD A,R resets P/V (D-006).
        // The CMOS variant does not have it.
        if self.eng.last_ld_air && self.variant == crate::cpu::Variant::Nmos {
            self.regs.f &= !flag::PV;
        }
        // IM 0 runs the same plan as IM 1; its hook decodes the byte read on the bus.
        let plan: &'static [Tok] = match self.regs.im {
            2 => &IM2_PLAN,
            _ => &IM1_PLAN,
        };
        let ret = self.regs.pc;
        if self.regs.halted {
            self.regs.halted = false;
            self.eng.halt_cycle = false;
        }
        self.eng.tmp_h = (ret >> 8) as u8;
        self.eng.tmp_l = ret as u8;
        self.eng.plan = plan;
        self.eng.idx = 0;
        self.eng.int_seq = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pins::Pins;

    #[test]
    fn e09_01_reset_values() {
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0x1234;
        cpu.regs.i = 0x55;
        cpu.regs.r = 0x66;
        cpu.regs.iff1 = true;
        cpu.regs.iff2 = true;
        cpu.regs.im = 2;
        cpu.regs.set_af(0x1234);
        cpu.regs.sp = 0x8000;
        cpu.regs.wz = 0xABCD;
        cpu.reset();
        assert_eq!(cpu.regs.pc, 0);
        assert_eq!(cpu.regs.i, 0);
        assert_eq!(cpu.regs.r, 0);
        assert!(!cpu.regs.iff1);
        assert!(!cpu.regs.iff2);
        assert_eq!(cpu.regs.im, 0);
        assert_eq!(cpu.regs.af(), 0xFFFF);
        assert_eq!(cpu.regs.sp, 0xFFFF);
        assert_eq!(cpu.regs.wz, 0);
        // Cpu::new() applies the same state.
        let fresh = Cpu::new();
        assert_eq!(fresh.regs.af(), 0xFFFF);
        assert_eq!(fresh.regs.sp, 0xFFFF);
        assert_eq!(fresh.regs.bc(), 0);
    }

    #[test]
    fn e09_01_reset_midstream() {
        let mut cpu = Cpu::new();
        // Start LD HL,nn (opcode 0x21): feed the opcode byte on the data pins.
        let pins = Pins(0).set_data(0x21);
        for _ in 0..5 {
            cpu.tick(pins);
        }
        assert!(!cpu.at_instruction_boundary());
        cpu.eng.cond = true;
        cpu.eng.ei_delay = true;
        cpu.reset();
        assert!(cpu.at_instruction_boundary());
        assert!(!cpu.eng.cond);
        assert!(!cpu.eng.ei_delay);
        assert_eq!(cpu.regs.pc, 0);
    }

    #[test]
    fn e09_01_reset_pin_holds() {
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0x1234;
        let ctrl = Pins::M1 | Pins::MREQ | Pins::IORQ | Pins::RD | Pins::WR | Pins::RFSH;
        for _ in 0..5 {
            let out = cpu.tick(Pins(0).with(Pins::RESET));
            assert_eq!(out.0 & ctrl, 0, "no control output while RESET is held");
            assert_eq!(cpu.regs.pc, 0);
            assert!(cpu.at_instruction_boundary());
        }
        // After release, the next instruction starts normally: the first M1 reads PC = 0.
        cpu.tick(Pins(0));
        assert!(!cpu.at_instruction_boundary());
        assert_eq!(cpu.regs.pc, 1);
    }

    #[test]
    fn e09_01_reset_pin_resets_once() {
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0x4000;
        cpu.tick(Pins(0).with(Pins::RESET));
        assert_eq!(cpu.regs.pc, 0);
        // Registers touched while the pin stays active are not reset again.
        cpu.regs.pc = 0x0010;
        cpu.tick(Pins(0).with(Pins::RESET));
        assert_eq!(cpu.regs.pc, 0x0010);
        // A single-tick pulse (shorter than 3 T) still resets.
        cpu.tick(Pins(0));
        cpu.regs.pc = 0x2000;
        cpu.tick(Pins(0).with(Pins::RESET));
        assert_eq!(cpu.regs.pc, 0);
    }

    #[test]
    fn e09_01_reset_clears_halt() {
        let mut cpu = Cpu::new();
        cpu.regs.halted = true;
        cpu.reset();
        assert!(!cpu.regs.halted);
    }

    #[test]
    fn e09_01_others_unchanged() {
        let mut cpu = Cpu::new();
        cpu.regs.set_bc(0x1111);
        cpu.regs.set_de(0x2222);
        cpu.regs.set_hl(0x3333);
        cpu.regs.ix = 0x4444;
        cpu.regs.iy = 0x5555;
        cpu.regs.set_bc2(0x6666);
        cpu.reset();
        assert_eq!(cpu.regs.bc(), 0x1111);
        assert_eq!(cpu.regs.de(), 0x2222);
        assert_eq!(cpu.regs.hl(), 0x3333);
        assert_eq!(cpu.regs.ix, 0x4444);
        assert_eq!(cpu.regs.iy, 0x5555);
        assert_eq!(cpu.regs.bc2(), 0x6666);
    }

    use crate::bus::{Bus, SimpleRam};

    /// CPU that has just executed HALT at address 0 (halted, PC just past the HALT opcode).
    fn halted_cpu() -> (Cpu, SimpleRam) {
        let mut ram = SimpleRam::new();
        ram.write(0, 0x76);
        let mut cpu = Cpu::new();
        assert_eq!(cpu.step(&mut ram), 4);
        assert!(cpu.regs.halted);
        (cpu, ram)
    }

    #[test]
    fn e09_02_halt_4t() {
        let (mut cpu, mut ram) = halted_cpu();
        for _ in 0..3 {
            assert_eq!(cpu.step(&mut ram), 4);
        }
        assert!(cpu.regs.halted);
    }

    #[test]
    fn e09_02_halt_pin() {
        let (mut cpu, _ram) = halted_cpu();
        for t in 0..8 {
            let out = cpu.tick(Pins(0));
            assert!(out.has(Pins::HALT), "HALT pin active at T{t}");
        }
    }

    #[test]
    fn e09_02_halt_pc_stable() {
        let (mut cpu, mut ram) = halted_cpu();
        let pc = cpu.regs.pc;
        for _ in 0..3 {
            cpu.step(&mut ram);
            assert_eq!(cpu.regs.pc, pc);
        }
    }

    #[test]
    fn e09_02_halt_r_increments() {
        let (mut cpu, mut ram) = halted_cpu();
        for _ in 0..3 {
            let r = cpu.regs.r;
            cpu.step(&mut ram);
            assert_eq!(cpu.regs.r & 0x7F, (r.wrapping_add(1)) & 0x7F);
        }
    }

    #[test]
    fn e09_02_halt_no_bus_write() {
        let (mut cpu, _ram) = halted_cpu();
        for _ in 0..12 {
            let out = cpu.tick(Pins(0));
            assert!(!out.has(Pins::WR));
            assert!(!out.has(Pins::IORQ));
        }
    }

    /// Run one 4 T instruction made of `byte` (an M1 cycle) with `extra` pins held.
    fn run_m1(cpu: &mut Cpu, byte: u8, extra: u64) {
        for _ in 0..4 {
            cpu.tick(Pins(0).set_data(byte).with(extra));
        }
    }

    #[test]
    fn e09_03_nmi_edge_latched() {
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0x100;
        cpu.tick(Pins(0));
        // NMI active for 3 ticks in a row inside the first NOP: one request only.
        for _ in 0..3 {
            cpu.tick(Pins(0).with(Pins::NMI));
        }
        assert!(cpu.eng.nmi_latch);
        assert!(cpu.at_instruction_boundary());
        // The next instruction start (NMI released) consumes the latch once.
        cpu.tick(Pins(0));
        assert_eq!(cpu.eng.irq, IrqKind::Nmi);
        assert!(!cpu.eng.nmi_latch);
        // The request was consumed: after the 11 T sequence no second NMI starts.
        for _ in 0..10 {
            cpu.tick(Pins(0));
        }
        assert!(cpu.at_instruction_boundary());
        cpu.tick(Pins(0));
        assert_eq!(cpu.eng.irq, IrqKind::None);
    }

    #[test]
    fn e09_03_nmi_second_edge() {
        let mut cpu = Cpu::new();
        cpu.tick(Pins(0));
        cpu.tick(Pins(0).with(Pins::NMI));
        assert!(cpu.eng.nmi_latch);
        cpu.eng.nmi_latch = false;
        // Still high: no new request.
        cpu.tick(Pins(0).with(Pins::NMI));
        assert!(!cpu.eng.nmi_latch);
        // Falls then rises again: new request.
        cpu.tick(Pins(0));
        cpu.tick(Pins(0).with(Pins::NMI));
        // This tick starts a new instruction, which consumes the new request.
        assert_eq!(cpu.eng.irq, IrqKind::Nmi);
    }

    #[test]
    fn e09_03_int_level_ignored_when_di() {
        let mut cpu = Cpu::new();
        cpu.regs.iff1 = false;
        for _ in 0..3 {
            run_m1(&mut cpu, 0, Pins::INT);
            assert_eq!(cpu.eng.irq, IrqKind::None);
        }
        cpu.regs.iff1 = true;
        run_m1(&mut cpu, 0, Pins::INT);
        cpu.tick(Pins(0).with(Pins::INT));
        assert_eq!(cpu.eng.irq, IrqKind::Int);
    }

    #[test]
    fn e09_03_ei_delay_one_instruction() {
        let mut cpu = Cpu::new();
        // EI with INT active: the boundary after EI does not accept.
        run_m1(&mut cpu, 0xFB, Pins::INT);
        assert!(cpu.regs.iff1);
        // NOP starts: boundary test sees ei_delay, INT ignored.
        run_m1(&mut cpu, 0, Pins::INT);
        assert_eq!(cpu.eng.irq, IrqKind::None);
        // Next boundary (after the NOP): accepted.
        cpu.tick(Pins(0).with(Pins::INT));
        assert_eq!(cpu.eng.irq, IrqKind::Int);
    }

    #[test]
    fn e09_03_ei_ei() {
        let mut cpu = Cpu::new();
        run_m1(&mut cpu, 0xFB, Pins::INT);
        run_m1(&mut cpu, 0xFB, Pins::INT);
        run_m1(&mut cpu, 0, Pins::INT);
        // Still inhibited at every boundary so far (EI EI), including the one after the NOP.
        assert_eq!(cpu.eng.irq, IrqKind::None);
        cpu.tick(Pins(0).with(Pins::INT));
        assert_eq!(cpu.eng.irq, IrqKind::Int);
    }

    #[test]
    fn e09_03_no_int_mid_prefix() {
        let mut cpu = Cpu::new();
        cpu.regs.iff1 = true;
        // DD prefix fetch: the instruction is still active after its 4 T.
        cpu.tick(Pins(0).set_data(0xDD));
        for _ in 0..3 {
            cpu.tick(Pins(0).set_data(0xDD).with(Pins::INT | Pins::NMI));
        }
        assert!(!cpu.at_instruction_boundary());
        assert_eq!(cpu.eng.irq, IrqKind::None);
        // Opcode NOP under DD (4 T more): accepted only after it completes.
        for _ in 0..4 {
            cpu.tick(Pins(0).with(Pins::INT));
        }
        assert!(cpu.at_instruction_boundary());
        assert_eq!(cpu.eng.irq, IrqKind::None);
        cpu.tick(Pins(0).with(Pins::INT));
        assert_ne!(cpu.eng.irq, IrqKind::None);
    }

    /// Tick `n` T-states serving `ram` by hand, with `extra` input pins held.
    fn run_ticks(cpu: &mut Cpu, ram: &mut SimpleRam, n: u32, extra: u64) {
        let mut data = 0u8;
        for _ in 0..n {
            let out = cpu.tick(Pins(0).set_data(data).with(extra));
            if out.is_mem_read() {
                data = ram.read(out.addr());
            }
            if out.is_mem_write() {
                ram.write(out.addr(), out.data());
            }
        }
    }

    /// CPU at address 0x1234 with SP = 0x8000 and an NMI request pending.
    fn nmi_cpu() -> (Cpu, SimpleRam) {
        let ram = SimpleRam::new();
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0x1234;
        cpu.regs.sp = 0x8000;
        cpu.eng.nmi_latch = true;
        (cpu, ram)
    }

    #[test]
    fn e09_04_nmi_11t() {
        let (mut cpu, mut ram) = nmi_cpu();
        assert_eq!(cpu.step(&mut ram), 11);
        assert_eq!(cpu.instruction_tstates(), 11);
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e09_04_nmi_pushes_pc() {
        let (mut cpu, mut ram) = nmi_cpu();
        cpu.step(&mut ram);
        assert_eq!(ram.read(0x7FFF), 0x12);
        assert_eq!(ram.read(0x7FFE), 0x34);
        assert_eq!(cpu.regs.sp, 0x7FFE);
    }

    #[test]
    fn e09_04_nmi_vector() {
        let (mut cpu, mut ram) = nmi_cpu();
        cpu.step(&mut ram);
        assert_eq!(cpu.regs.pc, 0x0066);
        assert_eq!(cpu.regs.wz, 0x0066);
    }

    #[test]
    fn e09_04_nmi_iff() {
        let (mut cpu, mut ram) = nmi_cpu();
        cpu.regs.iff1 = true;
        cpu.regs.iff2 = false;
        cpu.step(&mut ram);
        assert!(!cpu.regs.iff1);
        assert!(cpu.regs.iff2);
        let (mut cpu, mut ram) = nmi_cpu();
        cpu.regs.iff1 = false;
        cpu.regs.iff2 = true;
        cpu.step(&mut ram);
        assert!(!cpu.regs.iff1);
        assert!(!cpu.regs.iff2);
    }

    #[test]
    fn e09_04_nmi_wakes_halt() {
        let (mut cpu, mut ram) = halted_cpu();
        cpu.regs.sp = 0x8000;
        cpu.eng.nmi_latch = true;
        assert_eq!(cpu.step(&mut ram), 11);
        assert!(!cpu.regs.halted);
        assert_eq!(ram.read(0x7FFF), 0x00);
        assert_eq!(ram.read(0x7FFE), 0x01);
        assert_eq!(cpu.regs.pc, 0x0066);
    }

    #[test]
    fn e09_04_nmi_priority() {
        let (mut cpu, mut ram) = nmi_cpu();
        cpu.regs.iff1 = true;
        // INT held active the whole time, together with the NMI request.
        run_ticks(&mut cpu, &mut ram, 11, Pins::INT);
        assert_eq!(cpu.regs.pc, 0x0066);
        assert_eq!(ram.read(0x7FFE), 0x34);
        assert!(!cpu.regs.iff1, "NMI went first and masked INT");
        assert!(cpu.at_instruction_boundary());
    }

    #[test]
    fn e09_04_retn_restores() {
        let (mut cpu, mut ram) = nmi_cpu();
        ram.write(0x0066, 0xED);
        ram.write(0x0067, 0x45);
        cpu.regs.iff1 = true;
        cpu.step(&mut ram);
        assert!(!cpu.regs.iff1);
        assert_eq!(cpu.step(&mut ram), 14);
        assert!(cpu.regs.iff1);
        assert_eq!(cpu.regs.pc, 0x1234);
    }

    /// RAM with a controllable INT line and acknowledge byte; records the acknowledge pins.
    struct IntRam {
        ram: SimpleRam,
        irq: bool,
        ack: u8,
        ack_ticks: u32,
        bad_ack: bool,
    }

    impl IntRam {
        fn new() -> IntRam {
            IntRam {
                ram: SimpleRam::new(),
                irq: false,
                ack: 0xFF,
                ack_ticks: 0,
                bad_ack: false,
            }
        }
    }

    impl Bus for IntRam {
        fn read(&mut self, addr: u16) -> u8 {
            self.ram.read(addr)
        }
        fn write(&mut self, addr: u16, data: u8) {
            self.ram.write(addr, data);
        }
        fn irq(&mut self) -> bool {
            self.irq
        }
        fn int_ack(&mut self) -> u8 {
            self.ack
        }
        fn on_tick(&mut self, pins: Pins) {
            if pins.is_int_ack() {
                self.ack_ticks += 1;
                if pins.has(Pins::MREQ) || pins.has(Pins::RD) {
                    self.bad_ack = true;
                }
            }
        }
    }

    /// CPU with interrupts enabled in `im`, SP = 0x8000, after one NOP at 0x0100; INT is raised.
    fn int_cpu(im: u8) -> (Cpu, IntRam) {
        let mut bus = IntRam::new();
        let mut cpu = Cpu::new();
        cpu.regs.pc = 0x0100;
        cpu.regs.sp = 0x8000;
        cpu.regs.iff1 = true;
        cpu.regs.iff2 = true;
        cpu.regs.im = im;
        assert_eq!(cpu.step(&mut bus), 4);
        bus.irq = true;
        (cpu, bus)
    }

    #[test]
    fn e09_05_im1_13t() {
        let (mut cpu, mut bus) = int_cpu(1);
        assert_eq!(cpu.step(&mut bus), 13);
        assert_eq!(cpu.regs.pc, 0x0038);
        assert_eq!(cpu.regs.wz, 0x0038);
        assert_eq!(bus.ram.read(0x7FFF), 0x01);
        assert_eq!(bus.ram.read(0x7FFE), 0x01);
        assert_eq!(cpu.regs.sp, 0x7FFE);
    }

    #[test]
    fn e09_05_im2_19t() {
        let (mut cpu, mut bus) = int_cpu(2);
        cpu.regs.i = 0x40;
        bus.ack = 0x20;
        bus.ram.write(0x4020, 0x78);
        bus.ram.write(0x4021, 0x56);
        assert_eq!(cpu.step(&mut bus), 19);
        assert_eq!(cpu.regs.pc, 0x5678);
        assert_eq!(cpu.regs.wz, 0x5678);
        assert_eq!(cpu.regs.sp, 0x7FFE);
    }

    #[test]
    fn e09_05_im2_vector_bit0() {
        let (mut cpu, mut bus) = int_cpu(2);
        cpu.regs.i = 0x40;
        bus.ack = 0x21;
        bus.ram.write(0x4020, 0x78);
        bus.ram.write(0x4021, 0x56);
        bus.ram.write(0x4022, 0xEE);
        assert_eq!(cpu.step(&mut bus), 19);
        assert_eq!(cpu.regs.pc, 0x5678, "bit 0 of the bus byte is masked");
    }

    #[test]
    fn e09_05_ack_pins() {
        let (mut cpu, mut bus) = int_cpu(1);
        cpu.step(&mut bus);
        assert!(bus.ack_ticks >= 2, "acknowledge cycle seen");
        assert!(!bus.bad_ack, "no MREQ or RD during the acknowledge");
    }

    #[test]
    fn e09_05_iff_cleared() {
        for im in 1..=2u8 {
            let (mut cpu, mut bus) = int_cpu(im);
            cpu.step(&mut bus);
            assert!(!cpu.regs.iff1, "IM{im} IFF1");
            assert!(!cpu.regs.iff2, "IM{im} IFF2");
        }
    }

    #[test]
    fn e09_05_im1_wakes_halt() {
        let mut bus = IntRam::new();
        bus.ram.write(0, 0x76);
        let mut cpu = Cpu::new();
        cpu.regs.sp = 0x8000;
        cpu.regs.iff1 = true;
        cpu.regs.im = 1;
        assert_eq!(cpu.step(&mut bus), 4);
        assert!(cpu.regs.halted);
        bus.irq = true;
        assert_eq!(cpu.step(&mut bus), 13);
        assert!(!cpu.regs.halted);
        assert_eq!(bus.ram.read(0x7FFF), 0x00);
        assert_eq!(bus.ram.read(0x7FFE), 0x01);
        assert_eq!(cpu.regs.pc, 0x0038);
    }

    #[test]
    fn e09_05_masked_by_di() {
        let (mut cpu, mut bus) = int_cpu(1);
        cpu.regs.iff1 = false;
        assert_eq!(cpu.step(&mut bus), 4);
        assert_eq!(cpu.step(&mut bus), 4);
        assert_eq!(bus.ack_ticks, 0);
        assert_eq!(cpu.regs.pc, 0x0103);
    }

    #[test]
    fn e09_06_im0_rst38() {
        let (mut cpu, mut bus) = int_cpu(0);
        bus.ack = 0xFF;
        assert_eq!(cpu.step(&mut bus), 13);
        assert_eq!(cpu.regs.pc, 0x0038);
        assert_eq!(cpu.regs.wz, 0x0038);
        assert_eq!(bus.ram.read(0x7FFE), 0x01);
    }

    #[test]
    fn e09_06_im0_rst10() {
        let (mut cpu, mut bus) = int_cpu(0);
        bus.ack = 0xD7;
        assert_eq!(cpu.step(&mut bus), 13);
        assert_eq!(cpu.regs.pc, 0x0010);
    }

    #[test]
    fn e09_06_im0_other_falls_back() {
        let (mut cpu, mut bus) = int_cpu(0);
        bus.ack = 0x00;
        assert_eq!(cpu.step(&mut bus), 13);
        assert_eq!(cpu.regs.pc, 0x0038);
    }

    #[test]
    fn e09_06_im0_iff() {
        let (mut cpu, mut bus) = int_cpu(0);
        cpu.step(&mut bus);
        assert!(!cpu.regs.iff1);
        assert!(!cpu.regs.iff2);
    }

    /// One T-state with the memory served by hand (`data` carries the byte read last).
    fn tick_ram(cpu: &mut Cpu, ram: &mut SimpleRam, data: &mut u8, extra: u64) -> Pins {
        let out = cpu.tick(Pins(0).set_data(*data).with(extra));
        if out.is_mem_read() {
            *data = ram.read(out.addr());
        }
        if out.is_mem_write() {
            ram.write(out.addr(), out.data());
        }
        out
    }

    /// RAM holding `LD A,(0x9000)` at address 0 and 0x42 at 0x9000.
    fn ld_a_nn_ram() -> SimpleRam {
        let mut ram = SimpleRam::new();
        ram.write(0, 0x3A);
        ram.write(1, 0x00);
        ram.write(2, 0x90);
        ram.write(0x9000, 0x42);
        ram
    }

    #[test]
    fn e09_07_busrq_after_cycle() {
        let mut ram = ld_a_nn_ram();
        let mut cpu = Cpu::new();
        let mut data = 0;
        // BUSRQ rises during the first M1: that cycle (4 T) still completes normally.
        for t in 0..4 {
            let extra = if t >= 1 { Pins::BUSRQ } else { 0 };
            let out = tick_ram(&mut cpu, &mut ram, &mut data, extra);
            assert!(!out.has(Pins::BUSAK), "T{t} is still part of the cycle");
        }
        let out = tick_ram(&mut cpu, &mut ram, &mut data, Pins::BUSRQ);
        assert!(out.has(Pins::BUSAK));
    }

    #[test]
    fn e09_07_busak_pins() {
        let mut ram = ld_a_nn_ram();
        let mut cpu = Cpu::new();
        let mut data = 0;
        for _ in 0..4 {
            tick_ram(&mut cpu, &mut ram, &mut data, Pins::BUSRQ);
        }
        for _ in 0..5 {
            let out = tick_ram(&mut cpu, &mut ram, &mut data, Pins::BUSRQ);
            assert_eq!(out.0, Pins::BUSAK, "only BUSAK, A = 0, D = 0");
        }
    }

    #[test]
    fn e09_07_resume() {
        let mut ram = ld_a_nn_ram();
        let mut cpu = Cpu::new();
        let mut data = 0;
        let mut n = 0;
        while n < 100 {
            // BUSRQ held from the 3rd to the 12th tick.
            let extra = if (2..12).contains(&n) { Pins::BUSRQ } else { 0 };
            tick_ram(&mut cpu, &mut ram, &mut data, extra);
            n += 1;
            if cpu.at_instruction_boundary() {
                break;
            }
        }
        assert_eq!(cpu.regs.a, 0x42);
        assert_eq!(cpu.regs.pc, 3);
        assert_eq!(cpu.instruction_tstates(), 13);
        assert!(n > 13, "the granted T-states were added");
    }

    #[test]
    fn e09_07_no_progress() {
        let mut ram = ld_a_nn_ram();
        let mut cpu = Cpu::new();
        let mut data = 0;
        for _ in 0..4 {
            tick_ram(&mut cpu, &mut ram, &mut data, Pins::BUSRQ);
        }
        let (pc, r, ts) = (cpu.regs.pc, cpu.regs.r, cpu.instruction_tstates());
        for _ in 0..8 {
            tick_ram(&mut cpu, &mut ram, &mut data, Pins::BUSRQ);
            assert_eq!(cpu.regs.pc, pc);
            assert_eq!(cpu.regs.r, r);
            assert_eq!(cpu.instruction_tstates(), ts);
        }
    }

    #[test]
    fn e09_07_busak_clears() {
        let mut ram = ld_a_nn_ram();
        let mut cpu = Cpu::new();
        let mut data = 0;
        for _ in 0..6 {
            tick_ram(&mut cpu, &mut ram, &mut data, Pins::BUSRQ);
        }
        let out = tick_ram(&mut cpu, &mut ram, &mut data, 0);
        assert!(!out.has(Pins::BUSAK));
        assert!(
            out.has(Pins::MREQ) || out.addr() != 0,
            "the next cycle resumes"
        );
    }

    /// RAM whose BUSRQ line is high for the first `left` samples.
    struct BusrqRam {
        ram: SimpleRam,
        left: u32,
    }

    impl Bus for BusrqRam {
        fn read(&mut self, addr: u16) -> u8 {
            self.ram.read(addr)
        }
        fn write(&mut self, addr: u16, data: u8) {
            self.ram.write(addr, data);
        }
        fn busrq(&mut self) -> bool {
            if self.left > 0 {
                self.left -= 1;
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn e09_07_step_with_bus() {
        let mut bus = BusrqRam {
            ram: ld_a_nn_ram(),
            left: 10,
        };
        let mut cpu = Cpu::new();
        let n = cpu.step(&mut bus);
        // 13 T of instruction + 6 granted T-states (T5..T10) while BUSRQ is high.
        assert_eq!(n, 19);
        assert_eq!(cpu.instruction_tstates(), 13);
        assert_eq!(cpu.regs.a, 0x42);
    }
}
