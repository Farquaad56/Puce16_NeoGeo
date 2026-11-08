// T005 — Test de fumée des crates CPU (puce-m68k, puce-z80)
// Fixe le contrat d'utilisation : ne JAMAIS modifier ces crates.

use puce_m68k::bus::Bus;
use puce_m68k::cpu::Cpu as M68kCpu;
use puce_m68k::pins::Pins;
use puce_z80::{Bus as Z80Bus, Cpu as Z80Cpu};

// --- Machine 68000 : RAM 64 Kio miroir ---
struct M68kMachine {
    ram: [u8; 65536],
    ticks: u32,
}

impl M68kMachine {
    fn new() -> Self {
        let mut m = Self {
            ram: [0; 65536],
            ticks: 0,
        };
        // Vecteurs : SSP=0x1000 (octets 00 00 10 00), PC=0x100 (00 00 01 00)
        m.ram[0] = 0;
        m.ram[1] = 0;
        m.ram[2] = 0x10;
        m.ram[3] = 0;
        m.ram[4] = 0;
        m.ram[5] = 0;
        m.ram[6] = 0x01;
        m.ram[7] = 0;
        // Programme à 0x100 : MOVE.W #$1234,D0 ; STOP #$2700
        m.ram[0x100] = 0x30;
        m.ram[0x101] = 0x3C;
        m.ram[0x102] = 0x12;
        m.ram[0x103] = 0x34;
        m.ram[0x104] = 0x4E;
        m.ram[0x105] = 0x72;
        m.ram[0x106] = 0x27;
        m.ram[0x107] = 0x00;
        m
    }
}

impl Bus for M68kMachine {
    fn read(&mut self, addr: u32, _fc: u8, _uds: bool, _lds: bool) -> u16 {
        let a = (addr as usize) & 0xFFFE;
        u16::from_be_bytes([self.ram[a], self.ram[a + 1]])
    }

    fn write(&mut self, addr: u32, _fc: u8, uds: bool, lds: bool, data: u16) {
        let a = (addr as usize) & 0xFFFE;
        if uds {
            self.ram[a] = (data >> 8) as u8;
        }
        if lds {
            self.ram[a + 1] = data as u8;
        }
    }

    fn on_tick(&mut self, _pins: Pins) {
        self.ticks += 1;
    }
}

#[test]
fn test_m68k_reset_move_stop() {
    let mut machine = M68kMachine::new();
    let mut cpu = M68kCpu::new();
    cpu.reset(); // Démarre la séquence de reset (40 horloges)

    // Boucle step_clock : première frontière d'instruction après 40 ticks
    loop {
        if cpu.at_instruction_boundary() && machine.ticks >= 40 {
            break;
        }
        cpu.step_clock(&mut machine);
    }
    assert_eq!(machine.ticks, 40, "Reset doit se terminer à 40 ticks");
    let state = cpu.state();
    assert_eq!(
        state.pc, 0x104,
        "PC après reset doit être 0x104 (après fetch du premier opcode)"
    );
    assert_eq!(state.ssp, 0x1000, "SSP doit être 0x1000");

    // Exécution de MOVE.W #$1234,D0 : termine à 48 ticks
    loop {
        if cpu.at_instruction_boundary() && machine.ticks >= 48 {
            break;
        }
        cpu.step_clock(&mut machine);
    }
    assert_eq!(machine.ticks, 48, "MOVE.W doit se terminer à 48 ticks");
    let state = cpu.state();
    assert_eq!(state.d[0], 0x1234, "D0 doit contenir 0x1234 après MOVE.W");

    // STOP #$2700 : is_stopped() devient vrai après exécution de l'instruction
    loop {
        if cpu.is_stopped() && cpu.at_instruction_boundary() {
            break;
        }
        cpu.step_clock(&mut machine);
    }
    assert!(cpu.is_stopped(), "CPU doit être arrêté après STOP");

    // En mode stop, step() retourne 4 ticks par appel (cycle de bus vide)
    let ticks_before = machine.ticks;
    let elapsed = cpu.step(&mut machine);
    assert_eq!(elapsed, 4, "En mode stop : step() doit retourner 4 ticks");
    assert_eq!(
        machine.ticks - ticks_before,
        4,
        "En mode stop : 4 ticks écoulés"
    );
}

// --- Machine 68000 avec attente ---
struct M68kWaitMachine {
    ram: [u8; 65536],
    ticks: u32,
    wait_addr: u32,
    wait_cycles: u8,
}

