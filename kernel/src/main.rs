#![no_std]
#![no_main]

mod lock;
mod vga;

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga::clear_screen();
    println!("Hello, FroggeOS!");
    println!("6 * 7 = {}", 6 * 7);

    loop {
        unsafe { asm!("hlt") };
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
