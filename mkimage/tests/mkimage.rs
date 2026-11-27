// Runs mkimage on folders and disk images made for each test, then reads the
// LilyPad it made with the storage crate.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use storage::lilypad::{BLOCK_SIZE, DIRECTORY, ENTRY_SIZE, Entry, Superblock, Time};

const MB: usize = 1024 * 1024;

const NAME_RULES: &str = "a name is 1 to 32 printable ASCII characters, without spaces or '/'";

// A new, empty folder for one test, in Cargo's target folder.
fn scratch(test: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(&path).unwrap();
    path
}

// Runs mkimage, and returns whether it worked and the errors it printed.
fn mkimage(args: &[&Path]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_mkimage")).args(args).output().unwrap();
    assert!(output.stdout.is_empty());
    (output.status.success(), String::from_utf8(output.stderr).unwrap())
}

// A disk image of `size` bytes, all 0xAB, so the tests can see what mkimage
// changes.
fn image(dir: &Path, size: usize) -> PathBuf {
    let path = dir.join("disk.img");
    fs::write(&path, vec![0xAB; size]).unwrap();
    path
}

fn untouched(image: &Path, size: usize) -> bool {
    fs::read(image).unwrap() == vec![0xAB; size]
}

// Block `n` of the LilyPad in `disk`, which starts at 1 MB.
fn block(disk: &[u8], n: u32) -> &[u8; BLOCK_SIZE] {
    let start = MB + n as usize * BLOCK_SIZE;
    disk[start..start + BLOCK_SIZE].try_into().unwrap()
}

fn superblock(disk: &[u8]) -> Superblock {
    Superblock::from_bytes(block(disk, 0)).unwrap()
}

// What a file or directory holds. The rest of its last block must be zeros.
fn contents<'a>(disk: &'a [u8], entry: &Entry) -> &'a [u8] {
    let start = MB + entry.first as usize * BLOCK_SIZE;
    let size = entry.size as usize;
    let end = start + size.div_ceil(BLOCK_SIZE) * BLOCK_SIZE;
    assert!(disk[start + size..end].iter().all(|&byte| byte == 0), "{} isn't padded with zeros", entry.name());
    &disk[start..start + size]
}

// The entries in a directory.
fn entries(disk: &[u8], directory: &Entry) -> Vec<Entry> {
    assert!(directory.is_directory());
    let bytes = contents(disk, directory);
    bytes.chunks(ENTRY_SIZE).map(|entry| Entry::from_bytes(entry.try_into().unwrap()).unwrap()).collect()
}

// When each path last changed, in local time, as PowerShell (.NET) sees it.
// That's a separate way from the Windows calls mkimage makes.
fn local_times(paths: &[PathBuf]) -> Vec<Time> {
    let quoted: Vec<String> = paths.iter().map(|path| format!("'{}'", path.display())).collect();
    let script = format!(
        "foreach ($path in {}) {{ (Get-Item -LiteralPath $path).LastWriteTime.ToString('yyyy M d H m s') }}",
        quoted.join(", ")
    );
    let output = Command::new("powershell").args(["-NoProfile", "-Command", &script]).output().unwrap();
    let times: Vec<Time> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            let n: Vec<u16> = line.split(' ').map(|number| number.parse().unwrap()).collect();
            Time { year: n[0], month: n[1] as u8, day: n[2] as u8, hour: n[3] as u8, minute: n[4] as u8, second: n[5] as u8 }
        })
        .collect();
    assert_eq!(times.len(), paths.len());
    times
}

// Sets when `path` last changed: `seconds` after 1970 began, in UTC.
fn set_changed(path: &Path, seconds: u64) {
    let file = File::options().write(true).open(path).unwrap();
    file.set_modified(UNIX_EPOCH + Duration::from_secs(seconds)).unwrap();
}

