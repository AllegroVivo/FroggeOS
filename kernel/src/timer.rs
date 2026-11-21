// Programmable Interval Timer (PIT)

use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::port::outb;

pub const HZ: u64 = 100;
pub const PIT_CLOCK: u64 = 1_193_182;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    // PIT fires once every `divisor` cycles of its input clock.
    let divisor = (PIT_CLOCK / HZ) as u16;
    unsafe {
        outb(0x43, 0x36);
        outb(0x40, divisor as u8);
        outb(0x40, (divisor >> 8) as u8);
    }
}

// Called by the timer interrupt.
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

// Waits approximately `ms` milliseconds to the nearest timer tick.
pub fn sleep(ms: u64) {
    let end = ticks() + (ms * HZ).div_ceil(1000);
    while ticks() < end {
        unsafe { asm!("hlt") };
    }
}
