use storage::lilypad::{
    BITS_PER_BLOCK, BLOCK_SIZE, Bitmap, DIRECTORY, ENTRY_SIZE, Entry, MAGIC, Superblock, Time, VERSION, format,
    valid_name,
};
use storage::{BlockDevice, RamDisk, SECTOR_SIZE};

const TIME: Time = Time { year: 2026, month: 11, day: 26, hour: 13, minute: 45, second: 7 };

// A file entry with numbers whose every byte differs, so a field at the
// wrong offset, or in the wrong byte order, would show.
fn sample() -> Entry {
    Entry::new("ReadMe.txt", 0, 0x0403_0201, 0x0F0E_0D0C_0B0A_0908, TIME).unwrap()
}

#[test]
fn names_follow_the_rules() {
    let longest = "x".repeat(32);
    for name in ["a", "README.TXT", "hello_world-2.rbt", "~!#$%^&*()", longest.as_str()] {
        assert!(valid_name(name), "{name:?} should be allowed");
    }
    let too_long = "x".repeat(33);
    for name in ["", "two words", "a/b", "/", "tab\there", "\u{e9}t\u{e9}", "nul\0", too_long.as_str()] {
        assert!(!valid_name(name), "{name:?} should be refused");
    }
}

#[test]
fn new_checks_the_name() {
    let refused = Err("a name is 1 to 32 printable ASCII characters, without spaces or '/'");
    assert_eq!(Entry::new("two words", 0, 0, 0, TIME), refused);
    assert_eq!(Entry::new("", DIRECTORY, 0, 0, TIME), refused);
    assert_eq!(sample().name(), "ReadMe.txt");
}

#[test]
fn bytes_match_the_spec() {
    let bytes = sample().to_bytes();
    assert_eq!(bytes.len(), ENTRY_SIZE);
    assert_eq!(&bytes[0..10], b"ReadMe.txt");
    assert_eq!(bytes[10..32], [0; 22]);
    assert_eq!(bytes[32..36], [0, 0, 0, 0]);
    assert_eq!(bytes[36..40], [0x01, 0x02, 0x03, 0x04]);
    assert_eq!(bytes[40..48], [0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F]);
    // 2026 is 0x07EA, then the month, day, hour, minute and second.
    assert_eq!(bytes[48..55], [0xEA, 0x07, 11, 26, 13, 45, 7]);
    assert_eq!(bytes[55..64], [0; 9]);
}

#[test]
fn bytes_read_back_as_the_same_entry() {
    let longest = "Z".repeat(32);
    let directory = Entry::new(&longest, DIRECTORY, 5, 128, TIME).unwrap();
    for entry in [sample(), directory] {
        assert_eq!(Entry::from_bytes(&entry.to_bytes()), Some(entry));
    }
    // A 32-letter name fills its field, with no zero after it.
    assert_eq!(Entry::from_bytes(&directory.to_bytes()).unwrap().name(), longest);
}

#[test]
fn free_entries_read_as_none() {
    assert_eq!(Entry::from_bytes(&[0; ENTRY_SIZE]), None);
    // Only the first byte counts: a deleted entry may keep its other bytes.
    let mut deleted = sample().to_bytes();
    deleted[0] = 0;
    assert_eq!(Entry::from_bytes(&deleted), None);
}

#[test]
fn case_is_kept_but_ignored() {
    let entry = sample();
    assert_eq!(entry.name(), "ReadMe.txt");
    for name in ["ReadMe.txt", "readme.txt", "README.TXT", "rEaDmE.TxT"] {
        assert!(entry.is_named(name), "{name:?}");
    }
    for name in ["ReadMe.tx", "ReadMe.txt2", "ReadMe", ""] {
        assert!(!entry.is_named(name), "{name:?}");
    }
}

#[test]
fn directories_have_the_flag() {
    assert!(!sample().is_directory());
    assert!(Entry::new("docs", DIRECTORY, 5, 64, TIME).unwrap().is_directory());
}

// Block `n` of `disk`.
fn read_block(disk: &mut dyn BlockDevice, n: u64) -> [u8; BLOCK_SIZE] {
    let mut buffer = [0; BLOCK_SIZE];
    disk.read(n, &mut buffer).unwrap();
    buffer
}

