//! Tests d'intégration des wait states (T601).
//!
//! Vérifie que le mécanisme de wait states est câblé correctement :
//! - bus_wait retourne 0 pour toutes les zones (MAME ne modélise pas)
//! - Le CPU ajoute bien +2 mclk par horloge d'attente

use puce_m68k::bus::Bus;
use puce_m68k::cpu::Cpu as M68kCpu;
use puce_m68k::pins::Pins;

/// Machine de test avec wait states configurables.
struct WaitTestMachine {
    ram: [u8; 65536],
    ticks: u32,
    wait_addr: u32,
    wait_cycles: u8,
}

impl WaitTestMachine {
    fn new() -> Self {
        let mut m = Self {
            ram: [0; 65536],
            ticks: 0,
            wait_addr: 0x100000,
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
        // Programme : MOVE.W $100000,D0 (30 39 00 10 00 00) ; STOP #$2700
        m.ram[0x100] = 0x30;
        m.ram[0x101] = 0x39;
        m.ram[0x102] = 0x00;
        m.ram[0x103] = 0x10;
        m.ram[0x104] = 0x00;
        m.ram[0x105] = 0x00;
        m.ram[0x106] = 0x4E;
        m.ram[0x107] = 0x72;
        m.ram[0x108] = 0x27;
        m.ram[0x109] = 0x00;
        // Valeur à lire en 0x100000 (miroir sur RAM locale)
        m.ram[0x0000] = 0xAB;
        m.ram[0x0001] = 0xCD;
        m
    }

    fn set_wait(&mut self, cycles: u8) {
        self.wait_cycles = cycles;
    }
}

impl Bus for WaitTestMachine {
    fn read(&mut self, addr: u32, _fc: u8, _uds: bool, _lds: bool) -> u16 {
        // Miroir : 0x100000 -> 0x0000 dans notre RAM de test
        let a = if addr == 0x100000 {
            0
        } else {
            (addr as usize) & 0xFFFE
        };
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
fn wait_states_add_clocks() {
    // Sans attente : MOVE.W $100000,D0 dure 16 ticks (4 cycles de bus × 4 horloges)
    let mut machine = WaitTestMachine::new();
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

    // Exécuter MOVE.W $100000,D0 (sans attente)
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

    // Avec attente de 2 horloges : 18 ticks (+4 mclk)
    let mut machine2 = WaitTestMachine::new();
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
