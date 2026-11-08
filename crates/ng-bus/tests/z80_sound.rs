//! Tests for Z80 SoundBoard: memory map, banking, ports, latches, pin server.

use ng_bus::{SoundBoard, Ym};
use puce_z80::Cpu;

/// Fake YM2610 that records calls and returns predictable values.
#[derive(Default)]
struct FakeYm {
    pub advance_calls: Vec<u64>,
    pub read_port: u8,
    pub write_port: Option<(u8, u8)>,
    pub irq_state: bool,
}

impl Ym for FakeYm {
    fn advance(&mut self, mclk: u64) {
        self.advance_calls.push(mclk);
    }
    fn read(&mut self, port: u8) -> u8 {
        self.read_port = port;
        0xAA
    }
    fn write(&mut self, port: u8, v: u8) {
        self.write_port = Some((port, v));
    }
    fn irq(&self) -> bool {
        self.irq_state
    }
}

fn make_board() -> SoundBoard<FakeYm> {
    let mut board = SoundBoard::new(FakeYm::default());
    // Fill M1 ROM with pattern for banking tests (0x30000 bytes)
    board.m1 = vec![0; 0x30000];
    for i in 0..board.m1.len() {
        board.m1[i] = (i & 0xFF) as u8;
    }
    // Fill SM1 with different pattern
    board.sm1 = vec![0xCC; 0x8000];
    board
}

// === Banking offset calculation tests ===

#[test]
fn test_banking_offset_f000_window() {
    use ng_bus::z80map::banked_offset;
    // m1_len=0x30000, bank[0]=0x1E (window $F000, i=0)
    // mask = (0x30000 - 0x10000 - 1) & 0x3FFFF = 0x1FFFF
    // off = 0x10000 + ((0x1E << 11) & 0x1FFFF) = 0x1F000
    let offset = banked_offset(0x30000, 0x1E, 0, 0xF000);
    assert_eq!(offset, 0x1F000);
}

#[test]
fn test_banking_read_f000() {
    let mut board = make_board();
    // Default bank[0] = 0x1E -> offset 0x1F000
    let val = board.read(0xF000);
    assert_eq!(val, board.m1[0x1F000]);
}

#[test]
fn test_bank_switch_via_port() {
    let mut board = make_board();
    // Switch bank 0 to 5 via IN A,(C) with C=$08, B=5
    let result = board.input(0x0508);
    assert_eq!(result, 0);
    assert_eq!(board.bank[0], 5);

    // Now read at $F000 should come from different offset
    // off = 0x10000 + ((5 << 11) & mask)
    let val = board.read(0xF000);
    let expected_offset = 0x10000 + ((5u32 << 11) & 0x1FFFF);
    assert_eq!(val, board.m1[expected_offset as usize]);
}

// === Command latch tests ===

#[test]
fn test_cmd_latch_read_clears_pending() {
    let mut board = make_board();
    board.host_write_cmd(0xAB);
    assert!(board.cmd_pending);

    let val = board.input(0x0000);
    assert_eq!(val, 0xAB);
    assert!(!board.cmd_pending);
}

#[test]
fn test_cmd_latch_write_clears_pending() {
    let mut board = make_board();
    board.host_write_cmd(0xAB);
    assert!(board.cmd_pending);

    board.output(0x0000, 0x00);
    assert!(!board.cmd_pending);
}

#[test]
fn test_reply_latch() {
    let mut board = make_board();
    board.output(0x000C, 0x42);
    assert_eq!(board.reply, 0x42);
}

// === NMI tests ===

#[test]
fn test_nmi_enable_disable() {
    let mut board = make_board();
    // Enable NMI: out ($08)
    board.output(0x0008, 0x00);
    assert!(board.nmi_enabled);

    // Disable NMI: out ($18)
    board.output(0x0018, 0x00);
    assert!(!board.nmi_enabled);
}

#[test]
fn test_nmi_level() {
    let mut board = make_board();
    board.host_write_cmd(1);
    assert!(board.cmd_pending);

    // NMI not enabled yet -> no NMI level
    assert!(!board.nmi_level());

    // Enable NMI
    board.output(0x0008, 0x00);
    assert!(board.nmi_enabled);
    assert!(board.nmi_level());

    // Read cmd latch clears pending -> no more NMI
    let _ = board.input(0x0000);
    assert!(!board.cmd_pending);
    assert!(!board.nmi_level());
}

