#![no_std]
#![no_main]

mod vga;

use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut screen = vga::Writer::new();
    
    // More lines than fit on screen - to prove scrolling works
    for i in 1..=30 {
        writeln!(screen, "Line {} of 30", i).unwrap();
    }
    writeln!(screen, "Hello, FroggeOS! 6 * 7 = {}", 6 * 7).unwrap();

    loop {
        unsafe { asm!("hlt") };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
