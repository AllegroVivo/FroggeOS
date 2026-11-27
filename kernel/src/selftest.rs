// Unit testing for the kernel.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use core::array;
use core::slice;
use ribbit::calc;
use storage::{BlockDevice, RamDisk, SECTOR_SIZE};
use crate::{ata, boot, frames, fs, heap, paging, rtc, shell, timer};
use crate::{print, println};

// A check returns Ok or Err.
type Check = fn() -> Result<(), &'static str>;

const CHECKS: [(&str, Check); 13] = [
    ("heap",    check_heap),
    ("growth",  check_growth),
    ("frames",  check_frames),
    ("paging",  check_paging),
    ("spaces",  check_spaces),
    ("ram",     check_ram),
    ("apic",    check_apic),
    ("timer",   check_timer),
    ("clock",   check_clock),
    ("Ribbit",  check_ribbit),
    ("ramdisk", check_ramdisk),
    ("ata",     check_ata),
    ("lilypad", check_lilypad),
];

pub fn run() -> bool {
    let mut failed = 0;
    for (name, check) in CHECKS {
        print!("{} ... ", name);
        match check() {
            Ok(()) => println!("PASS"),
            Err(why) => {
                println!("FAIL: {}", why);
                failed += 1;
            },
        }
    }
    println!("{} passed, {} failed", CHECKS.len() - failed, failed);
    failed == 0
}

// Closes QEMU thru the isa-debug-exit device.
#[cfg(feature = "selftest")]
pub fn exit_qemu(passed: bool) {
    let value = if passed { 0x10 } else { 0x11 };
    unsafe { crate::port::outb(0xF4, value) };
}

// Grows a Vec and joins Strings plus checks memory releases.
fn check_heap() -> Result<(), &'static str> {
    let (used, _, allocations) = heap::stats();
    {
        let mut numbers: Vec<u64> = Vec::new();
        for n in 1..=1000 {
            numbers.push(n);
        }
        let name = String::from("FroggeOS") + " and Ribbit";
        if numbers.iter().sum::<u64>() != 500_500 || name.len() != 19 {
            return Err("a Vec or String holds the wrong values");
        }
    }
    let (used_after, _, allocations_after) = heap::stats();
    if used_after != used || allocations_after != allocations {
        return Err("memory was not deallocated");
    }
    Ok(())
}

// Allocates 2 MB and checks that the heap grew and has correct block indices.
fn check_growth() -> Result<(), &'static str> {
    const WORDS: usize = 1 << 18; // 2 MB of u64s
    let words: Vec<u64> = (0..WORDS as u64).collect();
    if heap::size() < WORDS * 8 {
        return Err("the heap didn't grow");
    }
    if words.iter().enumerate().any(|(i, &word)| word != i as u64) {
        return Err("2 MB Vec holds incorrect values");
    }
    Ok(())
}

fn check_frames() -> Result<(), &'static str> {
    let free = frames::free_count();
    let mut taken = [0; 3];
    for (i, frame) in taken.iter_mut().enumerate() {
        *frame = frames::alloc().ok_or("no frame was free")?;
        // 512 words of 8 bytes - the whole frame.
        let words = unsafe { slice::from_raw_parts_mut(*frame as *mut u64, 512) };
        words.fill(i as u64);
    }
    if frames::free_count() != free - 3 {
        return Err("taking 3 frames didn't lower the free count by 3");
    }
    for (i, frame) in taken.into_iter().enumerate() {
        let words = unsafe { slice::from_raw_parts(frame as *const u64, 512) };
        if frame < frames::RESERVED || words.iter().any(|&word| word != i as u64) {
            return Err("a frame was in the first 2 MB, or two were the same");
        }
        frames::free(frame);
    }
    // alloc() takes the lowest free frame and gets the first one back.
    if frames::free_count() != free || frames::alloc() != Some(taken[0]) {
        return Err("the frames weren't all given back");
    }
    frames::free(taken[0]);
    Ok(())
}

