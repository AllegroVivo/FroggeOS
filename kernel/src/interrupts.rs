// CPU Exceptions. An Interrupt Descriptor Table (IDT) informs the
// CPU which function to fun for each of the interrupt/exception numbers.

use core::arch::{asm, naked_asm};
use core::mem::size_of;

use crate::gdt;
use crate::lock::SpinLock;
use crate::println;

// A single IDT entry.
// The layout is 16 bytes, expected in this order by the CPU.
#[repr(C)]
struct IdtEntry {
    offset_low:     u16, // handler address, bits 0-15
    selector:       u16, // code segment to run handler in
    ist:            u8,  // alternate stack number, 0 to prevent switching
    flags:          u8,  // present, privilege level, gate type
    offset_mid:     u16, // handler address, bits 16-31
    offset_high:    u32, // handler address, bits 32-63
    reserved:       u32,
}

impl IdtEntry {
    const MISSING: IdtEntry = IdtEntry {
        offset_low: 0,
        selector:   0,
        ist:        0,
        flags:      0,
        offset_mid: 0,
        offset_high: 0,
        reserved:   0,
    };

    fn new(handler: extern "C" fn(), selector: u16) -> IdtEntry {
        let addr = handler as usize;
        IdtEntry {
            offset_low: addr as u16,
            selector,
            ist: 0,
            flags: 0x8E,
            offset_mid: (addr >> 16) as u16,
            offset_high: (addr >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct IdtPointer {
    limit: u16,
    base: u64,
}

static IDT: SpinLock<[IdtEntry; 256]> = SpinLock::new([IdtEntry::MISSING; 256]);

pub fn init() {
    // The code segment the kernel is running in right now.
    let cs: u16;
    unsafe { asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags)) };

    let mut idt = IDT.lock();
    idt[0] = IdtEntry::new(divide_error, cs);
    idt[3] = IdtEntry::new(breakpoint, cs);
    idt[6] = IdtEntry::new(invalid_opcode, cs);
    idt[8] = IdtEntry::new(double_fault, cs);
    idt[8].ist = gdt::DOUBLE_FAULT_IST; // runs on the emergency stack.
    idt[13] = IdtEntry::new(general_protection, cs);
    idt[14] = IdtEntry::new(page_fault, cs);
    idt[32] = IdtEntry::new(timer_interrupt, cs);
    idt[33] = IdtEntry::new(keyboard_interrupt, cs);

    let pointer = IdtPointer {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: idt.as_ptr() as u64,
    };
    unsafe { asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags)) };
}

// Allow hardware interrupts in.
// Call only after ``init()``, ``pic::init()`` and ``timer::init()``.
pub fn enable() {
    unsafe { asm!("sti", options(nostack, preserves_flags)) };
}

pub fn disable() {
    unsafe { asm!("cli", options(nostack, preserves_flags)) };
}

pub fn are_enabled() -> bool {
    let rflags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) rflags, options(nomem, preserves_flags)) };
    (rflags & 0x200) != 0
}

// ---- Entry stubs ----
// When an exception occurs, the CPU pushes ``RIP``, ``CS``, ``RFLAGS``, ``RSP``
// and ``SS`` and jumps to the handler. Some exceptions also push an error code.

macro_rules! exception_stub {
    ($name:ident, $vector:literal) => {
        #[unsafe(naked)]
        extern "C" fn $name() {
            naked_asm!(
                "push 0",   // dummy error code
                "push {vector}",
                "jmp {common}",
                vector = const $vector,
                common = sym exception_common,
            );
        }
    };
    ($name:ident, $vector:literal, has_error_code) => {
        #[unsafe(naked)]
        extern "C" fn $name() {
            naked_asm!(
                "push {vector}",
                "jmp {common}",
                vector = const $vector,
                common = sym exception_common,
            );
        }
    };
}

exception_stub!(divide_error, 0);
exception_stub!(breakpoint, 3);
exception_stub!(invalid_opcode, 6);
exception_stub!(double_fault, 8, has_error_code);
exception_stub!(general_protection, 13, has_error_code);
exception_stub!(page_fault, 14, has_error_code);
exception_stub!(timer_interrupt, 32);
exception_stub!(keyboard_interrupt, 33);

// Saves the registers a Rust function may to change, calls ``exception_handler``,
// restores them, and returns to the interrupted code.
#[unsafe(naked)]
extern "C" fn exception_common() {
    naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "lea rdi, [rsp + 9 * 8]",   // 1st argument - skip the saved registers
        "cld",                      // Rust expects the ``direction`` flag clear
        "call {handler}",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "add rsp, 16",              // drop the exception number and error code
        "iretq",                    // return to wherever the exception happened.
        handler = sym exception_handler,
    );
}

#[repr(C)]
struct ExceptionFrame {
    vector: u64,
    error_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

extern "C" fn exception_handler(frame: &ExceptionFrame) {
    if frame.vector == 32 {
        crate::timer::tick();
        crate::pic::end_of_interrupt();
        return;
    }
    if frame.vector == 33 {
        crate::keyboard::handle_interrupt();
        crate::pic::end_of_interrupt();
        return;
    }

    let name = match frame.vector {
        0 => "divide error",
        3 => "breakpoint",
        6 => "invalid opcode",
        8 => "double fault",
        13 => "general protection fault",
        14 => "page fault",
        _ => "unknown exception",
    };
    println!(
        "EXCEPTION: {} (#{}) at {:#x}, error code {:#x}",
        name, frame.vector, frame.rip, frame.error_code
    );

    if frame.vector == 14 {
        // In the event of a paging fault, Query CR2 for the address the code tried to use.
        let addr: u64;
        unsafe { asm!("mov {}, cr2", out(reg) addr, options(nomem, nostack, preserves_flags)) };
        println!("  address: {:#x}", addr);
    }

    if frame.vector == 3 {
        return; // breakpoints are harmless
    }
    loop {
        unsafe { asm!("hlt") };
    }
}
