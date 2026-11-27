// Custom filesystem for FroggeOS. (See FILESYSTEM.md)

use alloc::vec;
use alloc::vec::Vec;

use crate::{BlockDevice, SECTOR_SIZE};

pub const ENTRY_SIZE: usize = 64;
pub const NAME_SIZE: usize = 32;

// Flag bit set in a directory's entry.
pub const DIRECTORY: u32 = 1 << 0;

// A block is one sector.
pub const BLOCK_SIZE: usize = SECTOR_SIZE;

// The byte string starting every superblock.
pub const MAGIC: [u8; 8] = *b"LILYPAD!";
pub const VERSION: u32 = 1;

// How many blocks covered by one block of the bitmap.
pub const BITS_PER_BLOCK: u32 = BLOCK_SIZE as u32 * 8;

// When a file last changed. (In local time)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Time {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

// A file or directory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    name: [u8; NAME_SIZE],
    pub flags: u32,
    pub first: u32,     // The first block
    pub size: u64,      // in bytes
    pub time: Time,
}

impl Entry {
    // Creates an entry named `name`.
    pub fn new(name: &str, flags: u32, first: u32, size: u64, time: Time) -> Result<Entry, &'static str> {
        if !valid_name(name) {
            return Err("a name is 1 to 32 printable ASCII characters, without spaces or '/'");
        }
        let mut padded = [0; NAME_SIZE];
        padded[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Entry { name: padded, flags, first, size, time })
    }

    // The root directory's entry.
    pub fn root(time: Time) -> Entry {
        let mut name = [0; NAME_SIZE];
        name[0] = b'/';
        Entry { name, flags: DIRECTORY, first: 0, size: 0, time }
    }

    // The entry's name without any padding zeros.
    pub fn name(&self) -> &str {
        let length = self.name.iter().position(|&byte| byte == 0).unwrap_or(NAME_SIZE);
        str::from_utf8(&self.name[..length]).unwrap_or("?")
    }

    // Case-insensitively compares this entry's name.
    pub fn is_named(&self, name: &str) -> bool {
        self.name().eq_ignore_ascii_case(name)
    }

    pub fn is_directory(&self) -> bool {
        self.flags & DIRECTORY != 0
    }

    // The entry as stored by the disk.
    pub fn to_bytes(&self) -> [u8; ENTRY_SIZE] {
        let time = self.time;
        let mut bytes = [0; ENTRY_SIZE];
        bytes[0..32].copy_from_slice(&self.name);
        bytes[32..36].copy_from_slice(&self.flags.to_le_bytes());
        bytes[36..40].copy_from_slice(&self.first.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.size.to_le_bytes());
        bytes[48..50].copy_from_slice(&time.year.to_le_bytes());
        bytes[50..55].copy_from_slice(&[time.month, time.day, time.hour, time.minute, time.second]);
        bytes
    }

    // The Entry stored in `bytes`.
    pub fn from_bytes(bytes: &[u8; ENTRY_SIZE]) -> Option<Entry> {
        if bytes[0] == 0 {
            return None;
        }
        Some(Entry {
            name: bytes[0..32].try_into().unwrap(),
            flags: u32_at(bytes, 32),
            first: u32_at(bytes, 36),
            size: u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            time: Time {
                year: u16::from_le_bytes([bytes[48], bytes[49]]),
                month: bytes[50],
                day: bytes[51],
                hour: bytes[52],
                minute: bytes[53],
                second: bytes[54],
            },
        })
    }
}

// Whether `name` follows the filesystem's naming convention rules.
pub fn valid_name(name: &str) -> bool {
    (1..=NAME_SIZE).contains(&name.len())
        && name.bytes().all(|byte| byte.is_ascii_graphic() && byte != b'/')
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Superblock {
    pub blocks: u32,        // all blocks in the filesystem,
    pub bitmap_start: u32,
    pub bitmap_blocks: u32,
    pub root: Entry,
}

impl Superblock {
    pub fn to_bytes(&self) -> [u8; BLOCK_SIZE] {
        let mut bytes = [0; BLOCK_SIZE ];
        bytes[0..8].copy_from_slice(&MAGIC);
        bytes[8..12].copy_from_slice(&VERSION.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BLOCK_SIZE as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&self.blocks.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.bitmap_start.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.bitmap_blocks.to_le_bytes());
        bytes[32..96].copy_from_slice(&self.root.to_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8; BLOCK_SIZE]) -> Result<Superblock, &'static str> {
        if bytes[0..8] != MAGIC {
            return Err("the disk isn't formatted with LilyPad")
        }
        if u32_at(bytes, 8) != VERSION || u32_at(bytes, 12) != BLOCK_SIZE as u32 {
            return Err("this LilyPad version isn't supported :(")
        }
        let root = Entry::from_bytes(bytes[32..96].try_into().unwrap());
        Ok(Superblock {
            blocks: u32_at(bytes, 16),
            bitmap_start: u32_at(bytes, 20),
            bitmap_blocks: u32_at(bytes, 24),
            root: root.ok_or("the superblock has no root directory")?,
        })
    }
}

pub struct Bitmap {
    bytes: Vec<u8>,
}

impl Bitmap {
    pub fn new(blocks: u32) -> Bitmap {
        let size = blocks.div_ceil(BITS_PER_BLOCK) as usize * BLOCK_SIZE;
        let mut bitmap = Bitmap { bytes: vec![0; size] };
        for block in blocks as usize..size * 8 {
            bitmap.mark_used(block as u32);
        }
        bitmap
    }

    // How many blocks the bitmap takes on the disk.
    pub fn blocks(&self) -> u32 {
        (self.bytes.len() / BLOCK_SIZE) as u32
    }

    pub fn is_used(&self, block: u32) -> bool {
        self.bytes[block as usize / 8] & (1 << (block % 8)) != 0
    }

    pub fn mark_used(&mut self, block: u32) {
        self.bytes[block as usize / 8] |= 1 << (block % 8);
    }

    // Writes the bitmap to `disk`, starting at block `start`.
    pub fn write(&self, disk: &mut dyn BlockDevice, start: u32) -> Result<(), &'static str> {
        for (i, chunk) in self.bytes.chunks_exact(BLOCK_SIZE).enumerate() {
            disk.write(start as u64 + i as u64, chunk.try_into().unwrap())?;
        }
        Ok(())
    }
}

// Formats all of `disk ` as an empty LilyPad.
pub fn format(disk: &mut dyn BlockDevice, time: Time) -> Result<(), &'static str> {
    let blocks = u32::try_from(disk.sectors()).map_err(|_| "the disk is too big for LilyPad")?;
    let mut bitmap = Bitmap::new(blocks);
    let bitmap_blocks = bitmap.blocks();
    if blocks <= bitmap_blocks {
        return Err("the disk is too small for LilyPad");
    }
    for block in 0..=bitmap_blocks {
        bitmap.mark_used(block);
    }
    let superblock = Superblock { blocks, bitmap_start: 1, bitmap_blocks, root: Entry::root(time) };
    bitmap.write(disk, superblock.bitmap_start)?;
    disk.write(0, &superblock.to_bytes())?;
    disk.flush()
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
