#![no_std]
#![no_main]

extern crate alloc;

mod gdt;
mod heap;
mod interrupts;
mod keyboard;
mod lock;
mod paging;
mod pic;
mod port;
mod shell;
mod timer;
mod vga;

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    vga::clear_screen();

    gdt::init();
    interrupts::init();
    paging::init();
    pic::init();
    timer::init();

    println!("Hello, FroggeOS!");
    println!("6 * 7 = {}", 6 * 7);

    unsafe { asm!("int3") };
    println!("Back from the breakpoint handler.");

    interrupts::enable();
    println!("Interrupts enabled.");

    shell::run()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
