#![no_std]
#![no_main]

mod interrupts;
mod lock;
mod pic;
mod port;
mod timer;
mod vga;

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga::clear_screen();

    interrupts::init();
    pic::init();
    timer::init();

    println!("Hello, FroggeOS!");
    println!("6 * 7 = {}", 6 * 7);

    unsafe { asm!("int3") };
    println!("Back from the breakpoint handler.");

    interrupts::enable();
    println!("Interrupts enabled.");

    let mut shown = 0;
    loop {
        unsafe { asm!("hlt") }; // sleep until the next interrupt
        let seconds = timer::ticks() / timer::HZ;
        if seconds != shown {
            shown = seconds;
            println!("{} seconds", shown);
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