#[test]
fn a_folder_is_packed_with_everything_in_it() {
    let dir = scratch("packed");
    let files = dir.join("files");
    let docs = files.join("docs");
    fs::create_dir_all(&docs).unwrap();
    fs::create_dir(files.join("empty_folder")).unwrap();
    fs::write(files.join("README.TXT"), b"Welcome!\r\n").unwrap();
    let big: Vec<u8> = (0..1500).map(|i| (i % 251) as u8).collect();
    fs::write(files.join("big.bin"), &big).unwrap();
    fs::write(docs.join("hello.txt"), b"Hello from LilyPad!").unwrap();
    fs::write(docs.join("empty.txt"), b"").unwrap();
    // 2026-01-15 and 2026-07-15 at 12:34:56 UTC, in case daylight saving time
    // changes the local times.
    set_changed(&files.join("big.bin"), 1_768_480_496);
    set_changed(&docs.join("hello.txt"), 1_784_118_896);
    let image = image(&dir, 8 * MB);
    assert_eq!(mkimage(&[&image, &files]), (true, String::new()));

    let disk = fs::read(&image).unwrap();
    assert_eq!(disk.len(), 8 * MB);
    // The first megabyte, the boot sectors and the kernel's area, is left alone.
    assert!(disk[..MB].iter().all(|&byte| byte == 0xAB));
    let superblock = superblock(&disk);
    assert_eq!((superblock.blocks, superblock.bitmap_start, superblock.bitmap_blocks), (14_336, 1, 4));
    let root = superblock.root;
    assert_eq!(root.name(), "/");

    // Files are packed from block 5 on, in order of name. A folder's own
    // entries come after everything in it, and an empty file or folder has no
    // blocks.
    let top = entries(&disk, &root);
    let names: Vec<&str> = top.iter().map(|entry| entry.name()).collect();
    assert_eq!(names, ["README.TXT", "big.bin", "docs", "empty_folder"]);
    let inner = entries(&disk, &top[2]);
    let names: Vec<&str> = inner.iter().map(|entry| entry.name()).collect();
    assert_eq!(names, ["empty.txt", "hello.txt"]);
    let layout = |entry: &Entry| (entry.flags, entry.first, entry.size);
    assert_eq!(layout(&top[0]), (0, 5, 10));
    assert_eq!(layout(&top[1]), (0, 6, 1500));
    assert_eq!(layout(&inner[0]), (0, 0, 0));
    assert_eq!(layout(&inner[1]), (0, 9, 19));
    assert_eq!(layout(&top[2]), (DIRECTORY, 10, 2 * ENTRY_SIZE as u64));
    assert_eq!(layout(&top[3]), (DIRECTORY, 0, 0));
    assert_eq!(layout(&root), (DIRECTORY, 11, 4 * ENTRY_SIZE as u64));
    assert_eq!(contents(&disk, &top[0]), b"Welcome!\r\n");
    assert_eq!(contents(&disk, &top[1]), big);
    assert_eq!(contents(&disk, &inner[1]), b"Hello from LilyPad!");

    // Blocks 0 to 11 are used, and the bits past block 14,335 are set.
    let bitmap: Vec<u8> = (1..=4).flat_map(|n| *block(&disk, n)).collect();
    assert_eq!(bitmap[..2], [0xFF, 0x0F]);
    assert!(bitmap[2..1792].iter().all(|&byte| byte == 0));
    assert!(bitmap[1792..].iter().all(|&byte| byte == 0xFF));
    // The rest of LilyPad is zeros, whatever the image held before.
    assert_eq!(*block(&disk, 12), [0; BLOCK_SIZE]);
    assert_eq!(*block(&disk, 14_335), [0; BLOCK_SIZE]);

    // Every time is when that file or folder last changed, in local time.
    let paths = [
        files.join("README.TXT"),
        files.join("big.bin"),
        docs.clone(),
        files.join("empty_folder"),
        docs.join("empty.txt"),
        docs.join("hello.txt"),
        files.clone(),
    ];
    let times = [top[0].time, top[1].time, top[2].time, top[3].time, inner[0].time, inner[1].time, root.time];
    assert_eq!(times.to_vec(), local_times(&paths));
}

