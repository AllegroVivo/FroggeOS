#![no_std]
#![no_main]

extern crate alloc;

mod boot;
mod frames;
mod gdt;
mod heap;
mod interrupts;
mod keyboard;
mod lock;
mod paging;
mod pic;
mod port;
mod rtc;
mod selftest;
mod serial;
mod shell;
mod speaker;
mod timer;
mod vga;

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _start(boot_info: &'static boot::BootInfo) -> ! {
    vga::clear_screen();

    serial::init();
    gdt::init();
    interrupts::init();
    paging::init();
    frames::init(boot_info);
    pic::init();
    timer::init();

    println!("Hello, FroggeOS!");
    println!("Booted from drive {:#x}", boot_info.boot_drive);
    println!("6 * 7 = {}", 6 * 7);

    unsafe { asm!("int3") };
    println!("Back from the breakpoint handler.");

    interrupts::enable();
    println!("Interrupts enabled.");

    #[cfg(feature = "selftest")]
    selftest::exit_qemu(selftest::run());

    shell::run(boot_info)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
