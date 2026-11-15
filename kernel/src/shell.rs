// A command prompt.

use core::arch::asm;

use crate::{keyboard, timer, vga};
use crate::{print, println};

// ``> `` + 78 characters fills an 80-character row.
const MAX_LINE: usize = 78;

pub fn run() -> ! {
    println!("FroggeOS shell. Type 'help' for a list of commands.");

    let mut line = [0u8; MAX_LINE];
    let mut len = 0;
    print!("> ");

    loop {
        match wait_for_char() {
            '\n' => {
                println!();
                let text = core::str::from_utf8(&line[..len]).unwrap();
                run_command(text);
                len = 0;
                print!("> ");
            }
            '\x08' => {
                if len > 0 {
                    len -= 1;
                    print!("\x08");
                }
            }
            c => {
                if len < MAX_LINE {
                    line[len] = c as u8;
                    len += 1;
                    print!("{}", c);
                }
            }
        }
    }
}

fn wait_for_char() -> char {
    loop {
        if let Some(c) = keyboard::read_char() {
            return c;
        }
        unsafe { asm!("hlt") };
    }
}

fn run_command(line: &str) {
    let mut words = line.split_whitespace();
    let Some(command) = words.next() else {
        return;
    };

    match command {
        "help" => {
            println!("help         show this list");
            println!("clear        clear the screen");
            println!("echo <text>  print <text>");
            println!("uptime       time since the timer started");
        }
        "clear" => vga::clear_screen(),
        "echo" => {
            let text = line.trim_start()["echo".len()..].trim();
            println!("{}", text);
        }
        "uptime" => {
            let ticks = timer::ticks();
            println!("{}.{:02} seconds", ticks / timer::HZ, ticks % timer::HZ);
        }
        _ => println!("unknown command: {} (try 'help')", command),
    }
}
