// The boot disk's filesystem. (See FILESYSTEM.md)

use alloc::string::String;
use alloc::vec;
use storage::Partition;
use storage::lilypad::{LilyPad, join};
use crate::ata::{self, Ata};
use crate::lock::SpinLock;
use crate::{print, println};

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

// Runs `action` on the mounted LilyPad.
fn with_lilypad<T>(action: impl FnOnce(&mut LilyPad<Partition<Ata>>) -> Result<T, &'static str>) -> Result<T, &'static str> {
    let mut mounted = LILYPAD.lock();
    action(mounted.as_mut().ok_or("LilyPad isn't mounted")?)
}

// Lists the directory at `path` or shows the files there.
pub fn ls(cwd: &str, path: &str) -> Result<(), &'static str> {
    let entries = with_lilypad(|lilypad| {
        let entry = lilypad.find(&join(cwd, path))?;
        if entry.is_directory() { lilypad.list(&entry) } else { Ok(vec![entry]) }
    })?;
    for entry in entries {
        let mut name = String::from(entry.name());
        if entry.is_directory() {
            name.push('/');
        }
        let time = entry.time;
        println!("{:<14}{:>8}  {}-{:02}-{:02} {:02}:{:02}", name, entry.size, time.year, time.month, time.day, time.hour, time.minute);
    }
    Ok(())
}

// Prints the file at `path`
pub fn cat(cwd: &str, path: &str) -> Result<(), &'static str> {
    let contents = with_lilypad(|lilypad| {
        let entry = lilypad.find(&join(cwd, path))?;
        if entry.is_directory() {
            return Err("is a directory");
        }
        lilypad.read(&entry)
    })?;
    let text: String = contents.iter().filter(|&&byte| byte != b'\r').map(|&byte| if byte.is_ascii() { byte as char } else { '\x7F' }).collect();
    print!("{}", text);
    if !text.is_empty() && !text.ends_with('\n') {
        println!();
    }
    Ok(())
}

// Makes the directory at `path` the current working directory.
pub fn cd(cwd: &mut String, path: &str) -> Result<(), &'static str> {
    let (spelled, entry) = with_lilypad(|lilypad| lilypad.resolve(&join(cwd, path)))?;
    if !entry.is_directory() {
        return Err("not a directory");
    }
    *cwd = spelled;
    Ok(())
}
