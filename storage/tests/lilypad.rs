use storage::lilypad::{DIRECTORY, ENTRY_SIZE, Entry, Time, valid_name};

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
