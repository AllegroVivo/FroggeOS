// A command prompt.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use ribbit::calc;
use crate::{heap, keyboard, timer, vga};
use crate::keyboard::Key;
use crate::{print, println};

// ``> `` + 78 characters fills an 80-character row.
const MAX_LINE: usize = 78;

pub fn run() -> ! {
    println!("FroggeOS shell. Type 'help' for a list of commands.");

    let mut pending = String::new();
    let mut line = [0u8; MAX_LINE];
    let mut len = 0;
    let mut history: Vec<String> = Vec::new();
    let mut back = 0;
    let mut draft = String::new();
    let mut vars = calc::Variables::new();
    let mut functions = calc::Functions::new();
    print!("> ");

    loop {
        match wait_for_key() {
            Key::Char('\n') => {
                let text = core::str::from_utf8(&line[..len]).unwrap();
                if text.trim().is_empty() && !pending.is_empty() {
                    for _ in 0..len + 2 {
                        print!("\x08");
                    }
                } else {
                    println!();
                }
                len = 0;
                back = 0;
                if !text.trim().is_empty() {
                    if !pending.is_empty() {
                        pending.push('\n');
                    }
                    pending.push_str(text);
                    if !calc::is_complete(&pending) {
                        print!(". ");
                        continue;
                    }
                }
                if !pending.is_empty() {
                    history.push(pending.clone());
                    run_command(&pending, &history, &mut vars, &mut functions);
                    pending.clear();
                }
                print!("> ");
            },
            Key::Char('\x08') => {
                if len > 0 {
                    len -= 1;
                    print!("\x08");
                }
            },
            Key::Char(c) => {
                if len < MAX_LINE {
                    line[len] = c as u8;
                    len += 1;
                    print!("{}", c);
                }
            },
            Key::Up => {
                if let Some(entry) = recall(&history, back + 1) {
                    if back == 0 {
                        draft = String::from(core::str::from_utf8(&line[..len]).unwrap());
                    }
                    back += 1;
                    len = replace_line(&mut line, len, entry);
                }
            }
            Key::Down if back > 0 => {
                back -= 1;
                let text = if back == 0 { &draft } else { recall(&history, back).unwrap() };
                len = replace_line(&mut line, len, text);
            },
            _ => {}
        }
    }
}

fn wait_for_key() -> Key {
    loop {
        if let Some(key) = keyboard::read_key() {
            return key;
        }
        unsafe { asm!("hlt") };
    }
}

fn recall(history: &[String], back: usize) -> Option<&String> {
    history.iter().rev().filter(|entry| !entry.contains('\n')).nth(back - 1)
}

fn replace_line(line: &mut [u8], len: usize, text: &str) -> usize {
    for _ in 0..len {
        print!("\x08");
    }
    line[..text.len()].copy_from_slice(text.as_bytes());
    print!("{}", text);
    text.len()
}

struct Kernel;

impl calc::Host for Kernel {
    fn write(&mut self, text: &str) {
        print!("{}", text);
    }

    fn clear(&mut self) {
        vga::clear_screen();
    }

    fn uptime_ms(&self) -> i64 {
        (timer::ticks() * 1000 / timer::HZ) as i64
    }
}

fn run_command(
    line: &str,
    history: &[String],
    vars: &mut calc::Variables,
    functions: &mut calc::Functions,
) {
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
            println!("history      list the commands typed so far");
            println!("heap         show how much of the heap is used");
            println!("vars         list the stored variables");
            println!("lang         view a list of the current language features for Ribbit")
        },
        "clear" => vga::clear_screen(),
        "echo" => {
            let text = line.trim_start()["echo".len()..].trim();
            println!("{}", text);
        },
        "uptime" => {
            let ticks = timer::ticks();
            println!("{}.{:02} seconds", ticks / timer::HZ, ticks % timer::HZ);
        },
        "history" => {
            for (i, entry) in history.iter().enumerate() {
                println!("{:>3}  {}", i + 1, entry.replace('\n', "\n     "));
            }
        },
        "heap" => {
            let (in_use, reusable, allocations) = heap::stats();
            println!(
                "{} bytes in use by {} allocations, {} bytes free for reuse",
                in_use, allocations, reusable
            );
        },
        "vars" => {
            for (name, variable) in vars.iter() {
                let constant = if variable.constant { "const " } else { "" };
                println!("{}{} {} = {}", constant, variable.value.type_name(), name, variable.value);
            }
        },
        "lang" => {
            println!("-----------------------------------------------");
            println!("The following operators are available:");
            println!("math: + - * / %   compare: == != < <= > >=   logic: && || !");
            println!("bits: & | ^ ~ << >>   assign: = += -= *= /= %= &= |= ^= <<= >>= ++ --");
            println!("-----------------------------------------------");
            println!("The following types are available:");
            println!("Int64 (42, 0x2A), Boolean (true, false), String (\"hi\"), Void (functions only)");
            println!("-----------------------------------------------");
            println!("The following language keywords are available:");
            println!("if, else, while, do, for, break, continue, return, assert,");
            println!("var, const, true, false");
            println!("-----------------------------------------------");
            println!("The following language features are available:");
            println!("Statements: each ends with ';', except after a block's '}}'");
            println!("Variables: Int64 x = 5; var y = x + 1; const Int64 N = 3; then x = 6;");
            println!("Loops: do {{ x++; }} while (x < 9); runs its body before the first check");
            println!("Checks: assert x > 0, \"x must be positive\"; (the message is optional)");
            println!("Strings: \"Hi, \" + name, escapes \\n \\t \\\\ \\\" \\0, $\"x = {{x}}, hex {{x:X8}}\"");
            println!("Built-ins: Console.WriteLine(x), Write(x), Clear(); Timer.Uptime() in ms");
            println!("Functions: Int64 Sq(Int64 n) {{ return n * n; }}, then Sq(5);");
            println!("Multi-line: an open {{ continues on '. ' lines, and a blank line runs it");
        }
        _ => match calc::evaluate(line, vars, functions, &mut Kernel) {
            Ok(Some(value)) => println!("{}", value),
            Ok(None) => {} // line ended with an `if` or `while`.
            Err(failure) => match failure.at {
                Some(at) => mark(line, at, &failure.error),
                None => println!("Error: {}", failure.error),
            },
        },
    }
}

// Displays a mistake found during processing.
fn mark(code: &str, at: usize, error: &calc::Error) {
    let start = code [..at].rfind('\n').map_or(0, |i| i + 1);
    let end = code[at..].find('\n').map_or(code.len(), |i| at + i);
    let width = (at - start + 3).min(MAX_LINE + 2);
    if end == code.len() {
        println!("{:>width$}", "^");
        println!("Error: {}", error);
    } else {
        println!("  {}", &code[start..end]);
        println!("{:>width$}", "^");
        let number = code[..start].matches('\n').count() + 1;
        println!("Error on line {}: {}", number, error);
    }
}
