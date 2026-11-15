// The kernel heap. This memory is handed out at runtime for mutable structs
// such as String, Vec, Box, etc... Implemented as a free-list allocator.

use core::alloc::{GlobalAlloc, Layout};
use core::mem::size_of;
use core::ptr::null_mut;
use crate::lock::SpinLock;

// 1MB-2MB: ordinary RAM that nothing uses.
const START: usize = 0x10_0000;
const SIZE: usize = 0x10_0000;

// Every block must start on a boundary of this size and must be a
// multiple of this many bytes long.
const BLOCK_ALIGN: usize = 16;

struct FreeBlock {
    size: usize,            // the size of this whole block, in bytes
    next: *mut FreeBlock,   // the next free block, or ``null` for end of list.
}

struct Heap {
    next: usize,            // starting point for the unused memory.
    free: *mut FreeBlock,   // first block in the free list (or null if empty)
    allocations: usize      // the number of unfreed allocations
}

// Raw pointers aren't Send, so by default Heap couldn't go inside a static SpinLock.
// It's fine here because the lock lets only one piece of code touch it at a time.
unsafe impl Send for Heap {}

struct FreeListAllocator(SpinLock<Heap>);

#[global_allocator]
static ALLOCATOR: FreeListAllocator = FreeListAllocator(SpinLock::new(Heap {
    next: START,
    free: null_mut(),
    allocations: 0,
}));

unsafe impl GlobalAlloc for FreeListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > SIZE {
            return null_mut(); // can never fit
        }
        let size = block_size(layout);
        let align = layout.align().max(BLOCK_ALIGN);
        let mut heap = self.0.lock();

        // Reuse a free block if one has room.
        if let Some(addr) = heap.take_free(size, align) {
            heap.allocations += 1;
            return addr as *mut u8;
        }

        // Nothing suitable in the list - use fresh memory.
        let start = align_up(heap.next, align);
        let end = start + size;
        if end > START + SIZE {
            return null_mut(); // out of memory
        }
        if start > heap.next {
            // The gap skipped to reach alignment is still good memory.
            let gap = heap.next;
            unsafe { heap.push_free(gap, start - gap) };
        }
        heap.next = end;
        heap.allocations += 1;
        start as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = block_size(layout); // same size reserved by alloc()
        let mut heap = self.0.lock();
        unsafe { heap.push_free(ptr as usize, size) };
        heap.allocations -= 1;
    }
}

impl Heap {
    // Give a block back to the free list.
    unsafe fn push_free(&mut self, addr: usize, size: usize) {
        // Find neighbours - ``prev`` is the last free block below addr,
        // ``next`` the first one above it.
        let mut prev: *mut FreeBlock = null_mut();
        let mut next = self.free;
        while !next.is_null() && (next as usize) < addr {
            prev = next;
            next = unsafe { (*next).next }
        }

        // The block's memory becomes its list node.
        let block = addr as *mut FreeBlock;
        unsafe {
            block.write(FreeBlock{ size, next });

            // Consume the next free block if touched.
            if !next.is_null() && addr + size == next as usize {
                (*block).size += (*next).size;
                (*block).next = (*next).next;
            }

            if prev.is_null() {
                self.free = block;
            } else if prev as usize + (*prev).size == addr {
                (*prev).size += (*block).size;
                (*prev).next = (*block).next;
            } else {
                (*prev).next = block;
            }
        }
    }

    // Find the first free block with room for ``size`` bytes starting
    // at a multiple of ``align``, and cut that piece out.
    fn take_free(&mut self, size: usize, align: usize) -> Option<usize> {
        let mut prev: *mut FreeBlock = null_mut();
        let mut current = self.free;
        while !current.is_null() {
            let block = unsafe { &mut *current };
            let block_start = current as usize;
            let block_end = block_start + block.size;
            let start = align_up(block_start, align);
            let end = start + size;

            if end <= block_end {
                let behind = if end < block_end {
                    let rest = end as *mut FreeBlock;
                    unsafe { rest.write(FreeBlock { size: block_end - end, next: block.next}) };
                    rest
                } else {
                    block.next
                };
                if start > block_start {
                    block.size = start - block_start;
                    block.next = behind;
                } else if prev.is_null() {
                    self.free = behind;
                } else {
                    unsafe { (*prev).next = behind };
                }
                return Some(start)
            }
            prev = current;
            current = block.next;
        }
        None
    }
}

// How many bytes a block for ``layout`` takes.
fn block_size(layout: Layout) -> usize {
    align_up(layout.size().max(size_of::<FreeBlock>()), BLOCK_ALIGN)
}

// Round ``addr`` up to a multiple of ``align``.
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

// (bytes in use, bytes free in the list, allocations not yet freed)
pub fn stats() -> (usize, usize, usize) {
    let heap = ALLOCATOR.0.lock();
    let mut reusable = 0;
    let mut current = heap.free;
    while !current.is_null() {
        let block = unsafe { &*current };
        reusable += block.size;
        current = block.next;
    }
    (heap.next - START - reusable, reusable, heap.allocations)
}