// Maps a test page to one frame and then another.
fn check_paging() -> Result<(), &'static str> {
    // 256 GB + 2 MB + 8 KB: in the kernel's part, unused, and with a
    // different index at each level (0, 256, 1, 2), so a mix-up would show.
    const TEST_PAGE: u64 = 0x40_0020_2000;
    // The boot tables map the VGA buffer with a 4 KB page and 2-4 MB with a
    // 2 MB page, and leave the guard page out.
    let boot = [(0xB8000, Some(0xB8000)), (0x20_1234, Some(0x20_1234)), (0x7F000, None)];
    if boot.iter().any(|&(virt, phys)| paging::translate(virt) != phys) {
        return Err("the boot tables don't map what they should");
    }
    let a = frames::alloc().ok_or("no frame was free")?;
    let b = frames::alloc().ok_or("no frame was free")?;
    // 4 MB is inside one of the boot tables' 2 MB pages.
    if paging::map(0x40_0000, a).is_ok() {
        return Err("map() went inside a 2 MB page");
    }
    for frame in [a, b] {
        paging::map(TEST_PAGE, frame)?;
        if paging::map(TEST_PAGE, frame).is_ok() {
            return Err("map() mapped the same page twice");
        }
        unsafe { (TEST_PAGE as *mut u64).write_volatile(frame) };
        let read = unsafe { (frame as *const u64).read_volatile() };
        if read != frame || paging::translate(TEST_PAGE + 8) != Some(frame + 8) {
            return Err("the test page didn't lead to its frame");
        }
        if paging::unmap(TEST_PAGE) != Some(frame) || paging::translate(TEST_PAGE).is_some() {
            return Err("unmap() didn't remove the page");
        }
    }
    frames::free(a);
    frames::free(b);
    Ok(())
}

// Makes an address space and, from inside it, maps a page at 512 GB and
// writes through it.
fn check_spaces() -> Result<(), &'static str> {
    const PROGRAM_PAGE: u64 = 0x80_0000_0000;   // the first address programs get - 512 GB
    let free = frames::free_count();
    let kernel = paging::pml4();
    let space = paging::new_space()?;
    let frame = frames::alloc().ok_or("no frame was free")?;
    // The code, the stack and the interrupt handlers are all in the
    // kernel's part, so everything keeps running after the switch.
    paging::switch(space);
    let mapped = paging::map(PROGRAM_PAGE, frame);
    if mapped.is_ok() {
        unsafe { (PROGRAM_PAGE as *mut u64).write_volatile(frame) };
    }
    paging::switch(kernel);
    mapped?;
    let read = unsafe { (frame as *const u64).read_volatile() };
    if read != frame || paging::translate(PROGRAM_PAGE).is_some() {
        return Err("the page wasn't mapped in the new space alone");
    }
    paging::free_space(space);
    if frames::free_count() != free {
        return Err("freeing the space didn't give back every frame");
    }
    Ok(())
}

// RAM allocated by `frames::alloc()`.
fn check_ram() -> Result<(), &'static str> {
    let regions = boot::info().regions().iter();
    let usable = regions.filter(|r| r.kind == boot::USABLE && r.base + r.length > frames::RESERVED);
    for region in usable {
        let first = region.base.max(frames::RESERVED);
        let last = region.base + region.length - 1;
        for addr in (first..last).step_by(1 << 21).chain([last]) {
            if paging::translate(addr) != Some(addr) {
                return Err("some usable RAM isn't mapped at its own address");
            }
            unsafe { (addr as *const u8).read_volatile() };
        }
    }
    Ok(())
}

// Maps the local APIC's registers with map_device() and reads the version.
fn check_apic() -> Result<(), &'static str> {
    let msr = read_msr(0x1B);
    if msr & (1 << 11) == 0 {
        return Err("APIC is turned off");
    }
    let base = msr & !0xFFF;
    paging::map_device(base)?;
    let entry = paging::walk(base).last().map_or(0, |step| step.entry);
    let version = unsafe { ((base + 0x30) as *const u32).read_volatile() };
    paging::unmap(base);
    if entry & 0x18 != 0x18 {
        return Err("map_device() left the cache on");
    }
    if !(0x10..=0x15).contains(&(version & 0xFF)) {
        return Err("APIC's version register reads wrong");
    }
    Ok(())
}

// Reads the model-specific register `msr`.
fn read_msr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
    }
    (high as u64) << 32 | low as u64
}