impl M68kWaitMachine {
    fn new() -> Self {
        let mut m = Self {
            ram: [0; 65536],
            ticks: 0,
            wait_addr: 0x200,
            wait_cycles: 0,
        };
        // Vecteurs SSP=0x1000, PC=0x100
        m.ram[0] = 0;
        m.ram[1] = 0;
        m.ram[2] = 0x10;
        m.ram[3] = 0;
        m.ram[4] = 0;
        m.ram[5] = 0;
        m.ram[6] = 0x01;
        m.ram[7] = 0;
        // Programme : MOVE.W $200,D1 (32 39 00 00 02 00) ; STOP #$2700
        m.ram[0x100] = 0x32;
        m.ram[0x101] = 0x39;
        m.ram[0x102] = 0x00;
        m.ram[0x103] = 0x00;
        m.ram[0x104] = 0x02;
        m.ram[0x105] = 0x00;
        m.ram[0x106] = 0x4E;
        m.ram[0x107] = 0x72;
        m.ram[0x108] = 0x27;
        m.ram[0x109] = 0x00;
        // Valeur à lire en 0x200
        m.ram[0x200] = 0xAB;
        m.ram[0x201] = 0xCD;
        m
    }

    fn set_wait(&mut self, cycles: u8) {
        self.wait_cycles = cycles;
    }
}

impl Bus for M68kWaitMachine {
    fn read(&mut self, addr: u32, _fc: u8, _uds: bool, _lds: bool) -> u16 {
        let a = (addr as usize) & 0xFFFE;
        u16::from_be_bytes([self.ram[a], self.ram[a + 1]])
    }

    fn write(&mut self, addr: u32, _fc: u8, uds: bool, lds: bool, data: u16) {
        let a = (addr as usize) & 0xFFFE;
        if uds {
            self.ram[a] = (data >> 8) as u8;
        }
        if lds {
            self.ram[a + 1] = data as u8;
        }
    }

    fn wait_clocks(&mut self, addr: u32, _write: bool) -> u8 {
        if addr == self.wait_addr {
            return self.wait_cycles;
        }
        0
    }

    fn on_tick(&mut self, _pins: Pins) {
        self.ticks += 1;
    }
}

#[test]
fn test_m68k_wait_states() {
    // Sans attente : MOVE.W $200,D1 dure 16 ticks (4 cycles de bus × 4 horloges)
    let mut machine = M68kWaitMachine::new();
    let mut cpu = M68kCpu::new();
    cpu.reset();

    // Attendre la fin du reset + fetch du premier opcode
    loop {
        if cpu.at_instruction_boundary() && machine.ticks >= 40 {
            break;
        }
        cpu.step_clock(&mut machine);
    }
    let ticks_before = machine.ticks;

    // Exécuter MOVE.W $200,D1 (sans attente)
    loop {
        if cpu.at_instruction_boundary() && machine.ticks > ticks_before + 3 {
            break;
        }
        cpu.step_clock(&mut machine);
    }
    let elapsed_no_wait = machine.ticks - ticks_before;
    assert_eq!(
        elapsed_no_wait, 16,
        "MOVE.W sans attente doit durer 16 ticks"
    );

    // Avec attente de 2 horloges : 18 ticks
    let mut machine2 = M68kWaitMachine::new();
    machine2.set_wait(2);
    let mut cpu2 = M68kCpu::new();
    cpu2.reset();

    loop {
        if cpu2.at_instruction_boundary() && machine2.ticks >= 40 {
            break;
        }
        cpu2.step_clock(&mut machine2);
    }
    let ticks_before2 = machine2.ticks;

    loop {
        if cpu2.at_instruction_boundary() && machine2.ticks > ticks_before2 + 3 {
            break;
        }
        cpu2.step_clock(&mut machine2);
    }
    let elapsed_with_wait = machine2.ticks - ticks_before2;
    assert_eq!(
        elapsed_with_wait, 18,
        "MOVE.W avec attente de 2 horloges doit durer 18 ticks"
    );
}

// --- Machine Z80 : RAM simple ---
struct Z80Machine {
    ram: [u8; 65536],
}

impl Z80Machine {
    fn new() -> Self {
        let mut m = Self { ram: [0; 65536] };
        // Programme : LD A,1 ; ADD A,A ; HALT (3E 01 87 76)
        m.ram[0] = 0x3E;
        m.ram[1] = 0x01;
        m.ram[2] = 0x87;
        m.ram[3] = 0x76;
        m
    }
}

impl Z80Bus for Z80Machine {
    fn read(&mut self, addr: u16) -> u8 {
        self.ram[addr as usize]
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.ram[addr as usize] = data;
    }
}

#[test]
fn test_z80_step_api() {
    let mut machine = Z80Machine::new();
    let mut cpu = Z80Cpu::new();
    cpu.reset();

    // 3 instructions : LD A,1 (4 T-states) ; ADD A,A (4 T-states) ; HALT (4 T-states)
    // Total : 12 T-states pour les 3 step() + HALT ajoute des cycles d'attente
    let mut total_tstates = 0;
    for _ in 0..3 {
        total_tstates += cpu.step(&mut machine);
    }

    assert_eq!(cpu.regs.a, 2, "A doit être 2 après LD A,1 ; ADD A,A");
    assert_eq!(
        total_tstates, 15,
        "Total doit être 15 T-states (4+4+7 pour HALT)"
    );
}
