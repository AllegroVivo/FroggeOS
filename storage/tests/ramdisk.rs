use storage::{BlockDevice, RamDisk, SECTOR_SIZE};

// A whole sector of `byte`.
fn filled(byte: u8) -> [u8; SECTOR_SIZE] {
    [byte; SECTOR_SIZE]
}

// A sector whose every byte depends on both the sector and its place in it,
// so a mix-up between two sectors, or inside one, would show.
fn pattern(sector: u64) -> [u8; SECTOR_SIZE] {
    let mut data = [0; SECTOR_SIZE];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte = (sector as usize * 31 + i) as u8;
    }
    data
}

#[test]
fn starts_as_zeros() {
    let mut disk = RamDisk::new(8);
    assert_eq!(disk.sectors(), 8);
    for sector in 0..8 {
        let mut buffer = filled(0xAA);
        disk.read(sector, &mut buffer).unwrap();
        assert_eq!(buffer, filled(0));
    }
}

#[test]
fn reads_back_what_was_written() {
    let mut disk = RamDisk::new(16);
    for sector in 0..16 {
        disk.write(sector, &pattern(sector)).unwrap();
    }
    // Read them back in the other order.
    for sector in (0..16).rev() {
        let mut buffer = filled(0);
        disk.read(sector, &mut buffer).unwrap();
        assert_eq!(buffer, pattern(sector));
    }
}

#[test]
fn a_write_leaves_its_neighbours_alone() {
    let mut disk = RamDisk::new(3);
    disk.write(1, &filled(0xFF)).unwrap();
    let mut buffer = filled(0x55);
    for (sector, byte) in [(0, 0), (1, 0xFF), (2, 0)] {
        disk.read(sector, &mut buffer).unwrap();
        assert_eq!(buffer, filled(byte));
    }
    // Writing again replaces the whole sector.
    disk.write(1, &pattern(1)).unwrap();
    disk.read(1, &mut buffer).unwrap();
    assert_eq!(buffer, pattern(1));
}

#[test]
fn sectors_past_the_end() {
    let past_end = Err("the sector is past the end of the disk");
    let mut disk = RamDisk::new(4);
    let mut buffer = filled(0x55);
    assert_eq!(disk.read(4, &mut buffer), past_end);
    assert_eq!(disk.write(4, &filled(1)), past_end);
    assert_eq!(disk.read(u64::MAX, &mut buffer), past_end);
    // A refused read leaves the buffer as it was.
    assert_eq!(buffer, filled(0x55));
    // The last sector still works.
    disk.write(3, &filled(3)).unwrap();
    disk.read(3, &mut buffer).unwrap();
    assert_eq!(buffer, filled(3));
    // A disk with no sectors refuses everything.
    let mut empty = RamDisk::new(0);
    assert_eq!(empty.sectors(), 0);
    assert_eq!(empty.read(0, &mut buffer), past_end);
}

#[test]
fn through_the_trait() {
    // The filesystem will only see a `dyn BlockDevice`, never the RamDisk.
    let mut ram = RamDisk::new(2);
    let disk: &mut dyn BlockDevice = &mut ram;
    disk.write(1, &pattern(1)).unwrap();
    let mut buffer = filled(0);
    disk.read(1, &mut buffer).unwrap();
    assert_eq!((disk.sectors(), buffer), (2, pattern(1)));
}
