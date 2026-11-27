// Custom filesystem for FroggeOS. (See FILESYSTEM.md)

pub const ENTRY_SIZE: usize = 64;
pub const NAME_SIZE: usize = 32;

// Flag bit set in a directory's entry.
pub const DIRECTORY: u32 = 1 << 0;

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

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