// === RAM tests ===

#[test]
fn test_ram_read_write() {
    let mut board = make_board();
    board.write(0xF800, 0x11);
    assert_eq!(board.read(0xF800), 0x11);
}

#[test]
fn test_rom_write_ignored() {
    let mut board = make_board();
    let original = board.read(0x8000);
    board.write(0x8000, 0xFF);
    assert_eq!(board.read(0x8000), original);
}

#[test]
fn test_fixed_region_use_cart() {
    let mut board = make_board();
    // use_cart=false -> read from sm1
    board.use_cart = false;
    assert_eq!(board.read(0x0000), 0xCC);

    // use_cart=true -> read from m1
    board.use_cart = true;
    assert_eq!(board.read(0x0000), board.m1[0]);
}

// === YM2610 port tests ===

#[test]
fn test_ym_port_write() {
    let mut board = make_board();
    board.output(0x0004, 0x55); // YM address A
    assert_eq!(board.ym.write_port, Some((0, 0x55)));

    board.output(0x0005, 0xAA); // YM data A
    assert_eq!(board.ym.write_port, Some((1, 0xAA)));
}

#[test]
fn test_ym_port_read() {
    let mut board = make_board();
    let val = board.input(0x0004); // YM status
    assert_eq!(val, 0xAA);
    assert_eq!(board.ym.read_port, 0);
}

// === Real CPU execution tests ===

#[test]
fn test_cpu_out_instruction() {
    let mut board = make_board();
    let mut cpu = Cpu::new();
    cpu.reset();

    // Program: LD A,$12 ; OUT ($0C),A ; HALT
    // Bytes: 3E 12 D3 0C 76
    board.sm1 = vec![0x3E, 0x12, 0xD3, 0x0C, 0x76];

    for _ in 0..30 {
        if board.reply == 0x12 {
            break; // OUT completed and HALT reached
        }
        board.tick(&mut cpu);
    }

    assert_eq!(board.reply, 0x12);
}

#[test]
fn test_cpu_in_command_latch() {
    let mut board = make_board();
    let mut cpu = Cpu::new();
    cpu.reset();

    // Program: IN A,($00) ; HALT
    // Bytes: DB 00 76
    board.sm1 = vec![0xDB, 0x00, 0x76];

    board.host_write_cmd(0xAB);

    for _ in 0..30 {
        if cpu.at_instruction_boundary() && !board.cmd_pending {
            break; // IN completed and HALT reached
        }
        board.tick(&mut cpu);
    }

    assert_eq!(cpu.regs.a, 0xAB);
    assert!(!board.cmd_pending);
}

#[test]
fn test_cpu_nmi_vector() {
    let mut board = make_board();
    let mut cpu = Cpu::new();
    cpu.reset();

    // NMI vector is at $0066 on Z80
    // Program at 0x00: OUT ($08),A (enable NMI) - D3 08
    // Program at 0x66: LD A,$77 ; HALT - 3E 77 76
    board.sm1 = vec![0; 0x100];
    board.sm1[0] = 0xD3; // OUT ($08),A
    board.sm1[1] = 0x08;
    board.sm1[2] = 0x76; // HALT (after enabling NMI)

    board.sm1[0x66] = 0x3E; // LD A,$77
    board.sm1[0x67] = 0x77;
    board.sm1[0x68] = 0x76; // HALT

    board.host_write_cmd(1);

    for _ in 0..60 {
        if cpu.at_instruction_boundary() && cpu.regs.a == 0x77 {
            break; // NMI handler completed
        }
        board.tick(&mut cpu);
    }

    assert_eq!(cpu.regs.a, 0x77);
}

#[test]
fn test_time_advances_per_tick() {
    let mut board = make_board();
    let mut cpu = Cpu::new();
    cpu.reset();

    // Simple program: HALT immediately
    board.sm1 = vec![0x76];

    for _ in 0..5 {
        board.tick(&mut cpu);
    }

    assert_eq!(board.time, 30); // 5 ticks * 6 mclk each
}
