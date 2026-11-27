// The boot disk's filesystem. (See FILESYSTEM.md)

use storage::Partition;
use storage::lilypad::LilyPad;
use crate::ata::{self, Ata};
use crate::lock::SpinLock;
use crate::println;

pub const FIRST_SECTOR: u64 = 2048;

// The boot disk's mounted LilyPad filesystem instance.
pub static LILYPAD: SpinLock<Option<LilyPad<Partition<Ata>>>> = SpinLock::new(None);

// Mounts the boot disk's LilyPad instance.
pub fn mount() {
    match mount_boot_disk() { 
        Ok((used, blocks)) => println!("LilyPad mounted: {} of {} blocks used", used, blocks),
        Err(error) => println!("LilyPad was not mounted: {}", error),
    }
}

fn mount_boot_disk() -> Result<(u32, u32), &'static str> {
    let disk = ata::identify().ok_or("no ATA disk found")?;
    let mut lilypad = LilyPad::mount(Partition::new(disk, FIRST_SECTOR))?;
    let used = lilypad.used()?;
    let blocks = lilypad.superblock().blocks;
    *LILYPAD.lock() = Some(lilypad);
    Ok((used, blocks))
}
