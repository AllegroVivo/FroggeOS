#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

const VGA_BUFFER: *mut u8 = 0xB8000 as *mut u8;
const WHITE_ON_BLACK: u8 = 0x0F;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Clear the screen - 80 cols x 25 rows
    for i in 0..(80 * 25) {
        unsafe {
            VGA_BUFFER.add(i * 2).write_volatile(b' ');
            VGA_BUFFER.add(i * 2 + 1).write_volatile(WHITE_ON_BLACK);
        }
    }

    let msg = b"Hello, FroggeOS!";
    for (i, &byte) in msg.iter().enumerate() {
        unsafe {
            VGA_BUFFER.add(i * 2).write_volatile(byte);
            VGA_BUFFER.add(i * 2 + 1).write_volatile(WHITE_ON_BLACK);
        }
    }

    loop {
        unsafe { asm!("hlt") };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
