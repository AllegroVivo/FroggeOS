// Interface for the PS/2 keyboard.
// Key presses raise IRQ 1 and put a scancode on port 0x60.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::lock::SpinLock;
use crate::port::inb;

const DATA_PORT: u16 = 0x60;

// A ring buffer holding scancodes for ``read_key()``.
const QUEUE_SIZE: usize = 64;

struct Queue {
    buffer: [Key; QUEUE_SIZE],
    start: usize,
    len: usize,
}

impl Queue {
    fn push(&mut self, key: Key) {
        if self.len == QUEUE_SIZE {
            return;
        }
        self.buffer[(self.start + self.len) % QUEUE_SIZE] = key;
        self.len += 1;
    }

    fn pop(&mut self) -> Option<Key> {
        if self.len == 0 {
            return None;
        }
        let key = self.buffer[self.start];
        self.start = (self.start + 1) % QUEUE_SIZE;
        self.len -= 1;
        Some(key)
    }
}

static QUEUE: SpinLock<Queue> = SpinLock::new(Queue {
    buffer: [Key::Char('\0'); QUEUE_SIZE],
    start: 0,
    len: 0,
});

// Called by keyboard interrupt.
pub fn handle_interrupt() {
    let scancode = unsafe { inb(DATA_PORT) };
    if let Some(key) = translate(scancode) {
        QUEUE.lock().push(key);
    }
}

// A key press.
#[derive(Copy, Clone)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    PageUp,
    PageDown,
}

// Read in the next key press, or None.
pub fn read_key() -> Option<Key> {
    QUEUE.lock().pop()
}

// Checks whether 'Ctrl+C' has been pressed to interrupt execution.
pub fn take_ctrl_c() -> bool {
    let mut queue = QUEUE.lock();
    let pressed = (0..queue.len)
        .any(|i| matches!(queue.buffer[(queue.start + i) % QUEUE_SIZE], Key::Ctrl('c')));
    if pressed {
        queue.len = 0;
    }
    pressed
}

static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static ALT_DOWN: AtomicBool = AtomicBool::new(false);
static CAPS_LOCK: AtomicBool = AtomicBool::new(false);
static EXTENDED: AtomicBool = AtomicBool::new(false);

// US layout.
const NORMAL: &[u8; 58] = b"\0\01234567890-=\x08\0qwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const SHIFTED: &[u8; 58] = b"\0\0!@#$%^&*()_+\x08\0QWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

fn translate(scancode: u8) -> Option<Key> {
    // Keys added after the first PC keyboard (i.e. arrows) send `0xE0` first.
    if scancode == 0xE0 {
        EXTENDED.store(true, Ordering::Relaxed);
        return None;
    }
    if EXTENDED.swap(false, Ordering::Relaxed) {
        return extended(scancode);
    }
    match scancode {
        0x2A | 0x36 => {
            SHIFT_DOWN.store(true, Ordering::Relaxed);  // left/right Shift pressed
            None
        }
        0xAA | 0xB6 => {
            SHIFT_DOWN.store(false, Ordering::Relaxed); // ... released
            None
        }
        0x1D | 0x9D => {
            CTRL_DOWN.store(scancode == 0x1D, Ordering::Relaxed);
            None
        },
        0x38 | 0xB8 => {
            ALT_DOWN.store(scancode == 0x38, Ordering::Relaxed);
            None
        },
        0x3A => {
            CAPS_LOCK.fetch_xor(true, Ordering::Relaxed);
            None
        },
        0x00..=0x39 => {
            let table = if SHIFT_DOWN.load(Ordering::Relaxed) { SHIFTED } else { NORMAL };
            match table[scancode as usize] {
                0 => None, // key has no character (Ctrl/Alt, etc...)
                _ if ALT_DOWN.load(Ordering::Relaxed) => None,
                byte if CTRL_DOWN.load(Ordering::Relaxed) && byte.is_ascii_alphabetic() => {
                    Some(Key::Ctrl(byte.to_ascii_lowercase() as char))
                },
                byte if CAPS_LOCK.load(Ordering::Relaxed) && byte.is_ascii_alphabetic() => {
                    Some(Key::Char((byte ^ 0x20) as char))
                },
                byte => Some(Key::Char(byte as char)),
            }
        }
        _ => None,
    }
}

// Keys whose scancodes come after a code `0xE0`.
fn extended(scancode: u8) -> Option<Key> {
    Some(match scancode {
        0x1C => Key::Char('\n'),        // keypad 'Enter'
        0x35 => Key::Char('/'),         // keypad '/'
        0x1D | 0x9D | 0x38 | 0xB8 => return translate(scancode), // right Ctrl/Alt
        0x47 => Key::Home,
        0x48 => Key::Up,
        0x49 => Key::PageUp,
        0x4B => Key::Left,
        0x4D => Key::Right,
        0x4F => Key::End,
        0x50 => Key::Down,
        0x51 => Key::PageDown,
        0x53 => Key::Delete,
        _ => return None,
    })
}
