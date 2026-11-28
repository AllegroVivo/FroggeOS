// Custom filesystem for FroggeOS. (See FILESYSTEM.md)

use alloc::string::String;
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

    pub fn mark_free(&mut self, block: u32) {
        self.bytes[block as usize / 8] &= !(1 << (block % 8));
    }

    // Locates the first run of `count` free blocks in `start..end`, if any.
    pub fn find_free(&self, count: u32, start: u32, end: u32) -> Option<u32> {
        let mut run = 0;
        for block in start..end {
            if self.is_used(block) {
                run = 0;
            } else {
                run += 1;
                if run == count {
                    return Some(block + 1 - count);
                }
            }
        }
        None
    }

    // Writes the bitmap to `disk`, starting at block `start`.
    pub fn write(&self, disk: &mut dyn BlockDevice, start: u32) -> Result<(), &'static str> {
        for (i, chunk) in self.bytes.chunks_exact(BLOCK_SIZE).enumerate() {
            disk.write(start as u64 + i as u64, chunk.try_into().unwrap())?;
        }
        Ok(())
    }

    // Reads a bitmap from `disk` for `blocks blocks, beginning at `start`.
    pub fn read(disk: &mut dyn BlockDevice, start: u32, blocks: u32) -> Result<Bitmap, &'static str> {
        let mut bitmap = Bitmap::new(blocks);
        for (i, chunk) in bitmap.bytes.chunks_exact_mut(BLOCK_SIZE).enumerate() {
            disk.read(start as u64 + i as u64, chunk.try_into().unwrap())?;
        }
        Ok(bitmap)
    }
}

// Builds a new LilyPad on all of a disk.
pub struct Packer<'a> {
    disk: &'a mut dyn BlockDevice,
    blocks: u32,
    bitmap: Bitmap,
    next: u32       // the first block with no file yet
}

impl<'a> Packer<'a> {
    pub fn new(disk: &'a mut dyn BlockDevice) -> Result<Packer<'a>, &'static str> {
        let blocks = u32::try_from(disk.sectors()).map_err(|_| "the disk is too big for the LilyPad filesystem")?;
        let mut bitmap = Bitmap::new(blocks);
        let next = 1 + bitmap.blocks();
        if next > blocks {
            return Err("the disk is too small for the LilyPad filesystem");
        }
        for block in 0..next {
            bitmap.mark_used(block);
        }
        Ok(Packer { disk, blocks, bitmap, next })
    }

    // Writes a file's contents to the next free block(s), returning the first.
    pub fn add(&mut self, contents: &[u8]) -> Result<u32, &'static str> {
        if contents.is_empty() {
            return Ok(0);
        }
        if contents.len().div_ceil(BLOCK_SIZE) > (self.blocks - self.next) as usize {
            return Err("LilyPad is full");
        }
        let first = self.next;
        for chunk in contents.chunks(BLOCK_SIZE) {
            let mut block = [0; BLOCK_SIZE];
            block[..chunk.len()].copy_from_slice(chunk);
            self.disk.write(self.next as u64, &block)?;
            self.bitmap.mark_used(self.next);
            self.next += 1;
        }
        Ok(first)
    }

    // Writes the bitmap, a superblock containing `root`, then flushes.
    pub fn finish(self, root: Entry) -> Result<(), &'static str> {
        let superblock = Superblock { blocks: self.blocks, bitmap_start: 1, bitmap_blocks: self.bitmap.blocks(), root };
        self.bitmap.write(self.disk, superblock.bitmap_start)?;
        self.disk.write(0, &superblock.to_bytes())?;
        self.disk.flush()
    }
}

// Formats all of `disk` as an empty LilyPad.
pub fn format(disk: &mut dyn BlockDevice, time: Time) -> Result<(), &'static str> {
    Packer::new(disk)?.finish(Entry::root(time))
}

// A LilyPad with validated structure.
pub struct LilyPad<D: BlockDevice> {
    disk: D,
    superblock: Superblock
}