#[test]
fn the_root_is_an_empty_directory() {
    let root = Entry::root(TIME);
    assert_eq!(root.name(), "/");
    assert!(root.is_directory());
    assert_eq!((root.first, root.size, root.time), (0, 0, TIME));
    // No other entry may be called "/".
    assert!(Entry::new("/", DIRECTORY, 0, 0, TIME).is_err());
}

// A superblock with numbers whose every byte differs, like sample()'s.
fn sample_superblock() -> Superblock {
    Superblock { blocks: 0x0403_0201, bitmap_start: 0x0807_0605, bitmap_blocks: 0x0C0B_0A09, root: Entry::root(TIME) }
}

#[test]
fn superblock_bytes_match_the_spec() {
    assert_eq!((MAGIC, VERSION, BLOCK_SIZE, BITS_PER_BLOCK), (*b"LILYPAD!", 1, 512, 4096));
    let bytes = sample_superblock().to_bytes();
    assert_eq!(&bytes[0..8], b"LILYPAD!");
    assert_eq!(bytes[8..12], [1, 0, 0, 0]);
    // 512 is 0x0200.
    assert_eq!(bytes[12..16], [0x00, 0x02, 0, 0]);
    assert_eq!(bytes[16..20], [1, 2, 3, 4]);
    assert_eq!(bytes[20..24], [5, 6, 7, 8]);
    assert_eq!(bytes[24..28], [9, 10, 11, 12]);
    assert_eq!(bytes[28..32], [0; 4]);
    assert_eq!(bytes[32..96], Entry::root(TIME).to_bytes());
    assert_eq!(&bytes[32..34], b"/\0");
    assert_eq!(bytes[96..], [0; 416]);
}

#[test]
fn superblocks_read_back() {
    let superblock = sample_superblock();
    assert_eq!(Superblock::from_bytes(&superblock.to_bytes()), Ok(superblock));
}

#[test]
fn only_lilypad_superblocks_are_read() {
    let not_lilypad = Err("the disk isn't formatted with LilyPad");
    let unsupported = Err("this LilyPad version isn't supported :(");
    // The sample superblock with one byte changed.
    let changed = |at: usize, byte: u8| {
        let mut bytes = sample_superblock().to_bytes();
        bytes[at] = byte;
        Superblock::from_bytes(&bytes)
    };
    assert_eq!(Superblock::from_bytes(&[0; BLOCK_SIZE]), not_lilypad);
    assert_eq!(changed(7, b'?'), not_lilypad);
    // Version 2, then 1024-byte blocks.
    assert_eq!(changed(8, 2), unsupported);
    assert_eq!(changed(13, 4), unsupported);
    assert_eq!(changed(32, 0), Err("the superblock has no root directory"));
}

#[test]
fn bitmaps_cover_whole_blocks() {
    // 14,336 blocks, as on the 8 MB disk, need 4 bitmap blocks. Those have
    // room for 16,384 bits, and the 2,048 past the end are set.
    let bitmap = Bitmap::new(14_336);
    assert_eq!(bitmap.blocks(), 4);
    assert!((0..14_336).all(|block| !bitmap.is_used(block)));
    assert!((14_336..16_384).all(|block| bitmap.is_used(block)));
    // One block more than a bitmap block covers needs a second one.
    assert_eq!(Bitmap::new(4096).blocks(), 1);
    let bitmap = Bitmap::new(4097);
    assert_eq!(bitmap.blocks(), 2);
    assert!(!bitmap.is_used(4096));
    assert!((4097..8192).all(|block| bitmap.is_used(block)));
}

