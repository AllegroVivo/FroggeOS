// The integrated PC speaker.

use crate::port::{inb, outb};
use crate::timer;

// Plays a tone of `hz` cycles a second for `ms` milliseconds.
pub fn beep(hz: u32, ms: u64) {
    let divisor = (timer::PIT_CLOCK / hz as u64) as u16;
    unsafe {
        outb(0x43, 0xB6);
        outb(0x42, divisor as u8);
        outb(0x42, (divisor >> 8) as u8);
        outb(0x61, inb(0x61) | 0b11);
    }
    timer::sleep(ms);
    unsafe { outb(0x61, inb(0x61) & !0b11) };
}
