use storage::lilypad::{
    BITS_PER_BLOCK, BLOCK_SIZE, Bitmap, DIRECTORY, ENTRY_SIZE, Entry, LilyPad, MAGIC, Packer, Superblock, Time,
    VERSION, format, valid_name,
};
use storage::{BlockDevice, Partition, RamDisk, SECTOR_SIZE};

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
    assert_eq!(format(&mut Huge, TIME), Err("the disk is too big for the LilyPad filesystem"));
    for sectors in [0, 1] {
        assert_eq!(format(&mut RamDisk::new(sectors), TIME), Err("the disk is too small for the LilyPad filesystem"));
    }
    // Two blocks hold the superblock and the bitmap, with no room for files.
    let mut disk = RamDisk::new(2);
    format(&mut disk, TIME).unwrap();
    assert_eq!(Superblock::from_bytes(&read_block(&mut disk, 0)).unwrap().bitmap_blocks, 1);
    assert_eq!(read_block(&mut disk, 1), [0xFF; BLOCK_SIZE]);
}

#[test]
fn packed_files_follow_one_another() {
    let mut disk = RamDisk::new(64);
    let mut packer = Packer::new(&mut disk).unwrap();
    // Block 0 is the superblock and block 1 the bitmap, so files start at 2.
    assert_eq!(packer.add(b"hello"), Ok(2));
    let long: Vec<u8> = (0..600).map(|i| i as u8).collect();
    assert_eq!(packer.add(&long), Ok(3));
    // An empty file has no blocks.
    assert_eq!(packer.add(b""), Ok(0));
    assert_eq!(packer.add(&[7; BLOCK_SIZE]), Ok(5));
    let mut root = Entry::root(TIME);
    root.first = 5;
    root.size = BLOCK_SIZE as u64;
    packer.finish(root).unwrap();
    let expected = Superblock { blocks: 64, bitmap_start: 1, bitmap_blocks: 1, root };
    assert_eq!(Superblock::from_bytes(&read_block(&mut disk, 0)), Ok(expected));
    // The rest of a file's last block is zeros.
    let hello = read_block(&mut disk, 2);
    assert_eq!((&hello[..5], &hello[5..]), (&b"hello"[..], &[0; 507][..]));
    assert_eq!(read_block(&mut disk, 3)[..], long[..512]);
    let end = read_block(&mut disk, 4);
    assert_eq!((&end[..88], &end[88..]), (&long[512..], &[0; 424][..]));
    assert_eq!(read_block(&mut disk, 5), [7; BLOCK_SIZE]);
    assert_eq!(read_block(&mut disk, 6), [0; BLOCK_SIZE]);
    // Blocks 0 to 5 are used, and those from 64 on are past the end.
    let bitmap = read_block(&mut disk, 1);
    assert_eq!(bitmap[..8], [0b0011_1111, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(bitmap[8..], [0xFF; 504]);
}

#[test]
fn a_full_lilypad_takes_no_more() {
    // 4 blocks: the superblock, the bitmap and room for 2 blocks of files.
    let mut disk = Logged { disk: RamDisk::new(4), log: Vec::new() };
    let mut packer = Packer::new(&mut disk).unwrap();
    // A file a byte too big is refused before any of it is written.
    assert_eq!(packer.add(&[1; 2 * BLOCK_SIZE + 1]), Err("LilyPad is full"));
    assert_eq!(packer.add(&[2; 2 * BLOCK_SIZE]), Ok(2));
    assert_eq!(packer.add(&[3]), Err("LilyPad is full"));
    // An empty file still fits.
    assert_eq!(packer.add(b""), Ok(0));
    packer.finish(Entry::root(TIME)).unwrap();
    // The files go first, then the bitmap and the superblock.
    assert_eq!(disk.log, [Some(2), Some(3), Some(1), Some(0), None]);
    assert_eq!(read_block(&mut disk.disk, 3), [2; BLOCK_SIZE]);
    assert_eq!(read_block(&mut disk.disk, 1), [0xFF; BLOCK_SIZE]);
}

#[test]
fn a_formatted_disk_mounts() {
    let mut disk = RamDisk::new(100);
    format(&mut disk, TIME).unwrap();
    let mut lilypad = LilyPad::mount(disk).unwrap();
    let expected = Superblock { blocks: 100, bitmap_start: 1, bitmap_blocks: 1, root: Entry::root(TIME) };
    assert_eq!(*lilypad.superblock(), expected);
    // Only the superblock and the bitmap are used.
    assert_eq!(lilypad.used(), Ok(2));
}

#[test]
fn lilypad_mounts_after_the_first_megabyte() {
    // Like the boot disk: LilyPad starts at sector 2048.
    let mut partition = Partition::new(RamDisk::new(2048 + 14_336), 2048);
    format(&mut partition, TIME).unwrap();
    let mut lilypad = LilyPad::mount(partition).unwrap();
    assert_eq!((lilypad.superblock().blocks, lilypad.superblock().bitmap_blocks), (14_336, 4));
    assert_eq!(lilypad.used(), Ok(5));
}

#[test]
fn used_counts_what_the_bitmap_marks() {
    // 5,000 blocks need 2 bitmap blocks, and 3,192 bits past the end are set.
    let mut disk = RamDisk::new(5000);
    let mut packer = Packer::new(&mut disk).unwrap();
    packer.add(&[1; 3 * BLOCK_SIZE]).unwrap();
    packer.finish(Entry::root(TIME)).unwrap();
    let mut lilypad = LilyPad::mount(disk).unwrap();
    // The superblock, 2 bitmap blocks and the 3-block file. The bits past the
    // end don't count.
    assert_eq!(lilypad.used(), Ok(6));
    // In a full LilyPad, the last block counts too.
    let mut disk = RamDisk::new(4);
    let mut packer = Packer::new(&mut disk).unwrap();
    packer.add(&[1; 2 * BLOCK_SIZE]).unwrap();
    packer.finish(Entry::root(TIME)).unwrap();
    assert_eq!(LilyPad::mount(disk).unwrap().used(), Ok(4));
}

#[test]
fn only_a_superblock_that_fits_its_disk_mounts() {
    // A disk of `sectors` sectors with `superblock` in sector 0, mounted.
    let mount = |superblock: Superblock, sectors: u64| {
        let mut disk = RamDisk::new(sectors);
        disk.write(0, &superblock.to_bytes()).unwrap();
        LilyPad::mount(disk).err()
    };
    let fits = Superblock { blocks: 100, bitmap_start: 1, bitmap_blocks: 1, root: Entry::root(TIME) };
    assert_eq!(mount(fits, 100), None);
    // A LilyPad may be smaller than its disk, but not bigger.
    assert_eq!(mount(fits, 101), None);
    assert_eq!(mount(fits, 99), Some("LilyPad is bigger than its assigned disk"));
    assert_eq!(LilyPad::mount(RamDisk::new(100)).err(), Some("the disk isn't formatted with LilyPad"));
    assert_eq!(LilyPad::mount(RamDisk::new(0)).err(), Some("the sector is past the end of the disk"));
    // The bitmap must start at block 1 and have a bit for every block.
    let misplaced = Some("the superblock's bitmap doesn't fit LilyPad");
    assert_eq!(mount(Superblock { bitmap_start: 2, ..fits }, 100), misplaced);
    assert_eq!(mount(Superblock { bitmap_blocks: 2, ..fits }, 100), misplaced);
    assert_eq!(mount(Superblock { blocks: 4097, ..fits }, 5000), misplaced);
    assert_eq!(mount(Superblock { blocks: 4097, bitmap_blocks: 2, ..fits }, 5000), None);
    // One block holds the superblock, so the bitmap has no room.
    assert_eq!(mount(Superblock { blocks: 1, ..fits }, 100), misplaced);
    let file = Entry::new("root", 0, 0, 0, TIME).unwrap();
    assert_eq!(mount(Superblock { root: file, ..fits }, 100), Some("the root isn't a directory"));
}

// A LilyPad holding:
//   /README.TXT       "Welcome!"
//   /big.bin          1,500 bytes, over 3 blocks
//   /docs/            two files, with a free entry between them
//   /docs/hello.txt   "Hello from LilyPad!"
//   /docs/empty.txt   nothing
fn sample_lilypad() -> LilyPad<RamDisk> {
    let mut disk = RamDisk::new(64);
    let mut packer = Packer::new(&mut disk).unwrap();
    let file = |name: &str, first: u32, size: usize| Entry::new(name, 0, first, size as u64, TIME).unwrap();
    let readme = packer.add(b"Welcome!").unwrap();
    let big = packer.add(&big_contents()).unwrap();
    let hello = packer.add(b"Hello from LilyPad!").unwrap();
    let mut docs = Vec::new();
    docs.extend_from_slice(&file("hello.txt", hello, 19).to_bytes());
    docs.extend_from_slice(&[0; ENTRY_SIZE]);
    docs.extend_from_slice(&file("empty.txt", 0, 0).to_bytes());
    let docs_first = packer.add(&docs).unwrap();
    let mut top = Vec::new();
    top.extend_from_slice(&file("README.TXT", readme, 8).to_bytes());
    top.extend_from_slice(&file("big.bin", big, 1500).to_bytes());
    top.extend_from_slice(&Entry::new("docs", DIRECTORY, docs_first, docs.len() as u64, TIME).unwrap().to_bytes());
    let mut root = Entry::root(TIME);
    root.first = packer.add(&top).unwrap();
    root.size = top.len() as u64;
    packer.finish(root).unwrap();
    LilyPad::mount(disk).unwrap()
}

fn big_contents() -> Vec<u8> {
    (0..1500).map(|i| (i % 251) as u8).collect()
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.name()).collect()
}

#[test]
fn files_read_back_whole() {
    let mut lilypad = sample_lilypad();
    let readme = lilypad.find("/README.TXT").unwrap();
    assert_eq!(lilypad.read(&readme), Ok(b"Welcome!".to_vec()));
    // Its last block is cut at the file's size.
    let big = lilypad.find("/big.bin").unwrap();
    assert_eq!(lilypad.read(&big), Ok(big_contents()));
    // An empty file has no blocks to read.
    let empty = lilypad.find("/docs/empty.txt").unwrap();
    assert_eq!(lilypad.read(&empty), Ok(Vec::new()));
}

#[test]
fn entries_outside_lilypad_are_not_read() {
    // 64 blocks: the superblock, the bitmap, then 62 for files.
    let mut lilypad = sample_lilypad();
    let file = |first: u32, size: u64| Entry::new("x", 0, first, size, TIME).unwrap();
    let outside = Err("the files blocks are outside the LilyPad-managed file structure");
    assert_eq!(lilypad.read(&file(0, 10)), outside);
    assert_eq!(lilypad.read(&file(1, 10)), outside);
    // Block 63 is the last one, so a 2-block file can't start there.
    assert_eq!(lilypad.read(&file(63, 513)), outside);
    assert_eq!(lilypad.read(&file(63, 512)), Ok(vec![0; 512]));
    assert_eq!(lilypad.read(&file(2, 0)), Ok(Vec::new()));
}

#[test]
fn directories_list_their_entries() {
    let mut lilypad = sample_lilypad();
    let root = lilypad.superblock().root;
    assert_eq!(names(&lilypad.list(&root).unwrap()), ["README.TXT", "big.bin", "docs"]);
    // The free entry between the two files is left out.
    let docs = lilypad.find("/docs").unwrap();
    assert_eq!(names(&lilypad.list(&docs).unwrap()), ["hello.txt", "empty.txt"]);
    let readme = lilypad.find("/README.TXT").unwrap();
    assert_eq!(lilypad.list(&readme), Err("not a directory"));
}

#[test]
fn paths_find_files_and_directories() {
    let mut lilypad = sample_lilypad();
    let root = lilypad.superblock().root;
    assert_eq!(lilypad.find("/"), Ok(root));
    let hello = lilypad.find("/docs/hello.txt").unwrap();
    assert_eq!((hello.name(), hello.size), ("hello.txt", 19));
    assert!(lilypad.find("/docs").unwrap().is_directory());
    // Names ignore case, and extra slashes don't count.
    for path in ["/DOCS/Hello.TXT", "//docs//hello.txt", "/docs/hello.txt/"] {
        assert_eq!(lilypad.find(path), Ok(hello), "{path}");
    }
    // `.` stays put and `..` goes up a level, worked out from the path
    // alone, and `..` at the root stays there.
    for path in ["/docs/./hello.txt", "/docs/../docs/hello.txt", "/../docs/hello.txt", "/docs/hello.txt/../hello.txt"] {
        assert_eq!(lilypad.find(path), Ok(hello), "{path}");
    }
    assert_eq!(lilypad.find("/docs/.."), Ok(root));
    assert_eq!(lilypad.find("/.."), Ok(root));
}

#[test]
fn bad_paths_are_refused() {
    let mut lilypad = sample_lilypad();
    assert_eq!(lilypad.find("docs/hello.txt"), Err("a path must start with '/'"));
    assert_eq!(lilypad.find(""), Err("a path must start with '/'"));
    let missing = Err("no such file or directory");
    assert_eq!(lilypad.find("/nothing"), missing);
    assert_eq!(lilypad.find("/docs/nothing.txt"), missing);
    assert_eq!(lilypad.find("/docs/hello"), missing);
    // A path can't go on through a file.
    assert_eq!(lilypad.find("/README.TXT/x"), Err("not a directory"));
    assert_eq!(lilypad.find("/docs/hello.txt/more"), Err("not a directory"));
}
