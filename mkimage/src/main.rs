// Packs a folder into a disk image's LilyPad.

use std::ffi::c_void;
use std::fmt::Display;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use std::process::ExitCode;
use std::ptr;

use storage::lilypad::{DIRECTORY, Entry, Packer, Time};
use storage::{BlockDevice, RamDisk, SECTOR_SIZE};

const FIRST_SECTOR: usize = 2048;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: mkimage <disk image> <folder>");
        return ExitCode::FAILURE;
    }
    match make(Path::new(&args[1]), Path::new(&args[2])) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mkimage: {}", error);
            ExitCode::FAILURE
        }
    }
}

// Replaces sector 2048 of `image` and everything after it with a LilyPad
// containing the entries in `folder`.
fn make(image: &Path, folder: &Path) -> Result<(), String> {
    let mut bytes = fs::read(image).map_err(|error| at(image, error))?;
    let sectors = (bytes.len() / SECTOR_SIZE).saturating_sub(FIRST_SECTOR);
    let mut disk = RamDisk::new(sectors as u64);
    let mut packer = Packer::new(&mut disk)?;
    let metadata = fs::metadata(folder).map_err(|error| at(folder, error))?;
    let directory = pack(&mut packer, folder)?;
    let mut root = Entry::root(changed(&metadata));
    root.first = packer.add(&directory).map_err(|error| at(folder, error))?;
    root.size = directory.len() as u64;
    packer.finish(root)?;
    let mut buffer = [0; SECTOR_SIZE];
    for sector in 0 ..sectors {
        disk.read(sector as u64, &mut buffer)?;
        let start = (FIRST_SECTOR + sector) * SECTOR_SIZE;
        bytes[start..start + SECTOR_SIZE].copy_from_slice(&buffer);
    }
    fs::write(image, bytes).map_err(|error| at(image, error))
}

// Packs each entry in `folder` and returns the containing directory.
fn pack(packer: &mut Packer, folder: &Path) -> Result<Vec<u8>, String> {
    let mut paths = Vec::new();
    for item in fs::read_dir(folder).map_err(|error| at(folder, error))? {
        paths.push(item.map_err(|error| at(folder, error))?.path());
    }
    paths.sort();
    let mut directory = Vec::new();
    for path in paths {
        let metadata = fs::metadata(&path).map_err(|error| at(&path, error))?;
        let (flags, contents) = if metadata.is_dir() {
            (DIRECTORY, pack(packer, &path)?)
        } else {
            (0, fs::read(&path).map_err(|error| at(&path, error))?)
        };
        let first = packer.add(&contents).map_err(|error| at(&path, error))?;
        let name = path.file_name().unwrap().to_string_lossy();
        let entry = Entry::new(&name, flags, first, contents.len() as u64, changed(&metadata)).map_err(|error| at(&path, error))?;
        directory.extend_from_slice(&entry.to_bytes());
    }
    Ok(directory)
}

// An error message indicating the file or folder it pertains to.
fn at(path: &Path, error: impl Display) -> String {
    format!("{}: {error}", path.display())
}

// A DateTime as structured by the Windows API.
#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn FileTimeToSystemTime(file_time: *const u64, time: *mut SystemTime) -> i32;
    fn SystemTimeToTzSpecificLocalTime(zone: *const c_void, utc: *const SystemTime, local: *mut SystemTime) -> i32;
}

// Local time of a file or folder's last change.
fn changed(metadata: &fs::Metadata) -> Time {
    let mut utc = SystemTime::default();
    let mut local = SystemTime::default();
    let converted = unsafe {
        FileTimeToSystemTime(&metadata.last_write_time(), &mut utc) != 0
            && SystemTimeToTzSpecificLocalTime(ptr::null(), &utc, &mut local) != 0
    };
    // The only failures would be for times thousands of years from now
    assert!(converted, "Windows couldn't convert a file's time");
    Time {
        year: local.year,
        month: local.month as u8,
        day: local.day as u8,
        hour: local.hour as u8,
        minute: local.minute as u8,
        second: local.second as u8,
    }
}
