use storage::{BlockDevice, Partition, RamDisk, SECTOR_SIZE};

const PAST_END: &str = "the sector is past the end of the disk";

// A RAM disk whose sector n is filled with the byte n.
fn numbered(sectors: u64) -> RamDisk {
    let mut disk = RamDisk::new(sectors);
    for n in 0..sectors {
        disk.write(n, &[n as u8; SECTOR_SIZE]).unwrap();
    }
    disk
}

fn read(disk: &mut dyn BlockDevice, n: u64) -> Result<[u8; SECTOR_SIZE], &'static str> {
    let mut buffer = [0; SECTOR_SIZE];
    disk.read(n, &mut buffer).map(|()| buffer)
}

#[test]
fn a_partition_starts_at_its_first_sector() {
    let mut partition = Partition::new(numbered(10), 4);
    assert_eq!(partition.sectors(), 6);
    for n in 0..6 {
        assert_eq!(read(&mut partition, n), Ok([4 + n as u8; SECTOR_SIZE]));
    }
    // Its sector 6 would be the disk's sector 10, past the disk's end.
    assert_eq!(read(&mut partition, 6), Err(PAST_END));
    assert_eq!(read(&mut partition, u64::MAX), Err(PAST_END));
}

#[test]
fn writes_land_where_reads_look() {
    let mut partition = Partition::new(numbered(10), 4);
    partition.write(2, &[0xEE; SECTOR_SIZE]).unwrap();
    assert_eq!(read(&mut partition, 2), Ok([0xEE; SECTOR_SIZE]));
    // The sectors on either side keep the disk's sectors 5 and 7.
    assert_eq!(read(&mut partition, 1), Ok([5; SECTOR_SIZE]));
    assert_eq!(read(&mut partition, 3), Ok([7; SECTOR_SIZE]));
    assert_eq!(partition.write(6, &[0; SECTOR_SIZE]), Err(PAST_END));
    assert_eq!(partition.write(u64::MAX, &[0; SECTOR_SIZE]), Err(PAST_END));
}

// A RAM disk that can't flush, to show a partition passes flushes on.
struct Unflushable(RamDisk);

impl BlockDevice for Unflushable {
    fn sectors(&self) -> u64 {
        self.0.sectors()
    }

    fn read(&mut self, sector: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        self.0.read(sector, buffer)
    }

    fn write(&mut self, sector: u64, buffer: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
        self.0.write(sector, buffer)
    }

    fn flush(&mut self) -> Result<(), &'static str> {
        Err("not flushed")
    }
}

#[test]
fn flushes_reach_the_disk() {
    let mut partition = Partition::new(Unflushable(RamDisk::new(4)), 1);
    assert_eq!(partition.flush(), Err("not flushed"));
}

#[test]
fn a_partition_from_the_end_on_is_empty() {
    for first in [4, 5, u64::MAX] {
        let mut partition = Partition::new(numbered(4), first);
        assert_eq!(partition.sectors(), 0);
        assert_eq!(read(&mut partition, 0), Err(PAST_END));
    }
    // From sector 0 on, a partition is the whole disk.
    let mut whole = Partition::new(numbered(4), 0);
    assert_eq!(whole.sectors(), 4);
    assert_eq!(read(&mut whole, 3), Ok([3; SECTOR_SIZE]));
}
