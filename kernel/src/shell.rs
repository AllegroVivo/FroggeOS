// A command prompt.

use alloc::string::String;
use alloc::vec::Vec;
use core::arch::asm;
use ribbit::calc;

use crate::{heap, keyboard, timer, vga};
use crate::{print, println};

// ``> `` + 78 characters fills an 80-character row.
const MAX_LINE: usize = 78;

pub fn run() -> ! {
    println!("FroggeOS shell. Type 'help' for a list of commands.");

    let mut line = [0u8; MAX_LINE];
    let mut len = 0;
    let mut history: Vec<String> = Vec::new();
    let mut vars = calc::Variables::new();
    let mut functions = calc::Functions::new();
    print!("> ");

    loop {
        match wait_for_char() {
            '\n' => {
                println!();
                let text = core::str::from_utf8(&line[..len]).unwrap();
                if !text.trim().is_empty() {
                    history.push(String::from(text)); // heap copy
                }
                run_command(text, &history, &mut vars, &mut functions);
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
            println!("<expression> calculate it, e.g. 2 * (3 + 4) - 10 / 5");
            println!("a < b        compare, giving True or False (also == != <= > >=)");
            println!("a && b       and: both true; also a || b (or: either one) and !a (not)");
            println!("Int64 x = 5  declare x; also Boolean b = true, var y = x + 1, const Int64 N = 3");
            println!("x = <expr>   change a declared variable: x = x + 1, x += 1 or x++");
            println!("a; b         do a, then b");
            // println! requires `{{` and `}}` to print single `{` and `}`.
            println!("if (x) {{}}    run the block if x is true: if (x > 1) {{ y = 2 }} else {{ y = 3 }}");
            println!("while (x) {{}} repeat the block while x is true: while (i < 5) {{ i = i + 1 }}");
            println!("for (;;) {{}}  repeat with a counter: for (Int64 i = 0; i < 5; i++) {{}}");
            println!("break        leave the loop at once; continue skips to the next round");
            println!("Int64 F() {{}} declare a function: Int64 Sq(Int64 n) {{ return n * n }}, then Sq(5)");
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
                println!("{:>3}  {}", i + 1, entry);
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
        _ => match calc::evaluate(line, vars, functions) {
            Ok(Some(value)) => println!("{}", value),
            Ok(None) => {} // line ended with an `if` or `while`.
            Err(error) => println!("Error: {}", error),
        },
    }
}