impl<D: BlockDevice> LilyPad<D> {
    // Reads the superblock at `disk` and checks that it fits the disk.
    pub fn mount(mut disk: D) -> Result<LilyPad<D>, &'static str> {
        let mut block = [0; BLOCK_SIZE];
        disk.read(0, &mut block)?;
        let superblock = Superblock::from_bytes(&block)?;
        let blocks = superblock.blocks;
        if blocks as u64 > disk.sectors() {
            return Err("LilyPad is bigger than its assigned disk");
        }
        let bitmap_blocks = blocks.div_ceil(BITS_PER_BLOCK);
        if superblock.bitmap_start != 1 || superblock.bitmap_blocks != bitmap_blocks || blocks <= bitmap_blocks {
            return Err("the superblock's bitmap doesn't fit LilyPad");
        }
        if !superblock.root.is_directory() {
            return Err("the root isn't a directory");
        }
        Ok(LilyPad { disk, superblock })
    }

    pub fn superblock(&self) -> &Superblock {
        &self.superblock
    }

    // How many blocks are used by the bitmap marks.
    pub fn used(&mut self) -> Result<u32, &'static str> {
        let blocks = self.superblock.blocks;
        let bitmap = Bitmap::read(&mut self.disk, self.superblock.bitmap_start, blocks)?;
        Ok((0..blocks).filter(|&block| bitmap.is_used(block)).count() as u32)
    }

    // Locates and marks used `count` free blocks in a row, returning the first.
    pub fn allocate(&mut self, count: u32) -> Result<u32, &'static str> {
        if count == 0 {
            return Ok(0);
        }
        let start = self.superblock.bitmap_start;
        let mut bitmap = Bitmap::read(&mut self.disk, start, self.superblock.blocks)?;
        let files_start = 1 + self.superblock.bitmap_blocks;
        let first = bitmap.find_free(count, files_start, self.superblock.blocks).ok_or("LilyPad has no free run of that length")?;
        for block in first..first + count {
            bitmap.mark_used(block);
        }
        bitmap.write(&mut self.disk, start)?;
        Ok(first)
    }

    // Locates and marks `count` blocks from `first` as freed.
    pub fn free(&mut self, first: u32, count: u32) -> Result<(), &'static str> {
        if count == 0 {
            return Ok(());
        }
        let files_start = 1 + self.superblock.bitmap_blocks;
        if first < files_start || first as u64 + count as u64 > self.superblock.blocks as u64 {
            return Err("the blocks to free are outside the LilyPad block");
        }
        let start = self.superblock.bitmap_start;
        let mut bitmap = Bitmap::read(&mut self.disk, start, self.superblock.blocks)?;
        for block in first..first + count {
            if !bitmap.is_used(block) {
                return Err("a block to free has already been freed.");
            }
            bitmap.mark_free(block);
        }
        bitmap.write(&mut self.disk, start)
    }

    // Reads a whole file or directory.
    pub fn read(&mut self, entry: &Entry) -> Result<Vec<u8>, &'static str> {
        let blocks = entry.size.div_ceil(BLOCK_SIZE as u64);
        let files_start = 1 + self.superblock.bitmap_blocks;
        if blocks > 0 && (entry.first < files_start || entry.first as u64 + blocks > self.superblock.blocks as u64) {
            return Err("the files blocks are outside the LilyPad-managed file structure");
        }
        let mut contents = vec![0; blocks as usize * BLOCK_SIZE];
        for (i, chunk) in contents.chunks_exact_mut(BLOCK_SIZE).enumerate() {
            self.disk.read(entry.first as u64 + i as u64, chunk.try_into().unwrap())?;
        }
        contents.truncate(entry.size as usize);
        Ok(contents)
    }

    // The files and other directories within `directory`.
    pub fn list(&mut self, directory: &Entry) -> Result<Vec<Entry>, &'static str> {
        if !directory.is_directory() {
            return Err("not a directory");
        }
        let contents = self.read(directory)?;
        Ok(contents.chunks_exact(ENTRY_SIZE).filter_map(|bytes| Entry::from_bytes(bytes.try_into().unwrap())).collect())
    }

    // Locates the file or directory at `path`.
    pub fn find(&mut self, path: &str) -> Result<Entry, &'static str> {
        Ok(self.resolve(path)?.1)
    }

    pub fn resolve(&mut self, path: &str) -> Result<(String, Entry), &'static str> {
        let rest = path.strip_prefix('/').ok_or("a path must start with '/'")?;
        let mut names = Vec::new();
        for name in rest.split('/') {
            match name {
                "" | "." => {},
                ".." => {
                    names.pop();
                },
                _ => names.push(name),
            }
        }
        let mut entry = self.superblock.root;
        let mut spelled = String::new();
        for name in names {
            let children = self.list(&entry)?;
            entry = children.into_iter().find(|child| child.is_named(name)).ok_or("no such file or directory")?;
            spelled.push('/');
            spelled.push_str(entry.name());
        }
        if spelled.is_empty() {
            spelled.push('/');
        }
        Ok((spelled, entry))
    }
}

// `path` as a path from the root.
pub fn join(directory: &str, path: &str) -> String {
    if path.starts_with('/') {
        return String::from(path);
    }
    let mut joined = String::from(directory);
    if !joined.ends_with('/') {
        joined.push('/');
    }
    joined.push_str(path);
    joined
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