// Sleeping 100ms should take 10-12 ticks.
fn check_timer() -> Result<(), &'static str> {
    let start = timer::ticks();
    timer::sleep(100);
    match timer::ticks() - start {
        10..=12 => Ok(()),
        _ => Err("sleep(100) didn't take 10 ticks"),
    }
}

// The clock should provide a datetime that makes sense.
fn check_clock() -> Result<(), &'static str> {
    let now = rtc::now();
    let sensible = now.year >= 2025
        && (1..=12).contains(&now.month)
        && (1..=31).contains(&now.day)
        && now.hour < 24
        && now.minute < 60
        && now.second < 60;
    if sensible { Ok(()) } else { Err("the date makes no sense") }
}

fn check_ribbit() -> Result<(), &'static str> {
    let mut vars = calc::Variables::new();
    let mut functions = calc::Functions::new();
    let mut run = |line: &'static str| calc::evaluate(line, &mut vars, &mut functions, &mut shell::Kernel { printed: false });
    
    let d = run("Int64 D(Int64 n) { if (n == 0) { return 0; } return 1 + D(n - 1); }");
    let f = run("Int64 F(Int64 n) { return F(n + 1); }");
    if d.is_err() || f.is_err() {
        return Err("a function didn't compile");
    }
    if !matches!(run("D(29);"), Ok(Some(calc::Value::Int64(29)))) {
        return Err("D(29) didn't give 29");
    }
    if !matches!(run("F(0);"), Err(calc::Failure { error: calc::Error::TooDeep, .. })) {
        return Err("F(0) wasn't stopped as too deep");
    }
    Ok(())
}

fn check_ramdisk() -> Result<(), &'static str> {
    const SECTORS: u64 = 64;
    let mut disk = RamDisk::new(SECTORS);
    for sector in 0..SECTORS {
        disk.write(sector, &[sector as u8; SECTOR_SIZE])?;
    }
    let mut buffer = [0; SECTOR_SIZE];
    for sector in 0..SECTORS {
        disk.read(sector, &mut buffer)?;
        if buffer != [sector as u8; SECTOR_SIZE] {
            return Err("a RAM disk sector read back incorrectly");
        }
    }
    if disk.sectors() != SECTORS || disk.read(SECTORS, &mut buffer).is_ok() {
        return Err("the RAM disk is of an invalid size");
    }
    Ok(())
}

fn check_ata() -> Result<(), &'static str> {
    let mut disk = ata::identify().ok_or("no ATA disk was found")?;
    let mut sector = [0; SECTOR_SIZE];
    disk.read(0, &mut sector)?;
    if sector[510..] != [0x55, 0xAA] {
        return Err("sector 0 doesn't end in the boot signature");
    }
    disk.read(5, &mut sector)?;
    let kernel = unsafe { slice::from_raw_parts(0x10000 as *const u8, SECTOR_SIZE) };
    if sector[..] != *kernel {
        return Err("sector 5 doesn't hold the start of the kernel");
    }
    let last = disk.sectors() - 1;
    let test = 2047;
    let mut old = [0; SECTOR_SIZE];
    disk.read(test, &mut old)?;
    if disk.read(disk.sectors(), &mut sector).is_ok() {
        return Err("the disk read a sector past its end");
    }
    let pattern: [u8; SECTOR_SIZE] = array::from_fn(|i| (i % 251) as u8);
    for data in [pattern, old] {
        disk.write(test, &data)?;
        disk.flush()?;
        disk.read(test, &mut sector)?;
        if sector != data {
            return Err("sector 2047 didn't read back what was written");
        }
    }
    if disk.write(last + 1, &pattern).is_ok() {
        return Err("the disk wrote a sector past its end");
    }
    Ok(())
}

fn check_lilypad() -> Result<(), &'static str> {
    let mut mounted = fs::LILYPAD.lock();
    let lilypad = mounted.as_mut().ok_or("LilyPad wasn't mounted at boot time")?;
    let disk = ata::identify().ok_or("no ATA disk found")?;
    if lilypad.superblock().blocks as u64 != disk.sectors() - fs::FIRST_SECTOR {
        return Err("LilyPad doesn't fill the first from sector 2048 onward");
    }
    if lilypad.used()? <= lilypad.superblock().bitmap_blocks {
        return Err("LilyPad's bitmap hasn't marked its own blocks as used");
    }
    Ok(())
}
