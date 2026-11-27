// Disks for FroggeOS.

#![no_std]

extern crate alloc;

pub mod lilypad;

use alloc::vec;
use alloc::vec::Vec;

// Disks read and write sectors of this many bytes.
pub const SECTOR_SIZE: usize = 512;

const PAST_END: &str = "the sector is past the end of the disk";

// A disk.
pub trait BlockDevice {
    // How many sectors the disk holds.
    fn sectors(&self) -> u64;

    // Copies `sector` into `buffer`.
    fn read(&mut self, sector: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str>;

    // Copies `buffer` into sector `sector`.
    fn write(&mut self, sector: u64, buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str>;

    // Ensures all sectors written so far are actually on the disk.
    fn flush(&mut self) -> Result<(), &'static str>;
}

// A disk kept on the heap.
pub struct RamDisk {
    data: Vec<[u8; SECTOR_SIZE]>,
}

impl RamDisk {
    pub fn new(sectors: u64) -> RamDisk {
        RamDisk { data: vec![[0; SECTOR_SIZE]; sectors as usize] }
    }
}

impl BlockDevice for RamDisk {
    fn sectors(&self) -> u64 {
        self.data.len() as u64
    }

    fn read(&mut self, sector: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        *buffer = *self.data.get(sector as usize).ok_or(PAST_END)?;
        Ok(())
    }

    fn write(&mut self, sector: u64, buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        *self.data.get_mut(sector as usize).ok_or(PAST_END)? = *buffer;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), &'static str> {
        Ok(())
    }
}
