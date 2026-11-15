// Programmable Interrupt Controller

use crate::port::outb;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const OFFSET: u8 = 32;

pub fn init() {
    unsafe {
        outb(PIC1_COMMAND, 0x11);
        outb(PIC2_COMMAND, 0x11);
        outb(PIC1_DATA, OFFSET);
        outb(PIC2_DATA, OFFSET + 8);
        outb(PIC1_DATA, 4);
        outb(PIC2_DATA, 2);
        outb(PIC1_DATA, 0x01);
        outb(PIC2_DATA, 0x01);
        outb(PIC1_DATA, 0xFC);
        outb(PIC2_DATA, 0xFF);
    }
}

pub fn end_of_interrupt() {
    unsafe { outb(PIC1_COMMAND, 0x20) }; 
}
