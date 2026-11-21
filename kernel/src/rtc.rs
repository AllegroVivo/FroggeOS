// The CMOS real-time clock.

use crate::port::{inb, outb};

const INDEX: u16 = 0x70;
const DATA: u16 = 0x71;

pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

fn read(register: u8) -> u8 {
    unsafe {
        outb(INDEX, register);
        inb(DATA)
    }
}

fn read_all() -> [u8; 6] {
    while read(0x0A) & 0x80 != 0 {}
    [read(0x00), read(0x02), read(0x04), read(0x07), read(0x08), read(0x09)]
}

pub fn now() -> DateTime {
    // Handle clock ticking over between two registers.
    let mut raw = read_all();
    loop {
        let again = read_all();
        if again == raw {
            break;
        }
        raw = again;
    }
    // Numbers are typically binary-coded decimal.
    let bcd = read(0x0B) & 0x04 == 0;
    let decode = |value: u8| if bcd { (value >> 4) * 10 + (value & 0x0F) } else { value };
    let [second, minute, hour, day, month, year] = raw.map(decode);
    // The internal clock only tracks the last two digits of the year.
    DateTime { year: 2000 + year as u16, month, day, hour, minute, second }
}
