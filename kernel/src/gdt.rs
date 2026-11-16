// Global Descriptor Table

use core::arch::asm;
use core::mem::size_of;

use crate::lock::SpinLock;

pub const DOUBLE_FAULT_IST: u8 = 1;

// Emergency stack specs.
const STACK_SIZE: usize = 4096;
static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

#[repr(C, packed)]
struct Tss {
    reserved1: u32,
    rsp: [u64; 3],      // stacks for coming in from user mode
    reserved2: u64,
    ist: [u64; 7],      // Interrupt Stack TAble
    reserved3: u64,
    reserved4: u16,
    iomap_base: u16,    // I/O permission map offset
}

static TSS: SpinLock<Tss> = SpinLock::new(Tss {
    reserved1: 0,
    rsp: [0; 3],
    reserved2: 0,
    ist: [0; 7],
    reserved3: 0,
    reserved4: 0,
    iomap_base: size_of::<Tss>() as u16,
});

// Using the same segments as the bootloader's GDT.
static GDT: SpinLock<[u64; 6]> = SpinLock::new([
    0,                     // 0x00: null - required by the CPU
    0x00CF_9A00_0000_FFFF, // 0x08: 32-bit code used only by the bootloader
    0x00CF_9200_0000_FFFF, // 0x10: data used only by the bootloader
    0x00A0_9A00_0000_FFFF, // 0x18: 64-bit code run by the kernel
    0,                     // 0x20: TSS, low half
    0,                     //       TSS, high half
]);

const TSS_SELECTOR: u16 = 0x20;

#[repr(C, packed)]
struct GdtPointer {
    limit: u16,
    base: u64,
}

pub fn init() {
    let mut tss = TSS.lock();
    let stack_bottom = &raw const DOUBLE_FAULT_STACK as u64;
    tss.ist[DOUBLE_FAULT_IST as usize - 1] = stack_bottom + STACK_SIZE as u64;
    
    let base = &*tss as *const Tss as u64;
    let limit = (size_of::<Tss>() - 1) as u64;
    let mut gdt = GDT.lock();
    gdt[4] = limit                      // limit, bits 0-15
        | (base & 0xFF_FFFF) << 16      // base, bits 0-23
        | 0x89 << 40                    // present, type 9
        | ((base >> 24) & 0xFF) << 56;  // base, bits 24-31
    gdt[5] = base >> 32;                // base, bits 32-63
    
    let pointer = GdtPointer {
        limit: (size_of::<[u64; 6]>() - 1) as u16,
        base: gdt.as_ptr() as u64,
    };
    unsafe {
        asm!("lgdt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags));
        asm!("ltr {0:x}", in(reg) TSS_SELECTOR, options(nostack, preserves_flags));
    }
}
