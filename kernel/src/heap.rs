// The kernel heap.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

use crate::lock::SpinLock;

// 1MB-2MB: ordinary RAM that nothing uses.
const START: usize = 0x10_0000;
pub const SIZE: usize = 0x10_0000;

struct Bump {
    next: usize,        // address of the next free byte
    allocations: usize, // how many allocations haven't been freed
}

struct BumpAllocator(SpinLock<Bump>);

// Tells Rust to send every allocation in the kernel to this static.
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator(SpinLock::new(Bump {
    next: START,
    allocations: 0,
}));

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.0.lock();
        let start = align_up(bump.next, layout.align());
        let Some(end) = start.checked_add(layout.size()) else {
            return null_mut();  // so big the address would overflow
        };
        if end > START + SIZE {
            return null_mut(); // out of memory
        };
        bump.next = end;
        bump.allocations += 1;
        start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.0.lock();
        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = START; // everything is free
        }
    }
}

// Round ``addr`` up to a multiple of ``align``.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

// (bytes used, allocations not yet freed) for the shell's `heap` command.
pub fn stats() -> (usize, usize) {
    let bump = ALLOCATOR.0.lock();
    (bump.next - START, bump.allocations)
}