#[test]
fn bits_go_lowest_first() {
    let mut bitmap = Bitmap::new(8192);
    for block in [0, 9, 4095, 4096] {
        bitmap.mark_used(block);
    }
    assert!(bitmap.is_used(9) && !bitmap.is_used(8) && !bitmap.is_used(10));
    let mut disk = RamDisk::new(3);
    bitmap.write(&mut disk, 1).unwrap();
    assert_eq!(read_block(&mut disk, 0), [0; BLOCK_SIZE]);
    // Block 9 is bit 1 of byte 1, and block 4095 is the top bit of the last
    // byte. Block 4096 starts the second bitmap block.
    let first = read_block(&mut disk, 1);
    assert_eq!((first[0], first[1], first[511]), (0b0000_0001, 0b0000_0010, 0b1000_0000));
    assert_eq!(first[2..511], [0; 509]);
    let second = read_block(&mut disk, 2);
    assert_eq!(second[0], 0b0000_0001);
    assert_eq!(second[1..], [0; 511]);
}

#[test]
fn format_writes_an_empty_lilypad() {
    let mut disk = RamDisk::new(14_336);
    let marker = [0xAB; BLOCK_SIZE];
    for n in [5, 14_335] {
        disk.write(n, &marker).unwrap();
    }
    format(&mut disk, TIME).unwrap();
    let expected = Superblock { blocks: 14_336, bitmap_start: 1, bitmap_blocks: 4, root: Entry::root(TIME) };
    assert_eq!(Superblock::from_bytes(&read_block(&mut disk, 0)), Ok(expected));
    // Blocks 0 to 4, the superblock and the bitmap, are used.
    let first = read_block(&mut disk, 1);
    assert_eq!(first[0], 0b0001_1111);
    assert_eq!(first[1..], [0; 511]);
    assert_eq!(read_block(&mut disk, 2), [0; BLOCK_SIZE]);
    assert_eq!(read_block(&mut disk, 3), [0; BLOCK_SIZE]);
    // The last bitmap block covers blocks 12,288 to 16,383, so its bits past
    // the end fill its last 256 bytes.
    let last = read_block(&mut disk, 4);
    assert_eq!(last[..256], [0; 256]);
    assert_eq!(last[256..], [0xFF; 256]);
    // The blocks after the bitmap are left as they were.
    for n in [5, 14_335] {
        assert_eq!(read_block(&mut disk, n), marker);
    }
}

// A RAM disk that logs each write (as its sector) and each flush (as None).
struct Logged {
    disk: RamDisk,
    log: Vec<Option<u64>>,
}

impl BlockDevice for Logged {
    fn sectors(&self) -> u64 {
        self.disk.sectors()
    }

    fn read(&mut self, sector: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        self.disk.read(sector, buffer)
    }

    fn write(&mut self, sector: u64, buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        self.log.push(Some(sector));
        self.disk.write(sector, buffer)
    }

    fn flush(&mut self) -> Result<(), &'static str> {
        self.log.push(None);
        Ok(())
    }
}

#[test]
fn format_writes_the_superblock_last_then_flushes() {
    let mut disk = Logged { disk: RamDisk::new(14_336), log: Vec::new() };
    format(&mut disk, TIME).unwrap();
    assert_eq!(disk.log, [Some(1), Some(2), Some(3), Some(4), Some(0), None]);
}

// A disk with more sectors than 32-bit block numbers can reach. It refuses
// every read, write and flush, so format() mustn't try any.
struct Huge;

impl BlockDevice for Huge {
    fn sectors(&self) -> u64 {
        1 << 32
    }

    fn read(&mut self, _: u64, _: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        Err("read")
    }

    fn write(&mut self, _: u64, _: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        Err("written")
    }

    fn flush(&mut self) -> Result<(), &'static str> {
        Err("flushed")
    }
}

#[test]
fn format_needs_a_disk_of_the_right_size() {
    assert_eq!(format(&mut Huge, TIME), Err("the disk is too big for LilyPad"));
    for sectors in [0, 1] {
        assert_eq!(format(&mut RamDisk::new(sectors), TIME), Err("the disk is too small for LilyPad"));
    }
    // Two blocks hold the superblock and the bitmap, with no room for files.
    let mut disk = RamDisk::new(2);
    format(&mut disk, TIME).unwrap();
    assert_eq!(Superblock::from_bytes(&read_block(&mut disk, 0)).unwrap().bitmap_blocks, 1);
    assert_eq!(read_block(&mut disk, 1), [0xFF; BLOCK_SIZE]);
}
