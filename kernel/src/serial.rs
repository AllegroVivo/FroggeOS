// The COM1 serial port.

use core::fmt;
use crate::port::{inb, outb};

const COM1: u16 = 0x3F8;

pub fn init() {
    unsafe {
        outb(COM1 + 1, 0x00);   // no interrupts
        outb(COM1 + 3, 0x80);   // ports +0 & +1 hold the speed divisor
        outb(COM1, 1);               // low byte - 115200 / 1 bits per sec.
        outb(COM1 + 1, 0);      // high byte
        outb(COM1 + 3, 0x03);   // back to normal
        outb(COM1 + 2, 0x07);   // turn on FIFO buffers
        outb(COM1 + 4, 0x03);   // emit ready signal
    }
}

// Sends one byte.
pub fn send(byte: u8) {
    unsafe {
        while inb(COM1 + 5) & 0x20 == 0 {}
        outb(COM1, byte);
    }
}

pub struct Serial;

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                // A terminal requires a carriage return to get back col 0.
                b'\n' => {
                    send(b'\r');
                    send(b'\n');
                },
                // Backspace should erase the last character on screen.
                0x08 => {
                    send(0x08);
                    send(b' ');
                    send(0x08);
                },
                _ => send(byte),
            }
        }
        Ok(())
    }
}
