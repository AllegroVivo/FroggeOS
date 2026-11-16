// Paging

use core::arch::asm;

// The bootloader's page directory.
const PAGE_DIRECTORY: *mut u64 = 0x3000 as *mut u64;
const PAGE_TABLE: *mut u64 = 0x4000 as *mut u64;

const PAGE_SIZE: usize = 4096;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;

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

pub fn stack_overflowed() -> bool {
    let addr: u64;
    unsafe { asm!("mov {}, cr2", out(reg) addr, options(nomem, nostack, preserves_flags)) };
    (GUARD_PAGE..GUARD_PAGE + PAGE_SIZE).contains(&(addr as usize))
}
