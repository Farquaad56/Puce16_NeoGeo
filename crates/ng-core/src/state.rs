//! Savestates (T604) : `Machine::save_state` et `Machine::load_state`.
//!
//! Format : magic `NGST`, version u32 (little-endian), puis une suite de blocs bincode.
//! Les ROM (cartouche, BIOS) ne sont pas sauvegardées : le CRC32 de la ROM P est stocké et
//! vérifié au chargement. Les deux CPU sont sauvés à une frontière d'instruction.
//! La RAM de cartouche (V-Liner, Jockey GP) est sauvée depuis la version 2, la sélection du panneau
//! mahjong depuis la version 3, les compteurs trackball / molette depuis la version 4.
//! Limites connues : l'état STOP/arrêt du 68000 (Q-M68KSTATE) et l'état d'un port de
//! protection de cartouche ne sont pas sauvegardés.

use ng_common::crc32_bytes;
use puce_m68k::state::CpuState;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::machine::Machine;

const MAGIC: &[u8; 4] = b"NGST";
const VERSION: u32 = 4;

/// Registres du 68000 : d, a, usp, ssp, sr, pc, prefetch.
type M68 = ([u32; 8], [u32; 7], u32, u32, u16, u32, [u16; 2]);
/// Registres d'E/S : pads, système, test, réponse son, chien de garde, commande son, DIP, RTC, carte.
type IoState = ([u8; 2], u8, bool, u8, bool, Option<u8>, u8, bool, bool, u8);

fn put<T: Serialize + ?Sized>(buf: &mut Vec<u8>, v: &T) {
    bincode::serialize_into(buf, v).expect("écriture d'un savestate en mémoire");
}

fn get<T: DeserializeOwned>(rd: &mut &[u8]) -> Result<T, String> {
    bincode::deserialize_from(rd).map_err(|e| format!("savestate illisible : {e}"))
}

fn fill<T: Copy>(dst: &mut [T], src: Vec<T>) -> Result<(), String> {
    if dst.len() != src.len() {
        return Err(format!(
            "savestate : bloc de {} éléments, {} attendus",
            src.len(),
            dst.len()
        ));
    }
    dst.copy_from_slice(&src);
    Ok(())
}

impl Machine {
    /// Sérialise tout l'état. Termine d'abord l'instruction en cours des deux CPU.
    pub fn save_state(&mut self) -> Vec<u8> {
        self.finish_instruction();
        {
            let (_, mut host) = self.split();
            host.catch_up();
        }
        while !self.z80.at_instruction_boundary() {
            self.sound.tick(&mut self.z80);
        }

        let mut buf = Vec::with_capacity(0x60000);
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&VERSION.to_le_bytes());
        put(&mut buf, &crc32_bytes(&self.bus.cart_p));

        let s = self.cpu.state();
        let m68: M68 = (s.d, s.a, s.usp, s.ssp, s.sr, s.pc, s.prefetch);
        put(&mut buf, &m68);
        let z80 = self
            .z80
            .save_state()
            .expect("Z80 à une frontière d'instruction");
        put(&mut buf, &z80);

        put(&mut buf, &self.bus.work_ram[..]);
        put(
            &mut buf,
            &(
                self.bus.vectors_from_cart,
                self.bus.pbank_base,
                self.bus.open_bus,
            ),
        );

        let snd = &self.sound;
        put(&mut buf, &snd.ram[..]);
        put(
            &mut buf,
            &(
                snd.time,
                snd.use_cart,
                snd.bank,
                snd.cmd,
                snd.cmd_pending,
                snd.reply,
                snd.nmi_enabled,
                snd.port_state(),
            ),
        );
        snd.ym.save_into(&mut buf);

