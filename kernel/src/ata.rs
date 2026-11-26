// The ATA disk driver for the master drive on the primary IDE channel.

use storage::{BlockDevice, SECTOR_SIZE};
use crate::port::{inb, inw, outb};

// The primary channel's ports.
const DATA: u16 = 0x1F0;
const COUNT: u16 = 0x1F2;
const LBA_LOW: u16 = 0x1F3;
const LBA_MID: u16 = 0x1F4;
const LBA_HIGH: u16 = 0x1F5;
const DRIVE: u16 = 0x1F6;
const STATUS: u16 = 0x1F7;      // reading gives the status, writing sends a command
const ALT_STATUS: u16 = 0x3F6;  // the status, but reading it changes nothing

// Bits of the status.
const BUSY: u8 = 1 << 7;
const FAULT: u8 = 1 << 5;
const DATA_READY: u8 = 1 << 3;
const ERROR: u8 = 1 << 0;

// Commands.
const IDENTIFY: u8 = 0xEC;
const READ_SECTORS: u8 = 0x20;

// The drive as described by IDENTIFY.
pub struct Ata {
    sectors: u64,
    model: [u8; 40],
}

// Requests the type of drive from the master drive on the primary channel.
pub fn identify() -> Option<Ata> {
    let status = unsafe {
        outb(DRIVE, 0xA0);
        for port in [COUNT, LBA_LOW, LBA_MID, LBA_HIGH] {
            outb(port, 0);
        }
        outb(STATUS, IDENTIFY);
        inb(STATUS)
    };
    // 0 = No drive / 0xFF = No channel
    if status == 0 || status == 0xFF {
        return None;
    }
    wait().ok()?;
    let mut words = [0u16; 256];
    for word in &mut words {
        *word = unsafe { inw(DATA) };
    }
    // Words 27-46 hold the model.
    let mut model = [0; 40];
    for (pair, word) in model.chunks_exact_mut(2).zip(&words[27..47]) {
        pair.copy_from_slice(&word.to_be_bytes());
    }
    // Words 60 and 61 hold the number of sectors.
    let sectors = (words[61] as u64) << 16 | words[60] as u64;
    Some(Ata { sectors, model })
}

impl Ata {
    // The drive's name.
    pub fn model(&self) -> &str {
        str::from_utf8(&self.model).unwrap_or("?").trim_end()
    }
}

impl BlockDevice for Ata {
    fn sectors(&self) -> u64 {
        self.sectors
    }

    fn read(&mut self, sector: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        if sector >= self.sectors {
            return Err("the sector is past the end of the disk");
        }
        unsafe {
            outb(DRIVE, 0xE0 | (sector >> 24) as u8);
            outb(COUNT, 1);
            outb(LBA_LOW, sector as u8);
            outb(LBA_MID, (sector >> 8) as u8);
            outb(LBA_HIGH, (sector >> 16) as u8);
            outb(STATUS, READ_SECTORS);
        }
        wait()?;
        for pair in buffer.chunks_exact_mut(2) {
            let word = unsafe { inw(DATA) };
            pair.copy_from_slice(&word.to_le_bytes());
        }
        Ok(())
    }

    fn write(&mut self, _sector: u64, _buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        Err("ATA driver can't write yet")
    }
}

// Waits while the drive is busy then checks for data readiness.
fn wait() -> Result<(), &'static str> {
    // The status requires 400 ns to catch up with a new command.
    for _ in 0..4 {
        unsafe { inb(ALT_STATUS) };
    }
    let mut status = unsafe { inb(STATUS) };
    while status & BUSY != 0 {
        status = unsafe { inb(STATUS) };
    }
    if status & (ERROR | FAULT) != 0 {
        return Err("the disk reported an error");
    }
    if status & DATA_READY == 0 {
        return Err("the disk has no data ready");
    }
    Ok(())
}
