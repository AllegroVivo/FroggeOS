// Paging

use alloc::vec::Vec;
use core::arch::asm;
use core::ptr;
use crate::boot::{self, BootInfo};
use crate::frames;

// The bootloader's page directory.
const PAGE_DIRECTORY: *mut u64 = 0x3000 as *mut u64;
const PAGE_TABLE: *mut u64 = 0x4000 as *mut u64;

const PAGE_SIZE: usize = 4096;
const LARGE_PAGE: u64 = 1 << 21;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const WRITE_THROUGH: u64 = 1 << 3;
const NO_CACHE: u64 = 1 << 4;
const HUGE: u64 = 1 << 7;
const ADDRESS: u64 = 0x000F_FFFF_FFFF_F000;

const LEVELS: [(&str, u64); 4] = [
    ("PML4", 1 << 39),  // 512 KB
    ("PDPT", 1 << 30),  // 1 GB
    ("PD", 1 << 21),    // 2 MB
    ("PT", 1 << 12),    // 4 KB
];

const GUARD_PAGE: usize = 0x7F000;

pub fn init() {
    unsafe {
        for i in 0..512 {
            let addr = i * PAGE_SIZE;
            let entry = if addr == GUARD_PAGE {
                0 // not present - any access faults
            } else {
                addr as u64 | PRESENT | WRITABLE
            };
            PAGE_TABLE.add(i).write_volatile(entry);
        }
        PAGE_DIRECTORY.write_volatile(PAGE_TABLE as u64 | PRESENT | WRITABLE);

        asm!("mov {tmp}, cr3", "mov cr3, {tmp}", tmp = out(reg) _, options(nostack, preserves_flags));
    }
}

// Maps every 2 MB page that holds usable RAM at its own address.
pub fn map_ram(boot_info: &BootInfo) {
    let usable = boot_info.regions().iter().filter(|r| r.kind == boot::USABLE);
    for region in usable {
        let first = region.base & !(LARGE_PAGE - 1);
        let end = region.base + region.length;
        for page in (first..end).step_by(LARGE_PAGE as usize) {
            let Ok(entry) = find_entry(page, 2, true) else {
                panic!("couldn't map the RAM above 1 GB");
            };
            if *entry & PRESENT == 0 {
                *entry = page | PRESENT | WRITABLE | HUGE;
            }
        }
    }
}

pub fn stack_overflowed() -> bool {
    let addr: u64;
    unsafe { asm!("mov {}, cr2", out(reg) addr, options(nomem, nostack, preserves_flags)) };
    (GUARD_PAGE..GUARD_PAGE + PAGE_SIZE).contains(&(addr as usize))
}

// What a page-table entry leads to.
#[derive(Clone, Copy)]
pub enum Target {
    Table(u64),         // the next level's table (address)
    Page(u64, u64),     // a page (address, size)
    Missing,            // a non-present entry
}

// One level of a walk.
pub struct Step {
    pub table: &'static str,
    pub index: u64,
    pub entry: u64,
    pub target: Target,
}

// Follows `virt` down from the PML4 until it reaches a not-present page or entry.
pub fn walk(virt: u64) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut table = pml4();
    for (name, covers) in LEVELS {
        let index = (virt / covers) % 512;
        let entry = *slot(table, index);
        let target = if entry & PRESENT == 0 {
            Target::Missing
        } else if name == "PT" || entry & HUGE != 0 {
            Target::Page(entry & ADDRESS & !(covers - 1), covers)
        } else {
            Target::Table(entry & ADDRESS)
        };
        steps.push(Step { table: name, index, entry, target });
        let Target::Table(next) = target else { break };
        table = next;
    }
    steps
}

// The physical address `virt` maps to.
pub fn translate(virt: u64) -> Option<u64> {
    match walk(virt).last()?.target {
        Target::Page(addr, size) => Some(addr + virt % size),
        _ => None
    }
}

// Maps the 4 KB page at `virt` to `frame`.
pub fn map(virt: u64, frame: u64) -> Result<(), &'static str> {
    map_with(virt, frame, WRITABLE)
}

// Maps a device's 4 KB page at it sown address with cache off.
pub fn map_device(addr: u64) -> Result<(), &'static str> {
    map_with(addr, addr, WRITABLE| WRITE_THROUGH | NO_CACHE)
}

// Maps the 4 KB page at `virt` to `frame`, using `flags`.
fn map_with(virt: u64, frame: u64, flags: u64) -> Result<(), &'static str> {
    let entry = find_entry(virt, 3, true)?;
    if *entry & PRESENT != 0 {
        return Err("this page is already mapped");
    }
    *entry = frame | PRESENT | flags;
    flush(virt);
    Ok(())
}


// Unmaps the 4 KB page at `virt` and returns its frame.
pub fn unmap(virt: u64) -> Option<u64> {
    let entry = find_entry(virt, 3, false).ok()?;
    if *entry & PRESENT == 0 {
        return None;
    }
    let frame = *entry & ADDRESS;
    *entry = 0;
    flush(virt);
    Some(frame)
}

// Makes a new address space.
pub fn new_space() -> Result<u64, &'static str> {
    let space = frames::alloc().ok_or("no frame was free for a PML4")?;
    unsafe { ptr::write_bytes(space as *mut u8, 0, PAGE_SIZE) };
    *slot(space, 0) = *slot(pml4(), 0);
    Ok(space)
}

// Loads `space` into CR3.
pub fn switch(space: u64) {
    unsafe { asm!("mov cr3, {}", in(reg) space, options(nostack, preserves_flags)) };
}

// Frees an address space that isn't in use.
//noinspection ALL
pub fn free_space(space: u64) {
    assert!(space != pml4(), "the address space at {:#x} is in use", space);
    for index in 1..512 {
        free_entry(*slot(space, index), 0);
    }
    frames::free(space);
}

// Frees what `entry` leads to.
fn free_entry(entry: u64, depth: usize) {
    if entry & PRESENT == 0 {
        return;
    }
    let addr = entry & ADDRESS;
    if depth < 3 {
        for index in 0..512 {
            free_entry(*slot(addr, index), depth + 1);
        }
    }
    frames::free(addr);
}

// Finds the PT entry for `virt`.
fn find_entry(virt: u64, depth: usize, create: bool) -> Result<&'static mut u64, &'static str> {
    let mut table = pml4();
    for &(_, covers) in &LEVELS[..depth] {
        let entry = slot(table, (virt / covers) % 512);
        if *entry & PRESENT == 0 {
            if !create {
                return Err("a table on the way is missing")
            }
            let frame = frames::alloc().ok_or("no frame was free for a table")?;
            unsafe { ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE) };
            *entry = frame | PRESENT | WRITABLE;
        } else if *entry & HUGE != 0 {
            return Err("the address is inside a 2 MB or 1 GB page");
        }
        table = *entry & ADDRESS;
    }
    Ok(slot(table, (virt / LEVELS[depth].1) % 512))
}

// Entry `index` of the table at `table`.
fn slot(table: u64, index: u64) -> &'static mut u64 {
    unsafe { &mut *(table as *mut u64).add(index as usize) }
}

// The PML4's address.
pub fn pml4() -> u64 {
    let cr3: u64;
    unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
    cr3 & ADDRESS
}

fn flush(virt: u64) {
    unsafe { asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags)) };
}