        put(&mut buf, &*self.lspc.borrow());
        {
            let pal = self.pal.borrow();
            put(&mut buf, &pal.ram[..]);
            put(&mut buf, &pal.pens[..]);
            put(&mut buf, &(pal.bank, pal.shadow));
        }
        {
            let io = self.io.borrow();
            let st: IoState = (
                [io.pad[0].bits, io.pad[1].bits],
                io.system.bits,
                io.test_pressed,
                io.sound_reply,
                io.watchdog_kick,
                io.pending_sound_cmd,
                io.dipsw,
                io.rtc_tp,
                io.rtc_data_out,
                io.card_bank,
            );
            put(&mut buf, &st);
        }
        {
            let backup = self.backup.borrow();
            put(&mut buf, &backup.data[..]);
            put(&mut buf, &backup.unlocked);
        }
        put(&mut buf, &self.latch.q);
        put(&mut buf, &self.rtc);
        put(
            &mut buf,
            &(
                self.wd.counter,
                self.now,
                self.frame_start,
                self.synced,
                self.reset_request,
            ),
        );
        put(&mut buf, &self.fb.pixels);
        put(&mut buf, &self.audio);
        // v2 : RAM de cartouche (vide si la cartouche n'en a pas).
        let cart_ram: Vec<u8> = self
            .bus
            .port
            .as_ref()
            .and_then(|p| p.nv_ram())
            .map(|r| r.to_vec())
            .unwrap_or_default();
        put(&mut buf, &cart_ram);
        // v3 : sélection des rangées du panneau mahjong.
        put(&mut buf, &self.io.borrow().mj_sel);
        // v4 : compteurs trackball / molette.
        {
            let io = self.io.borrow();
            put(&mut buf, &(io.track, io.dial_pos));
        }
        buf
    }

    /// Restaure un état produit par `save_state` pour le même jeu.
    ///
    /// L'en-tête et le CRC de la ROM P sont vérifiés avant toute modification ; si la suite
    /// est corrompue, la machine est laissée dans un état partiel (faire un `reset`).
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        if data.len() < 8 || &data[..4] != MAGIC {
            return Err("savestate : magic NGST absent".to_string());
        }
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if version != VERSION {
            return Err(format!("savestate : version {version} non prise en charge"));
        }
        let mut rd = &data[8..];
        let crc: u32 = get(&mut rd)?;
        if crc != crc32_bytes(&self.bus.cart_p) {
            return Err("savestate : crc de la ROM P différent (autre jeu)".to_string());
        }

        let (d, a, usp, ssp, sr, pc, prefetch): M68 = get(&mut rd)?;
        self.cpu.load_state(&CpuState {
            d,
            a,
            usp,
            ssp,
            sr,
            pc,
            prefetch,
        });
        self.z80.load_state(&get(&mut rd)?);

        fill(&mut self.bus.work_ram[..], get(&mut rd)?)?;
        (
            self.bus.vectors_from_cart,
            self.bus.pbank_base,
            self.bus.open_bus,
        ) = get(&mut rd)?;

        fill(&mut self.sound.ram[..], get(&mut rd)?)?;
        let port;
        let snd = &mut self.sound;
        (
            snd.time,
            snd.use_cart,
            snd.bank,
            snd.cmd,
            snd.cmd_pending,
            snd.reply,
            snd.nmi_enabled,
            port,
        ) = get(&mut rd)?;
        snd.set_port_state(port);
        snd.ym.load_from(&mut rd)?;

        *self.lspc.borrow_mut() = get(&mut rd)?;
        {
            let mut pal = self.pal.borrow_mut();
            fill(&mut pal.ram[..], get(&mut rd)?)?;
            fill(&mut pal.pens[..], get(&mut rd)?)?;
            (pal.bank, pal.shadow) = get(&mut rd)?;
        }
        {
            let st: IoState = get(&mut rd)?;
            let mut io = self.io.borrow_mut();
            io.pad[0].bits = st.0[0];
            io.pad[1].bits = st.0[1];
            io.system.bits = st.1;
            io.test_pressed = st.2;
            io.sound_reply = st.3;
            io.watchdog_kick = st.4;
            io.pending_sound_cmd = st.5;
            io.dipsw = st.6;
            io.rtc_tp = st.7;
            io.rtc_data_out = st.8;
            io.card_bank = st.9;
        }
        {
            let mut backup = self.backup.borrow_mut();
            fill(&mut backup.data[..], get(&mut rd)?)?;
            backup.unlocked = get(&mut rd)?;
        }
        self.latch.q = get(&mut rd)?;
        self.rtc = get(&mut rd)?;
        (
            self.wd.counter,
            self.now,
            self.frame_start,
            self.synced,
            self.reset_request,
        ) = get(&mut rd)?;
        fill(&mut self.fb.pixels[..], get(&mut rd)?)?;
        self.audio = get(&mut rd)?;
        let cart_ram: Vec<u8> = get(&mut rd)?;
        match self.bus.port.as_mut().and_then(|p| p.nv_ram_mut()) {
            Some(ram) => fill(ram, cart_ram)?,
            None if cart_ram.is_empty() => {}
            None => return Err("savestate : RAM de cartouche inattendue".to_string()),
        }
        self.io.borrow_mut().mj_sel = get(&mut rd)?;
        {
            let mut io = self.io.borrow_mut();
            (io.track, io.dial_pos) = get(&mut rd)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::tests_support::{machine_with, program_bios};

    fn running() -> Machine {
        // ADDQ.W #1,$100000 ; BRA.s au début : la RAM change à chaque tour.
        machine_with(
            program_bios(&[0x52, 0x79, 0x00, 0x10, 0x00, 0x00, 0x60, 0xF8]),
            0x00,
        )
    }

    fn fb_crc(m: &Machine) -> u32 {
        let bytes: Vec<u8> = m.fb.pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
        crc32_bytes(&bytes)
    }

    #[test]
    fn cart_ram_survives_save_state() {
        let mut m = running();
        m.bus.port = Some(Box::new(ng_bus::prot_cartram::CartRam::new(false)));
        m.bus.write16(0x200020, 0xCAFE, true, true);
        let st = m.save_state();
        m.bus.write16(0x200020, 0x0000, true, true);
        m.load_state(&st).unwrap();
        assert_eq!(m.bus.read16(0x200020), 0xCAFE);
        let again = m.save_state();
        assert_eq!(again.len(), st.len());
        // Un état avec RAM de cartouche est refusé par une machine sans RAM.
        let mut plain = running();
        assert!(plain.load_state(&st).is_err());
    }

    #[test]
    fn save_load_save_is_identical() {
        let mut m = running();
        for _ in 0..3 {
            m.run_frame();
        }
        let s1 = m.save_state();
        assert_eq!(&s1[..4], b"NGST");
        m.load_state(&s1).unwrap();
        assert_eq!(m.save_state(), s1);
    }

    #[test]
    fn load_resumes_deterministically() {
        let mut m = running();
        for _ in 0..10 {
            m.run_frame();
        }
        let saved = m.save_state();
        for _ in 0..10 {
            m.run_frame();
        }
        let (crc_a, end_a) = (fb_crc(&m), m.save_state());
        m.load_state(&saved).unwrap();
        for _ in 0..10 {
            m.run_frame();
        }
        assert_eq!(fb_crc(&m), crc_a);
        assert!(
            m.save_state() == end_a,
            "état final différent après reprise"
        );
    }

    #[test]
    fn other_game_is_refused() {
        let mut m = running();
        let saved = m.save_state();
        let mut other = running();
        other.bus.cart_p[0] ^= 0xFF;
        let err = other.load_state(&saved).unwrap_err();
        assert!(err.contains("crc"), "{err}");
    }

    #[test]
    fn bad_header_is_refused() {
        let mut m = running();
        assert!(m.load_state(b"XXXX\x01\0\0\0").is_err());
        assert!(m.load_state(b"NGST\x02\0\0\0").is_err());
    }
}
