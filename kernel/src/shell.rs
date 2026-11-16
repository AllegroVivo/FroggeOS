// A command prompt.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;

use crate::{calc, heap, keyboard, timer, vga};
use crate::{print, println};

// ``> `` + 78 characters fills an 80-character row.
const MAX_LINE: usize = 78;

pub fn run() -> ! {
    println!("FroggeOS shell. Type 'help' for a list of commands.");

    let mut line = [0u8; MAX_LINE];
    let mut len = 0;
    let mut history: Vec<String> = Vec::new();
    print!("> ");

    loop {
        match wait_for_char() {
            '\n' => {
                println!();
                let text = core::str::from_utf8(&line[..len]).unwrap();
                if !text.trim().is_empty() {
                    history.push(String::from(text)); // heap copy
                }
                run_command(text, &history);
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

fn run_command(line: &str, history: &[String]) {
    let mut words = line.split_whitespace();
    let Some(command) = words.next() else {
        return;
    };

    // A line starting with a digit, '(', or '-' is a calculation.
    if command.starts_with(|c: char| c.is_ascii_digit() || c == '(' || c == '-') {
        match calc::evaluate(line) {
            Ok(value) => println!("{}", value),
            Err(error) => println!("error: {}", error),
        }
        return;
    }

    match command {
        "help" => {
            println!("help         show this list");
            println!("clear        clear the screen");
            println!("echo <text>  print <text>");
            println!("uptime       time since the timer started");
            println!("history      list the commands typed so far");
            println!("heap         show how much of the heap is used");
            println!("<expression> calculate it, e.g. 2 * (3 + 4) - 10 / 5")
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
        "history" => {
            for (i, entry) in history.iter().enumerate() {
                println!("{:>3}  {}", i + 1, entry);
            }
        }
        "heap" => {
            let (in_use, reusable, allocations) = heap::stats();
            println!(
                "{} bytes in use by {} allocations, {} bytes free for reuse",
                in_use, allocations, reusable
            );
        }
        _ => println!("unknown command: {} (try 'help')", command),
    }
}
