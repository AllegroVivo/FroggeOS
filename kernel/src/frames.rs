// Physical frame allocator.

use core::slice;
use crate::boot::{self, BootInfo};
use crate::lock::SpinLock;

pub const FRAME_SIZE: u64 = 4096;

// Space below 2 MB is already reserved by BIOS data, page tables,
// boot info, kernel, and so on. Start the bit mapping after that.
pub const RESERVED: u64 = 0x20_0000;
const BITMAP: u64 = RESERVED;

struct Frames {
    bitmap: &'static mut [u64], // bit `b` of word `w` is frame `w * 64 + b`
    free: usize,
}

static FRAMES: SpinLock<Frames> = SpinLock::new(Frames { bitmap: &mut [], free: 0 });

impl Frames {
    fn set_free(&mut self, frame: usize) {
        self.bitmap[frame / 64] &= !(1 << (frame % 64));
        self.free += 1;
    }
}

// Builds the bitmap from the memory map.
pub fn init(boot_info: &BootInfo) {
    let usable = || boot_info.regions().iter().filter(|r| r.kind == boot::USABLE);
    let top = usable().map(|r| r.base + r.length).max().unwrap_or(0);
    let words = (top / FRAME_SIZE).div_ceil(64);
    let end = BITMAP + words * 8;

    // Writing the bitmap anywhere besides usable RAM would break something.
    assert!(
        usable().any(|r| r.base <= BITMAP && end <= r.base + r.length),
        "no usable RAM for the frame bitmap"
    );

    let mut frames = FRAMES.lock();
    frames.bitmap = unsafe { slice::from_raw_parts_mut(BITMAP as *mut u64, words as usize) };
    frames.bitmap.fill(u64::MAX);

    let first_free = end.div_ceil(FRAME_SIZE);
    for region in usable() {
        let first = region.base.div_ceil(FRAME_SIZE).max(first_free);
        let last = (region.base + region.length) / FRAME_SIZE;
        for frame in first..last {
            frames.set_free(frame as usize);
        }
    }
}

// Takes the lowest free frame and returns its address, or None if none is left.
pub fn alloc() -> Option<u64> {
    let mut frames = FRAMES.lock();
    let word = frames.bitmap.iter().position(|&bits| bits != u64::MAX)?;
    let bit = frames.bitmap[word].trailing_ones() as usize;
    frames.bitmap[word] |= 1 << bit;
    frames.free -= 1;
    Some((word * 64 + bit) as u64 * FRAME_SIZE)
}

// Gives back a frame handed out by alloc().
pub fn free(addr: u64) {
    let frame = (addr / FRAME_SIZE) as usize;
    let mut frames = FRAMES.lock();
    let used = frames.bitmap[frame / 64] & (1 << (frame % 64)) != 0;
    assert!(used, "frame {:#x} was already free", addr);
    frames.set_free(frame);
}

// The number of currently free frames.
pub fn free_count() -> usize {
    FRAMES.lock().free
}
