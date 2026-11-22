// Unit testing for the kernel.

use alloc::string::String;
use alloc::vec::Vec;
use ribbit::calc;
use crate::{heap, rtc, shell, timer};
use crate::{print, println};

// A check returns Ok or Err.
type Check = fn() -> Result<(), &'static str>;

const CHECKS: [(&str, Check); 4] = [
    ("heap", check_heap),
    ("timer", check_timer),
    ("clock", check_clock),
    ("Ribbit", check_ribbit),
];

pub fn run() -> bool {
    let mut failed = 0;
    for (name, check) in CHECKS {
        print!("{} ... ", name);
        match check() {
            Ok(()) => println!("PASS"),
            Err(why) => {
                println!("FAIL: {}", why);
                failed += 1;
            },
        }
    }
    println!("{} passed, {} failed", CHECKS.len() - failed, failed);
    failed == 0
}

// Closes QEMU thru the isa-debug-exit device.
#[cfg(feature = "selftest")]
pub fn exit_qemu(passed: bool) {
    let value = if passed { 0x10 } else { 0x11 };
    unsafe { crate::port::outb(0xF4, value) };
}

// Grows a Vec and joins Strings plus checks memory releases.
fn check_heap() -> Result<(), &'static str> {
    let (used, _, allocations) = heap::stats();
    {
        let mut numbers: Vec<u64> = Vec::new();
        for n in 1..=1000 {
            numbers.push(n);
        }
        let name = String::from("FroggeOS") + " and Ribbit";
        if numbers.iter().sum::<u64>() != 500_500 || name.len() != 19 {
            return Err("a Vec or String holds the wrong values");
        }
    }
    let (used_after, _, allocations_after) = heap::stats();
    if used_after != used || allocations_after != allocations {
        return Err("memory was not deallocated");
    }
    Ok(())
}

// Sleeping 100ms should take 10-12 ticks.
fn check_timer() -> Result<(), &'static str> {
    let start = timer::ticks();
    timer::sleep(100);
    match timer::ticks() - start {
        10..=12 => Ok(()),
        _ => Err("sleep(100) didn't take 10 ticks"),
    }
}

// The clock should provide a datetime that makes sense.
fn check_clock() -> Result<(), &'static str> {
    let now = rtc::now();
    let sensible = now.year >= 2025
        && (1..=12).contains(&now.month)
        && (1..=31).contains(&now.day)
        && now.hour < 24
        && now.minute < 60
        && now.second < 60;
    if sensible { Ok(()) } else { Err("the date makes no sense") }
}

fn check_ribbit() -> Result<(), &'static str> {
    let mut vars = calc::Variables::new();
    let mut functions = calc::Functions::new();
    let mut run = |line: &'static str| calc::evaluate(line, &mut vars, &mut functions, &mut shell::Kernel);

    let d = run("Int64 D(Int64 n) { if (n == 0) { return 0; } return 1 + D(n - 1); }");
    let f = run("Int64 F(Int64 n) { return F(n + 1); }");
    if d.is_err() || f.is_err() {
        return Err("a function didn't compile");
    }
    if !matches!(run("D(29);"), Ok(Some(calc::Value::Int64(29)))) {
        return Err("D(29) didn't give 29");
    }
    if !matches!(run("F(0);"), Err(calc::Failure { error: calc::Error::TooDeep, .. })) {
        return Err("F(0) wasn't stopped as too deep");
    }
    Ok(())
}
