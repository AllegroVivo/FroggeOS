// Interface for the PS/2 keyboard.
// Key presses raise IRQ 1 and put a scancode on port 0x60.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::lock::SpinLock;
use crate::port::inb;

const DATA_PORT: u16 = 0x60;

// A ring buffer holding scancodes for ``read_char()``.
const QUEUE_SIZE: usize = 64;

struct Queue {
    buffer: [u8; QUEUE_SIZE],
    start: usize,
    len: usize,
}

impl Queue {
    fn push(&mut self, scancode: u8) {
        if self.len == QUEUE_SIZE {
            return;
        }
        self.buffer[(self.start + self.len) % QUEUE_SIZE] = scancode;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let scancode = self.buffer[self.start];
        self.start = (self.start + 1) % QUEUE_SIZE;
        self.len -= 1;
        Some(scancode)
    }
}

static QUEUE: SpinLock<Queue> = SpinLock::new(Queue {
    buffer: [0; QUEUE_SIZE],
    start: 0,
    len: 0,
});

// Called by keyboard interrupt.
pub fn handle_interrupt() {
    let scancode = unsafe { inb(DATA_PORT) };
    QUEUE.lock().push(scancode);
}

// Read in the next typed character, or None.
pub fn read_char() -> Option<char> {
    loop {
        let scancode = QUEUE.lock().pop()?;
        if let Some(c) = translate(scancode) {
            return Some(c);
        }
        // Wasn't a character, try next.
    }
}

static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);

// US layout.
const NORMAL: &[u8; 58] = b"\0\01234567890-=\x08\0qwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const SHIFTED: &[u8; 58] = b"\0\0!@#$%^&*()_+\x08\0QWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

fn translate(scancode: u8) -> Option<char> {
    match scancode {
        0x2A | 0x36 => {
            SHIFT_DOWN.store(true, Ordering::Relaxed);  // left/right Shift pressed
            None
        }
        0xAA | 0xB6 => {
            SHIFT_DOWN.store(false, Ordering::Relaxed); // ... released
            None
        }
        0x00..=0x39 => {
            let table = if SHIFT_DOWN.load(Ordering::Relaxed) { SHIFTED } else { NORMAL };
            match table[scancode as usize] {
                0 => None, // key has no character (Ctrl/Alt, etc...)
                byte => Some(byte as char),
            }
        }
        _ => None,
    }
}
