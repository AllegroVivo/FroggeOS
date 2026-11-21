// The VGA text screen. Consists of 80 x 25 cells at 0xB8000. Each cell
// is a u16 - low bytes is the character, high byte is the color.

use core::fmt;
use crate::lock::SpinLock;
use crate::port::outb;

const BUFFER_ADDR: usize = 0xB8000;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;
const WHITE_ON_BLACK: u16 = 0x0F00;

const CRTC_INDEX: u16 = 0x3D4;
const CRTC_DATA: u16 = 0x3D5;

fn write_cell(row: usize, col: usize, value: u16) {
    assert!(row < HEIGHT && col < WIDTH);
    let buffer = BUFFER_ADDR as *mut u16;
    unsafe { buffer.add(row * WIDTH + col).write_volatile(value) }
}

fn read_cell(row: usize, col: usize) -> u16 {
    assert!(row < HEIGHT && col < WIDTH);
    let buffer = BUFFER_ADDR as *const u16;
    unsafe { buffer.add(row * WIDTH + col).read_volatile() }
}

pub struct Writer {
    row: usize,
    col: usize,
}

impl Writer {

    pub fn clear(&mut self) {
        for row in 0..HEIGHT {
            clear_row(row);
        }
        self.row = 0;
        self.col = 0;
        self.move_cursor();
    }

    pub fn write_byte(&mut self, byte: u8) {
        if byte == b'\n' {
            self.new_line();
            return;
        }
        if self.col == WIDTH {
            self.new_line();
        }
        write_cell(self.row, self.col, WHITE_ON_BLACK | byte as u16);
        self.col += 1;
    }

    fn new_line(&mut self) {
        self.col = 0;
        if self.row < HEIGHT - 1 {
            self.row += 1;
        } else {
            scroll_up();
        }
    }

    // Step back by one cell and clear it.
    fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            write_cell(self.row, self.col, WHITE_ON_BLACK | b' ' as u16);
        }
    }

    // Moves the blinking cursor to the cell the next character goes in.
    fn move_cursor(&self) {
        let cell = self.row * WIDTH + self.col.min(WIDTH - 1);
        unsafe {
            outb(CRTC_INDEX, 0x0F);
            outb(CRTC_DATA, cell as u8);
            outb(CRTC_INDEX, 0x0E);
            outb(CRTC_DATA, (cell >> 8) as u8);
        }
    }
}

fn clear_row(row: usize) {
    for col in 0..WIDTH {
        write_cell(row, col, WHITE_ON_BLACK | b' ' as u16);
    }
}

// Move every row up by one and blank the bottom row.
fn scroll_up() {
    for row in 1..HEIGHT {
        for col in 0..WIDTH {
            write_cell(row - 1, col, read_cell(row, col));
        }
    }
    clear_row(HEIGHT - 1);
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                // Printable characters are passed thru as is.
                0x20..=0x7E | b'\n' => self.write_byte(byte),
                0x08 => self.backspace(),
                b'\t' => {
                    self.write_byte(b' ');
                    while self.col % 4 != 0 {
                        self.write_byte(b' ');
                    }
                },
                // Anything else shows as an unrenderable block.
                _ => self.write_byte(0xFE),
            }
        }
        Ok(())
    }
}

static WRITER: SpinLock<Writer> = SpinLock::new(Writer { row: 0, col: 0 });

pub fn clear_screen() {
    WRITER.lock().clear();
}
// Moves the cursor `n` cells to the left in the current row.
pub fn cursor_left(n: usize) {
    let mut writer = WRITER.lock();
    writer.col = writer.col.saturating_sub(n);
    writer.move_cursor();
}

// Called by print!/println! - must be pub so macros can expand in other modules.
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    let mut writer = WRITER.lock();
    writer.write_fmt(args).unwrap();
    writer.move_cursor();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => {$crate::print!("\n")};
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
