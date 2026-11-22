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
    for (vector, handler) in VECTORS.iter().enumerate() {
        idt[vector] = IdtEntry::new(handler.0, cs);
    }
    idt[8].ist = gdt::DOUBLE_FAULT_IST; // runs on the emergency stack

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

macro_rules! stub {
    ($vector:literal) => {{
        #[unsafe(naked)]
        extern "C" fn entry() {
            naked_asm!(
                "push 0",   // dummy error code
                "push {vector}",
                "jmp {common}",
                vector = const $vector,
                common = sym exception_common,
            );
        }
        entry
    }};
    ($vector:literal, has_error_code) => {{
        #[unsafe(naked)]
        extern "C" fn entry() {
            naked_asm!(
                "push {vector}",
                "jmp {common}",
                vector = const $vector,
                common = sym exception_common,
            );
        }
        entry
    }};
}

// CPU exceptions and their handlers.
static VECTORS: [(extern "C" fn(), &str); 34] = [
    (stub!(0), "divide error"),
    (stub!(1), "debug"),
    (stub!(2), "non-maskable interrupt"),
    (stub!(3), "breakpoint"),
    (stub!(4), "overflow"),
    (stub!(5), "bound range exceeded"),
    (stub!(6), "invalid opcode"),
    (stub!(7), "device not available"),
    (stub!(8, has_error_code), "double fault"),
    (stub!(9), "coprocessor segment overrun"),
    (stub!(10, has_error_code), "invalid TSS"),
    (stub!(11, has_error_code), "segment not present"),
    (stub!(12, has_error_code), "stack-segment fault"),
    (stub!(13, has_error_code), "general protection fault"),
    (stub!(14, has_error_code), "page fault"),
    (stub!(15), "reserved"),
    (stub!(16), "x87 floating-point error"),
    (stub!(17, has_error_code), "alignment check"),
    (stub!(18), "machine check"),
    (stub!(19), "SIMD floating-point error"),
    (stub!(20), "virtualization exception"),
    (stub!(21, has_error_code), "control protection"),
    (stub!(22), "reserved"),
    (stub!(23), "reserved"),
    (stub!(24), "reserved"),
    (stub!(25), "reserved"),
    (stub!(26), "reserved"),
    (stub!(27), "reserved"),
    (stub!(28), "hypervisor injection"),
    (stub!(29, has_error_code), "VMM communication"),
    (stub!(30, has_error_code), "security exception"),
    (stub!(31), "reserved"),
    (stub!(32), "timer"),
    (stub!(33), "keyboard"),
];

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

    let name = VECTORS[frame.vector as usize].1;
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
    if frame.vector == 8 && crate::paging::stack_overflowed() {
        println!("  cause: stack overflow");
    }
    if frame.vector == 3 {
        return; // breakpoints are harmless
    }
    loop {
        unsafe { asm!("hlt") };
    }
}