#[test]
fn names_lilypad_cannot_hold_are_refused() {
    let too_long = "x".repeat(33);
    for (test, name, folder) in [
        ("space", "two words.txt", false),
        ("long", too_long.as_str(), false),
        ("accent", "caf\u{e9}.txt", false),
        ("folder", "my docs", true),
    ] {
        let dir = scratch(&format!("name_{test}"));
        let files = dir.join("files");
        let bad = files.join(name);
        if folder {
            fs::create_dir_all(&bad).unwrap();
            fs::write(bad.join("fine.txt"), b"fine").unwrap();
        } else {
            fs::create_dir(&files).unwrap();
            fs::write(&bad, b"x").unwrap();
        }
        let image = image(&dir, 8 * MB);
        let refused = format!("mkimage: {}: {NAME_RULES}\n", bad.display());
        assert_eq!(mkimage(&[&image, &files]), (false, refused));
        // When mkimage fails, it doesn't change the image.
        assert!(untouched(&image, 8 * MB), "{name}");
    }
}

#[test]
fn files_that_do_not_fit_are_refused() {
    // 4 blocks of LilyPad: the superblock, the bitmap and 2 blocks for files.
    let size = MB + 4 * BLOCK_SIZE;
    let dir = scratch("full");
    let files = dir.join("files");
    fs::create_dir(&files).unwrap();
    let big = files.join("big.bin");
    let image = image(&dir, size);
    fs::write(&big, [1; 2 * BLOCK_SIZE + 1]).unwrap();
    assert_eq!(mkimage(&[&image, &files]), (false, format!("mkimage: {}: LilyPad is full\n", big.display())));
    // A 2-block file fits, but then the root directory's entry for it doesn't.
    fs::write(&big, [1; 2 * BLOCK_SIZE]).unwrap();
    assert_eq!(mkimage(&[&image, &files]), (false, format!("mkimage: {}: LilyPad is full\n", files.display())));
    assert!(untouched(&image, size));
    // A 1-block file and the root directory fill LilyPad exactly.
    fs::write(&big, [1; BLOCK_SIZE]).unwrap();
    assert_eq!(mkimage(&[&image, &files]), (true, String::new()));
    let disk = fs::read(&image).unwrap();
    let root = superblock(&disk).root;
    assert_eq!((root.first, root.size), (3, ENTRY_SIZE as u64));
    assert_eq!(*block(&disk, 2), [1; BLOCK_SIZE]);
    assert_eq!(*block(&disk, 1), [0xFF; BLOCK_SIZE]);
}

#[test]
fn an_image_needs_room_for_lilypad() {
    let dir = scratch("small");
    let files = dir.join("files");
    fs::create_dir(&files).unwrap();
    // LilyPad needs at least 2 blocks after the first megabyte: the superblock
    // and the bitmap.
    let too_small = (false, "mkimage: the disk is too small for the LilyPad filesystem\n".to_string());
    for size in [0, MB, MB + BLOCK_SIZE] {
        let image = image(&dir, size);
        assert_eq!(mkimage(&[&image, &files]), too_small);
        assert!(untouched(&image, size));
    }
    let image = image(&dir, MB + 2 * BLOCK_SIZE);
    assert_eq!(mkimage(&[&image, &files]), (true, String::new()));
    // An empty folder's directory has no entries, so it takes no blocks.
    let disk = fs::read(&image).unwrap();
    let superblock = superblock(&disk);
    assert_eq!((superblock.blocks, superblock.root.first, superblock.root.size), (2, 0, 0));
}

#[test]
fn mkimage_needs_an_image_and_a_folder() {
    let usage = (false, "usage: mkimage <disk image> <folder>\n".to_string());
    let dir = scratch("usage");
    assert_eq!(mkimage(&[]), usage);
    assert_eq!(mkimage(&[&dir]), usage);
    assert_eq!(mkimage(&[&dir, &dir, &dir]), usage);
}

#[test]
fn missing_files_are_reported() {
    let dir = scratch("missing");
    let files = dir.join("files");
    fs::create_dir(&files).unwrap();
    let no_image = dir.join("no.img");
    let (worked, errors) = mkimage(&[&no_image, &files]);
    assert!(!worked && errors.starts_with(&format!("mkimage: {}: ", no_image.display())), "{errors}");
    let image = image(&dir, 8 * MB);
    let no_folder = dir.join("no_folder");
    let (worked, errors) = mkimage(&[&image, &no_folder]);
    assert!(!worked && errors.starts_with(&format!("mkimage: {}: ", no_folder.display())), "{errors}");
    assert!(untouched(&image, 8 * MB));
}
